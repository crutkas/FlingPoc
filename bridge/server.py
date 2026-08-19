from __future__ import annotations

import argparse
import asyncio
import contextlib
import json
import mimetypes
import os
import pathlib
import secrets
import socket
import subprocess
import threading
import time
import uuid
from dataclasses import dataclass
from http import HTTPStatus
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import urlparse


class BridgeError(Exception):
    def __init__(self, message: str, status: HTTPStatus = HTTPStatus.BAD_REQUEST):
        super().__init__(message)
        self.status = status


@dataclass
class PairingState:
    handler: Any


class Bridge:
    def __init__(self) -> None:
        self.pairings: dict[str, PairingState] = {}
        self.ffmpeg: asyncio.subprocess.Process | None = None
        self.media_server: ThreadingHTTPServer | None = None
        self.media_thread: threading.Thread | None = None
        self.media_root: pathlib.Path | None = None

    @staticmethod
    def _pyatv() -> Any:
        try:
            import pyatv
        except ImportError as error:
            raise BridgeError(
                "pyatv is not installed; run 'python -m pip install -r bridge/requirements.txt'.",
                HTTPStatus.SERVICE_UNAVAILABLE,
            ) from error
        return pyatv

    async def scan(self) -> list[Any]:
        pyatv = self._pyatv()
        return await pyatv.scan(asyncio.get_running_loop(), timeout=5)

    async def find(self, device_id: str, credentials: str | None = None) -> Any:
        pyatv = self._pyatv()
        devices = await self.scan()
        config = next(
            (
                item
                for item in devices
                if str(item.identifier) == device_id or str(item.address) == device_id
            ),
            None,
        )
        if config is None:
            raise BridgeError("Apple TV was not found.", HTTPStatus.NOT_FOUND)
        if credentials:
            config.set_credentials(pyatv.const.Protocol.AirPlay, credentials)
        return config

    async def devices(self) -> list[dict[str, Any]]:
        return [
            {
                "id": str(item.identifier),
                "name": item.name,
                "address": str(item.address),
                "protocols": [service.protocol.name for service in item.services],
            }
            for item in await self.scan()
        ]

    async def pair_start(self, device_id: str) -> dict[str, str]:
        pyatv = self._pyatv()
        config = await self.find(device_id)
        handler = await pyatv.pair(
            config,
            pyatv.const.Protocol.AirPlay,
            asyncio.get_running_loop(),
        )
        await handler.begin()
        session_id = uuid.uuid4().hex
        self.pairings[session_id] = PairingState(handler)
        return {"sessionId": session_id}

    async def pair_finish(self, session_id: str, pin: str) -> dict[str, str]:
        pairing = self.pairings.pop(session_id, None)
        if pairing is None:
            raise BridgeError("Pairing session was not found.", HTTPStatus.NOT_FOUND)
        try:
            if pin:
                pairing.handler.pin(int(pin))
            await pairing.handler.finish()
            if not pairing.handler.has_paired:
                raise BridgeError("Pairing was not accepted.")
            return {"credentials": pairing.handler.service.credentials}
        finally:
            await pairing.handler.close()

    async def play(self, device_id: str, url: str, credentials: str | None) -> None:
        parsed = urlparse(url)
        if parsed.scheme not in {"http", "https"}:
            raise BridgeError("Apple TV playback requires an HTTP or HTTPS URL.")
        pyatv = self._pyatv()
        config = await self.find(device_id, credentials)
        atv = await pyatv.connect(config, asyncio.get_running_loop())
        try:
            await atv.stream.play_url(url)
        finally:
            atv.close()

    async def stop(self, device_id: str, credentials: str | None) -> None:
        pyatv = self._pyatv()
        config = await self.find(device_id, credentials)
        atv = await pyatv.connect(config, asyncio.get_running_loop())
        try:
            await atv.remote_control.stop()
        finally:
            atv.close()
        await self.stop_stream()

    async def play_file(self, device_id: str, path: str, credentials: str | None) -> None:
        media = pathlib.Path(path).resolve(strict=True)
        await self.start_media_server(media.parent)
        url = f"http://{lan_address()}:{self.media_server.server_port}/{media.name}"
        await self.play(device_id, url, credentials)

    async def mirror(
        self,
        device_id: str,
        display: str | None,
        credentials: str | None,
    ) -> None:
        if os.name != "nt":
            raise BridgeError("Desktop capture currently supports Windows only.")
        if not shutil_which("ffmpeg"):
            raise BridgeError("FFmpeg was not found on PATH.", HTTPStatus.SERVICE_UNAVAILABLE)

        await self.stop_stream()
        root = pathlib.Path(os.getenv("TEMP", ".")) / f"fling-{secrets.token_hex(8)}"
        root.mkdir(parents=True)
        await self.start_media_server(root)
        playlist = root / "desktop.m3u8"
        desktop = f"desktop{display}" if display else "desktop"
        args = [
            "ffmpeg", "-hide_banner", "-loglevel", "warning",
            "-f", "gdigrab", "-framerate", "30", "-i", desktop,
            "-c:v", "libx264", "-preset", "veryfast", "-tune", "zerolatency",
            "-pix_fmt", "yuv420p", "-profile:v", "high", "-level", "4.1",
            "-g", "30", "-keyint_min", "30", "-sc_threshold", "0",
            "-f", "hls", "-hls_time", "1", "-hls_list_size", "6",
            "-hls_flags", "delete_segments+append_list+independent_segments",
            str(playlist),
        ]
        self.ffmpeg = await asyncio.create_subprocess_exec(
            *args,
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.PIPE,
        )
        for _ in range(100):
            if playlist.exists():
                break
            if self.ffmpeg.returncode is not None:
                error = (await self.ffmpeg.stderr.read()).decode(errors="replace")
                raise BridgeError(f"FFmpeg capture failed: {error.strip()}")
            await asyncio.sleep(0.1)
        else:
            raise BridgeError("FFmpeg did not produce a stream in time.")
        await self.play(
            device_id,
            f"http://{lan_address()}:{self.media_server.server_port}/desktop.m3u8",
            credentials,
        )

    async def start_media_server(self, root: pathlib.Path) -> None:
        if self.media_server is not None:
            self.media_server.shutdown()
            self.media_server.server_close()
        self.media_root = root
        handler = lambda *args, **kwargs: QuietFileHandler(  # noqa: E731
            *args, directory=str(root), **kwargs
        )
        self.media_server = ThreadingHTTPServer(("0.0.0.0", 0), handler)
        self.media_thread = threading.Thread(
            target=self.media_server.serve_forever,
            name="fling-media",
            daemon=True,
        )
        self.media_thread.start()

    async def stop_stream(self) -> None:
        if self.ffmpeg is not None and self.ffmpeg.returncode is None:
            self.ffmpeg.terminate()
            with contextlib.suppress(asyncio.TimeoutError):
                await asyncio.wait_for(self.ffmpeg.wait(), timeout=3)
            if self.ffmpeg.returncode is None:
                self.ffmpeg.kill()
        self.ffmpeg = None

    async def close(self) -> None:
        await self.stop_stream()
        if self.media_server is not None:
            self.media_server.shutdown()
            self.media_server.server_close()
            self.media_server = None
        for pairing in self.pairings.values():
            await pairing.handler.close()
        self.pairings.clear()


