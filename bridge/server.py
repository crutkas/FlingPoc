from __future__ import annotations

import argparse
import asyncio
import contextlib
import json
import os
import pathlib
import secrets
import socket
import subprocess
import sys
import tempfile
import threading
import uuid
from dataclasses import dataclass
from http import HTTPStatus
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from typing import Any
from urllib.parse import quote, unquote, urlparse


class BridgeError(Exception):
    def __init__(self, message: str, status: HTTPStatus = HTTPStatus.BAD_REQUEST):
        super().__init__(message)
        self.status = status


@dataclass
class PairingState:
    handler: Any
    config: Any


class Bridge:
    def __init__(self) -> None:
        self.pairings: dict[str, PairingState] = {}
        self.ffmpeg: subprocess.Popen[bytes] | None = None
        self.ffmpeg_log: Any = None
        self.playback_task: asyncio.Task[None] | None = None
        self.media_server: ThreadingHTTPServer | None = None
        self.media_thread: threading.Thread | None = None
        self.media_root: pathlib.Path | None = None
        self.media_token = ""
        self.storage: Any = None

    async def initialize(self) -> None:
        self._pyatv()
        from pyatv.storage.file_storage import FileStorage

        self.storage = FileStorage.default_storage(asyncio.get_running_loop())
        await self.storage.load()

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
        return await pyatv.scan(
            asyncio.get_running_loop(),
            timeout=5,
            storage=self.storage,
        )

    async def find(self, device_id: str) -> Any:
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
        self.pairings[session_id] = PairingState(handler, config)
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
            await self.storage.update_settings(pairing.config)
            await self.storage.save()
            return {}
        finally:
            await pairing.handler.close()

    async def play(self, device_id: str, url: str) -> None:
        parsed = urlparse(url)
        if parsed.scheme not in {"http", "https"}:
            raise BridgeError("Apple TV playback requires an HTTP or HTTPS URL.")
        config = await self.find(device_id)
        await self.start_playback(config, url)

    async def start_playback(self, config: Any, url: str) -> None:
        await self.stop_playback()
        self.playback_task = asyncio.create_task(self.run_playback(config, url))
        await asyncio.sleep(0.25)
        if self.playback_task.done():
            await self.playback_task

    async def run_playback(self, config: Any, url: str) -> None:
        pyatv = self._pyatv()
        atv = await pyatv.connect(
            config,
            asyncio.get_running_loop(),
            protocol=pyatv.const.Protocol.AirPlay,
            storage=self.storage,
        )
        try:
            await atv.stream.play_url(url)
        finally:
            atv.close()

    async def stop_playback(self) -> None:
        if self.playback_task is not None and not self.playback_task.done():
            self.playback_task.cancel()
            with contextlib.suppress(asyncio.CancelledError):
                await self.playback_task
        self.playback_task = None

    async def stop(self, device_id: str) -> None:
        pyatv = self._pyatv()
        config = await self.find(device_id)
        atv = await pyatv.connect(
            config,
            asyncio.get_running_loop(),
            protocol=pyatv.const.Protocol.AirPlay,
            storage=self.storage,
        )
        try:
            await atv.remote_control.stop()
        finally:
            atv.close()
        await self.stop_playback()
        await self.stop_stream()

    async def status(self, device_id: str) -> dict[str, Any]:
        pyatv = self._pyatv()
        config = await self.find(device_id)
        atv = await pyatv.connect(
            config,
            asyncio.get_running_loop(),
            protocol=pyatv.const.Protocol.AirPlay,
            storage=self.storage,
        )
        try:
            playing = await atv.metadata.playing()
            return {
                "state": playing.device_state.name,
                "title": playing.title,
                "position": playing.position,
                "duration": playing.total_time,
            }
        finally:
            atv.close()

    async def play_file(self, device_id: str, path: str) -> None:
        media = pathlib.Path(path).resolve(strict=True)
        config = await self.find(device_id)
        await self.start_media_server(media.parent, media.name)
        url = (
            f"http://{lan_address(config.address)}:{self.media_server.server_port}/"
            f"{self.media_token}/{quote(media.name)}"
        )
        await self.start_playback(config, url)

    async def mirror(
        self,
        device_id: str,
    ) -> None:
        if os.name != "nt":
            raise BridgeError("Desktop capture currently supports Windows only.")
        if not shutil_which("ffmpeg"):
            raise BridgeError("FFmpeg was not found on PATH.", HTTPStatus.SERVICE_UNAVAILABLE)

        config = await self.find(device_id)
        await self.stop_stream()
        root = pathlib.Path(os.getenv("TEMP", ".")) / f"fling-{secrets.token_hex(8)}"
        root.mkdir(parents=True)
        await self.start_media_server(root)
        playlist = root / "desktop.m3u8"
        args = [
            "ffmpeg", "-hide_banner", "-loglevel", "warning",
            "-f", "gdigrab", "-framerate", "30", "-i", "desktop",
            "-c:v", "libx264", "-preset", "veryfast", "-tune", "zerolatency",
            "-pix_fmt", "yuv420p", "-profile:v", "high", "-level", "4.1",
            "-g", "30", "-keyint_min", "30", "-sc_threshold", "0",
            "-f", "hls", "-hls_time", "1", "-hls_list_size", "6",
            "-hls_flags", "delete_segments+append_list+independent_segments",
            str(playlist),
        ]
        self.ffmpeg_log = tempfile.TemporaryFile()
        self.ffmpeg = subprocess.Popen(
            *args,
            stdout=subprocess.DEVNULL,
            stderr=self.ffmpeg_log,
        )
        for _ in range(100):
            if playlist.exists():
                break
            if self.ffmpeg.poll() is not None:
                self.ffmpeg_log.seek(0)
                error = self.ffmpeg_log.read().decode(errors="replace")
                raise BridgeError(f"FFmpeg capture failed: {error.strip()}")
            await asyncio.sleep(0.1)
        else:
            raise BridgeError("FFmpeg did not produce a stream in time.")
        await self.start_playback(
            config,
            f"http://{lan_address(config.address)}:{self.media_server.server_port}/"
            f"{self.media_token}/desktop.m3u8",
        )

    async def start_media_server(
        self,
        root: pathlib.Path,
        allowed_file: str | None = None,
    ) -> None:
        if self.media_server is not None:
            self.media_server.shutdown()
            self.media_server.server_close()
        self.media_root = root
        self.media_token = secrets.token_urlsafe(24)
        handler = lambda *args, **kwargs: QuietFileHandler(  # noqa: E731
            *args,
            directory=str(root),
            allowed_file=allowed_file,
            url_token=self.media_token,
            **kwargs,
        )
        self.media_server = ThreadingHTTPServer(("0.0.0.0", 0), handler)
        self.media_thread = threading.Thread(
            target=self.media_server.serve_forever,
            name="fling-media",
            daemon=True,
        )
        self.media_thread.start()

    async def stop_stream(self) -> None:
        if self.ffmpeg is not None and self.ffmpeg.poll() is None:
            self.ffmpeg.terminate()
            try:
                await asyncio.to_thread(self.ffmpeg.wait, 3)
            except subprocess.TimeoutExpired:
                self.ffmpeg.kill()
        self.ffmpeg = None
        if self.ffmpeg_log is not None:
            self.ffmpeg_log.close()
            self.ffmpeg_log = None

    async def close(self) -> None:
        await self.stop_playback()
        await self.stop_stream()
        if self.media_server is not None:
            self.media_server.shutdown()
            self.media_server.server_close()
            self.media_server = None
        for pairing in self.pairings.values():
            await pairing.handler.close()
        self.pairings.clear()


