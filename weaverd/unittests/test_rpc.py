from __future__ import annotations

import msgspec.json as msjson
import pytest

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
    async def handler() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(pid=1, rss_mb=0.1, ready=True, message="ok")

    request = msjson.encode({"method": "project-status"})
    response = await dispatcher.handle(request)
    assert msjson.decode(response, type=ProjectStatus) == ProjectStatus(
        pid=1, rss_mb=0.1, ready=True, message="ok"
    )


@pytest.mark.anyio
async def test_dispatcher_passes_parameters() -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("echo")
    async def echo(value: int) -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(
            pid=value, rss_mb=float(value), ready=True, message=str(value)
        )

    request = msjson.encode({"method": "echo", "params": {"value": 42}})
    response = await dispatcher.handle(request)
    assert msjson.decode(response, type=ProjectStatus) == ProjectStatus(
        pid=42, rss_mb=42.0, ready=True, message="42"
    )


@pytest.mark.anyio
async def test_dispatcher_returns_error_on_unknown_method() -> None:
    dispatcher = RPCDispatcher()

    request = msjson.encode({"method": "missing"})
    response = await dispatcher.handle(request)
    assert msjson.decode(response, type=SchemaError) == SchemaError(
        message="unknown method: missing"
    )


@pytest.mark.anyio
async def test_dispatcher_returns_error_on_bad_json() -> None:
    dispatcher = RPCDispatcher()

    response = await dispatcher.handle(b"not-json")
    err = msjson.decode(response, type=SchemaError)
    assert err.type == "error" and "invalid request" in err.message