class QuietFileHandler(SimpleHTTPRequestHandler):
    def log_message(self, format: str, *args: Any) -> None:
        pass

    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-store")
        self.send_header("Access-Control-Allow-Origin", "*")
        super().end_headers()


def lan_address() -> str:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as connection:
        try:
            connection.connect(("192.0.2.1", 80))
            return str(connection.getsockname()[0])
        except OSError:
            return "127.0.0.1"


def shutil_which(command: str) -> str | None:
    paths = os.environ.get("PATH", "").split(os.pathsep)
    extensions = os.environ.get("PATHEXT", "").split(os.pathsep) if os.name == "nt" else [""]
    return next(
        (
            str(candidate)
            for path in paths
            for extension in extensions
            if (candidate := pathlib.Path(path) / f"{command}{extension}").is_file()
        ),
        None,
    )


async def read_request(reader: asyncio.StreamReader) -> tuple[str, str, dict[str, str], bytes]:
    line = await reader.readline()
    if not line:
        raise BridgeError("Empty request.")
    method, path, _ = line.decode("ascii").strip().split(" ", 2)
    headers: dict[str, str] = {}
    while True:
        line = await reader.readline()
        if line in {b"\r\n", b"\n", b""}:
            break
        name, value = line.decode("ascii").split(":", 1)
        headers[name.lower()] = value.strip()
    length = int(headers.get("content-length", "0"))
    if length > 1_048_576:
        raise BridgeError("Request body is too large.", HTTPStatus.REQUEST_ENTITY_TOO_LARGE)
    return method, path, headers, await reader.readexactly(length)


