import pathlib
import unittest
from unittest.mock import AsyncMock

from bridge.server import BridgeError, dispatch, lan_address


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


if __name__ == "__main__":
    unittest.main()
