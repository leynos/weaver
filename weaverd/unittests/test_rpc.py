from __future__ import annotations

import pytest
from msgspec import json

from weaver_schemas.error import SchemaError
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


@pytest.mark.anyio
async def test_dispatcher_passes_parameters() -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("echo")
    async def echo(value: int) -> ProjectStatus:
        return ProjectStatus(message=str(value))

    request = json.encode({"method": "echo", "params": {"value": 42}})
    response = await dispatcher.handle(request)
    assert json.decode(response, type=ProjectStatus) == ProjectStatus(message="42")


@pytest.mark.anyio
async def test_dispatcher_returns_error_on_unknown_method() -> None:
    dispatcher = RPCDispatcher()

    request = json.encode({"method": "missing"})
    response = await dispatcher.handle(request)
    assert json.decode(response, type=SchemaError) == SchemaError(
        message="unknown method: missing"
    )


@pytest.mark.anyio
async def test_dispatcher_returns_error_on_bad_json() -> None:
    dispatcher = RPCDispatcher()

    response = await dispatcher.handle(b"not-json")
    err = json.decode(response, type=SchemaError)
    assert err.type == "error" and "invalid request" in err.message
