import asyncio
import multiprocessing as mp
import typing as typ
from io import BytesIO, StringIO, TextIOWrapper
from pathlib import Path

import msgspec.json as msjson
import pytest

from weaver import client
from weaver.errors import DependencyErrorCode
from weaver_schemas.error import SchemaError
from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_rpc_call_existing_daemon(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    async def status() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(message="ok")

    sock = tmp_path / "d.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        buf = StringIO()
        await client.rpc_call("project-status", socket_path=sock, stdout=buf)
        assert msjson.decode(buf.getvalue(), type=ProjectStatus) == ProjectStatus(
            message="ok"
        )
    server.close()
    await server.wait_closed()


@pytest.mark.anyio
async def test_rpc_call_autostart(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    sock = tmp_path / "auto.sock"

    def spawn(path: Path) -> mp.Process:
        def _run() -> None:
            # Avoid event loop conflicts in subprocesses
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:

                async def _serve() -> None:
                    dispatcher = RPCDispatcher()

                    @dispatcher.register("project-status")
                    async def status() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
                        return ProjectStatus(message="ok")

                    server = await start_server(path, dispatcher)
                    async with server:
                        await server.serve_forever()

                loop.run_until_complete(_serve())
            finally:
                loop.close()

        proc = mp.Process(target=_run)
        proc.start()
        return proc

    started: mp.Process | None = None

    def wrapped(path: Path) -> mp.Process:
        nonlocal started
        started = spawn(path)
        return started

    monkeypatch.setattr(client, "spawn_daemon", wrapped)

    buf = StringIO()
    await client.rpc_call("project-status", socket_path=sock, stdout=buf)
    assert started is not None
    assert msjson.decode(buf.getvalue(), type=ProjectStatus) == ProjectStatus(
        message="ok"
    )
    if started and started.is_alive():
        started.terminate()
        try:
            started.join(timeout=5.0)
        finally:
            if started.is_alive():
                started.kill()


@pytest.mark.anyio
async def test_rpc_call_unknown_method(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()
    sock = tmp_path / "unk.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        buf = StringIO()
        await client.rpc_call("nope", socket_path=sock, stdout=buf)
        err = msjson.decode(buf.getvalue(), type=SchemaError)
        assert err.message == "unknown method: nope"
    server.close()
    await server.wait_closed()


@pytest.mark.parametrize("code", list(DependencyErrorCode))
def test_process_response_line_detects_error_codes(code: DependencyErrorCode) -> None:
    out = StringIO()
    line = msjson.encode({"type": "error", "error_code": code}) + b"\n"
    assert client._process_response_line(line, out)


@pytest.mark.parametrize("code", list(DependencyErrorCode))
def test_process_response_line_detects_code_key(code: DependencyErrorCode) -> None:
    out = StringIO()
    line = msjson.encode({"type": "error", "code": code}) + b"\n"
    assert client._process_response_line(line, out)


def test_process_response_line_non_dependency_error_code() -> None:
    out = StringIO()
    line = msjson.encode({"type": "error", "error_code": "SOME_OTHER_ERROR"}) + b"\n"
    assert not client._process_response_line(line, out)


@pytest.mark.parametrize(
    "payload",
    [
        {"type": "info", "message": "not an error"},
        {"foo": "bar"},
        {},
    ],
)
def test_process_response_line_non_error_type(payload: dict[str, typ.Any]) -> None:
    out = StringIO()
    line = msjson.encode(payload) + b"\n"
    assert not client._process_response_line(line, out)


@pytest.mark.parametrize(
    "message",
    [
        "missing dependency serena-agent",
        "missing dependency: foo-lib",
        "Serena-Agent not found",
        "serena agent unavailable",
    ],
)
def test_process_response_line_detects_error_messages(message: str) -> None:
    out = StringIO()
    line = msjson.encode({"type": "error", "message": message}) + b"\n"
    assert client._process_response_line(line, out)


def test_process_response_line_buffered_stdout() -> None:
    buf = BytesIO()
    out = TextIOWrapper(buf, encoding="utf-8")
    line = msjson.encode({"type": "result", "value": 42}) + b"\n"
    assert not client._process_response_line(line, out)
    out.flush()
    assert buf.getvalue() == line
