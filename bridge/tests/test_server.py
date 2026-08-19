import pathlib
import unittest
from types import SimpleNamespace
from unittest.mock import AsyncMock, MagicMock, patch

from bridge.server import Bridge, BridgeError, dispatch, lan_address


class DispatchTests(unittest.IsolatedAsyncioTestCase):
    async def test_health_does_not_require_pyatv(self) -> None:
        result = await dispatch(AsyncMock(), "GET", "/health", b"")
        self.assertEqual({"status": "ok"}, result)

    async def test_unknown_route_is_not_found(self) -> None:
        with self.assertRaises(BridgeError) as context:
            await dispatch(AsyncMock(), "GET", "/missing", b"")
        self.assertEqual(404, context.exception.status)

    def test_lan_address_returns_an_address(self) -> None:
        self.assertTrue(lan_address())

    async def test_play_connects_with_airplay_and_closes(self) -> None:
        bridge = Bridge()
        bridge.storage = object()
        config = object()
        bridge.find = AsyncMock(return_value=config)
        atv = MagicMock()
        atv.stream.play_url = AsyncMock()
        protocol = object()
        pyatv = SimpleNamespace(
            const=SimpleNamespace(Protocol=SimpleNamespace(AirPlay=protocol)),
            connect=AsyncMock(return_value=atv),
        )
        with patch.object(bridge, "_pyatv", return_value=pyatv):
            await bridge.play("device", "https://example.test/video.mp4")

        pyatv.connect.assert_awaited_once()
        atv.stream.play_url.assert_awaited_once_with(
            "https://example.test/video.mp4"
        )
        atv.close.assert_called_once()

    async def test_play_rejects_local_file_url(self) -> None:
        with self.assertRaises(BridgeError):
            await Bridge().play("device", "file:///private/video.mp4")


if __name__ == "__main__":
    unittest.main()
