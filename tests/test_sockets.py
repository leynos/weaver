import pathlib

import pytest

from weaver.sockets import can_connect


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_can_connect_nonexistent(tmp_path: pathlib.Path) -> None:
    path = tmp_path / "sock"
    result = await can_connect(str(path))
    assert result is False