class QuietFileHandler(SimpleHTTPRequestHandler):
    def __init__(
        self,
        *args: Any,
        allowed_file: str | None = None,
        url_token: str,
        **kwargs: Any,
    ):
        self.allowed_file = allowed_file
        self.url_token = url_token
        super().__init__(*args, **kwargs)

    def send_head(self) -> Any:
        requested = unquote(urlparse(self.path).path).lstrip("/")
        token, separator, requested = requested.partition("/")
        if not separator or not secrets.compare_digest(token, self.url_token):
            self.send_error(HTTPStatus.NOT_FOUND)
            return None
        if self.allowed_file is not None and requested != self.allowed_file:
            self.send_error(HTTPStatus.NOT_FOUND)
            return None
        if self.allowed_file is None and pathlib.PurePosixPath(requested).suffix not in {
            ".m3u8",
            ".ts",
        }:
            self.send_error(HTTPStatus.NOT_FOUND)
            return None
        self.path = "/" + quote(requested)
        return super().send_head()

    def list_directory(self, path: str) -> None:
        self.send_error(HTTPStatus.NOT_FOUND)
        return None

    def log_message(self, format: str, *args: Any) -> None:
        pass

    def end_headers(self) -> None:
        self.send_header("Cache-Control", "no-store")
        self.send_header("Access-Control-Allow-Origin", "*")
        super().end_headers()


def lan_address(target: Any = "192.0.2.1") -> str:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as connection:
        try:
            connection.connect((str(target), 80))
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
        await bridge.play(data["deviceId"], data["url"])
        return {}
    if method == "POST" and path == "/v1/file":
        await bridge.play_file(data["deviceId"], data["path"])
        return {}
    if method == "POST" and path == "/v1/mirror":
        await bridge.mirror(data["deviceId"])
        return {}
    if method == "POST" and path == "/v1/status":
        return await bridge.status(data["deviceId"])
    if method == "POST" and path == "/v1/stop":
        await bridge.stop(data["deviceId"])
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
        if not secrets.compare_digest(
            headers.get("authorization", ""),
            "Bearer " + token,
        ):
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
    await bridge.initialize()
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
    if sys.platform == "win32":
        asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())
    asyncio.run(serve(args.port))
