from __future__ import annotations

import pytest
from msgspec import json

from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_dispatcher_handles_registered_method() -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    async def handler() -> ProjectStatus:
        return ProjectStatus(message="ok")

    request = json.encode({"method": "project-status"})
    response = await dispatcher.handle(request)
    assert json.decode(response, type=ProjectStatus) == ProjectStatus(message="ok")
