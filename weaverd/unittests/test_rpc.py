from __future__ import annotations

import builtins
import typing as typ

import msgspec.json as msjson
import pytest

from weaver_schemas.error import SchemaError
from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


async def _test_dispatcher_call(
    dispatcher: RPCDispatcher,
    request_data: dict[str, typ.Any],
    expected_response: ProjectStatus,
) -> None:
    """Helper to send an RPC request and assert the response."""
    request = msjson.encode(request_data)
    results = dispatcher.handle(request)
    response = await builtins.anext(results)
    assert msjson.decode(response, type=ProjectStatus) == expected_response


@pytest.mark.anyio
async def test_dispatcher_handles_registered_method() -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    async def handler() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(pid=1, rss_mb=0.1, ready=True, message="ok")

    await _test_dispatcher_call(
        dispatcher,
        {"method": "project-status"},
        ProjectStatus(pid=1, rss_mb=0.1, ready=True, message="ok"),
    )


@pytest.mark.anyio
async def test_dispatcher_passes_parameters() -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("echo")
    async def echo(value: int) -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(
            pid=value, rss_mb=float(value), ready=True, message=str(value)
        )

    await _test_dispatcher_call(
        dispatcher,
        {"method": "echo", "params": {"value": 42}},
        ProjectStatus(pid=42, rss_mb=42.0, ready=True, message="42"),
    )


@pytest.mark.anyio
async def test_dispatcher_returns_error_on_unknown_method() -> None:
    dispatcher = RPCDispatcher()

    request = msjson.encode({"method": "missing"})
    results = dispatcher.handle(request)
    response = await builtins.anext(results)
    assert msjson.decode(response, type=SchemaError) == SchemaError(
        message="unknown method: missing"
    )


@pytest.mark.anyio
async def test_dispatcher_returns_error_on_bad_json() -> None:
    dispatcher = RPCDispatcher()

    results = dispatcher.handle(b"not-json")
    err = msjson.decode(await builtins.anext(results), type=SchemaError)
    assert err.type == "error" and "invalid request" in err.message


@pytest.mark.anyio
async def test_dispatcher_streams_multiple_results() -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("numbers")
    async def numbers() -> typ.AsyncIterator[int]:  # pyright: ignore[reportUnusedFunction]
        for i in range(3):
            yield i

    req = msjson.encode({"method": "numbers"})
    results = dispatcher.handle(req)
    output = [msjson.decode(await builtins.anext(results), type=int) for _ in range(3)]
    with pytest.raises(StopAsyncIteration):
        await builtins.anext(results)
    assert output == [0, 1, 2]


@pytest.mark.anyio
async def test_dispatcher_streams_midstream_error() -> None:
    dispatcher = RPCDispatcher()

    async def boom() -> typ.AsyncIterator[int]:
        yield 1
        raise RuntimeError("boom")

    @dispatcher.register("boom")
    async def handler() -> typ.AsyncIterator[int]:  # pyright: ignore[reportUnusedFunction]
        async for item in boom():
            yield item

    req = msjson.encode({"method": "boom"})
    results = dispatcher.handle(req)
    first = msjson.decode(await builtins.anext(results), type=int)
    err = msjson.decode(await builtins.anext(results), type=SchemaError)
    with pytest.raises(StopAsyncIteration):
        await builtins.anext(results)
    assert first == 1 and err.message == "boom"
