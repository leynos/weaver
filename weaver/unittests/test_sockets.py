import pathlib
import socket

import pytest

from weaver.sockets import can_connect


@pytest.fixture
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_can_connect_nonexistent(tmp_path: pathlib.Path) -> None:
    path = tmp_path / "sock"
    result = await can_connect(str(path))
    assert result is False


@pytest.mark.anyio
async def test_can_connect_existing_socket(tmp_path: pathlib.Path) -> None:
    sock_path = tmp_path / "test_socket"
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.bind(str(sock_path))
    sock.listen(1)
    try:
        result = await can_connect(str(sock_path))
        assert result is True
    finally:
        sock.close()