async def dispatch(bridge: Bridge, method: str, path: str, body: bytes) -> Any:
    data = json.loads(body or b"{}")
    if method == "GET" and path == "/health":
        return {"status": "ok"}
    if method == "GET" and path == "/v1/devices":
        return await bridge.devices()
    if method == "POST" and path == "/v1/pair/start":
        return await bridge.pair_start(data["deviceId"])
    if method == "POST" and path == "/v1/pair/finish":
        return await bridge.pair_finish(data["sessionId"], data["pin"])
    if method == "POST" and path == "/v1/play":
        await bridge.play(data["deviceId"], data["url"], data.get("credentials"))
        return {}
    if method == "POST" and path == "/v1/file":
        await bridge.play_file(data["deviceId"], data["path"], data.get("credentials"))
        return {}
    if method == "POST" and path == "/v1/mirror":
        await bridge.mirror(data["deviceId"], data.get("display"), data.get("credentials"))
        return {}
    if method == "POST" and path == "/v1/stop":
        await bridge.stop(data["deviceId"], data.get("credentials"))
        return {}
    raise BridgeError("Route not found.", HTTPStatus.NOT_FOUND)


async def handle(
    reader: asyncio.StreamReader,
    writer: asyncio.StreamWriter,
    bridge: Bridge,
    token: str,
) -> None:
    status = HTTPStatus.OK
    try:
        method, path, headers, body = await asyncio.wait_for(read_request(reader), timeout=15)
        if not secrets.compare_digest(headers.get("authorization", ""), f"******"):
            raise BridgeError("Unauthorized.", HTTPStatus.UNAUTHORIZED)
        payload = await dispatch(bridge, method, path, body)
    except BridgeError as error:
        status, payload = error.status, {"error": str(error)}
    except (KeyError, ValueError, json.JSONDecodeError) as error:
        status, payload = HTTPStatus.BAD_REQUEST, {"error": f"Invalid request: {error}"}
    except Exception as error:
        status, payload = HTTPStatus.INTERNAL_SERVER_ERROR, {"error": str(error)}
    encoded = json.dumps(payload).encode()
    writer.write(
        f"HTTP/1.1 {status.value} {status.phrase}\r\n"
        f"Content-Type: application/json\r\nContent-Length: {len(encoded)}\r\n"
        "Connection: close\r\n\r\n".encode()
        + encoded
    )
    await writer.drain()
    writer.close()
    await writer.wait_closed()


async def serve(port: int) -> None:
    token = os.environ.get("FLING_TOKEN")
    if not token:
        raise RuntimeError("FLING_TOKEN must be set.")
    bridge = Bridge()
    server = await asyncio.start_server(
        lambda reader, writer: handle(reader, writer, bridge, token),
        "127.0.0.1",
        port,
    )
    try:
        async with server:
            await server.serve_forever()
    finally:
        await bridge.close()


def main() -> None:
    parser = argparse.ArgumentParser(description="FlingPoc local pyatv bridge")
    parser.add_argument("--port", type=int, required=True)
    args = parser.parse_args()
    asyncio.run(serve(args.port))
