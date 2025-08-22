import asyncio
import multiprocessing as mp
import typing as typ
from io import BytesIO, StringIO, TextIOWrapper
from pathlib import Path

import msgspec.json as msjson
import pytest
import typer

from weaver import client
from weaver.errors import DependencyErrorCode
from weaver_schemas.error import SchemaError
from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server


@pytest.fixture
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_rpc_call_existing_daemon(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    def status() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(pid=99, rss_mb=1.0, ready=True, message="ok")

    sock = tmp_path / "d.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        buf = StringIO()
        await client.rpc_call("project-status", socket_path=sock, stdout=buf)
        assert msjson.decode(buf.getvalue(), type=ProjectStatus) == ProjectStatus(
            pid=99, rss_mb=1.0, ready=True, message="ok"
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
                    def status() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
                        return ProjectStatus(
                            pid=999, rss_mb=1.0, ready=True, message="ok"
                        )

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

    async def wrapped(path: Path) -> mp.Process:
        nonlocal started
        started = await asyncio.to_thread(spawn, path)
        return started

    monkeypatch.setattr(client, "spawn_daemon", wrapped)

    buf = StringIO()
    await client.rpc_call("project-status", socket_path=sock, stdout=buf)
    assert started is not None
    assert msjson.decode(buf.getvalue(), type=ProjectStatus) == ProjectStatus(
        pid=999, rss_mb=1.0, ready=True, message="ok"
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


def test_resolve_socket_path_passthrough() -> None:
    path = Path("test.sock")
    assert client._resolve_socket_path(path) == path


def test_resolve_socket_path_discover(monkeypatch: pytest.MonkeyPatch) -> None:
    expected = Path("discover.sock")
    monkeypatch.setattr(client, "discover_socket", lambda: expected)
    assert client._resolve_socket_path(None) == expected


@pytest.mark.anyio
async def test_establish_rpc_connection_daemon_error(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    async def fail(path: Path) -> None:  # pragma: no cover - stub
        await asyncio.sleep(0)
        raise client.DaemonStartError()

    monkeypatch.setattr(client, "ensure_daemon_running", fail)
    with pytest.raises(typer.Exit):
        await client._establish_rpc_connection(Path("x.sock"))
    assert "Could not ensure daemon is running" in capsys.readouterr().err


@pytest.mark.anyio
async def test_establish_rpc_connection_connect_error(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    async def succeed(path: Path) -> None:
        await asyncio.sleep(0)

    class ConnectError(OSError):
        """Test-only error to simulate socket failure."""

        def __init__(self) -> None:
            super().__init__("no socket")

    async def connect(path: str) -> None:  # pragma: no cover - stub
        await asyncio.sleep(0)
        raise ConnectError()

    monkeypatch.setattr(client, "ensure_daemon_running", succeed)
    monkeypatch.setattr(asyncio, "open_unix_connection", connect)
    with pytest.raises(typer.Exit):
        await client._establish_rpc_connection(Path("x.sock"))
    assert "Failed to connect to daemon" in capsys.readouterr().err


@pytest.mark.anyio
async def test_establish_rpc_connection_success(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    async def succeed(path: Path) -> None:
        await asyncio.sleep(0)

    reader = typ.cast("asyncio.StreamReader", object())
    writer = typ.cast("asyncio.StreamWriter", object())

    async def connect(path: str) -> tuple[object, object]:  # pragma: no cover - stub
        await asyncio.sleep(0)
        return reader, writer

    monkeypatch.setattr(client, "ensure_daemon_running", succeed)
    monkeypatch.setattr(asyncio, "open_unix_connection", connect)
    assert await client._establish_rpc_connection(Path("x.sock")) == (
        typ.cast("asyncio.StreamReader", reader),
        typ.cast("asyncio.StreamWriter", writer),
    )


class _DummyWriter:
    def __init__(self) -> None:
        self.closed = False
        self.written: list[bytes] = []

    def write(self, data: bytes) -> None:  # pragma: no cover - simple
        self.written.append(data)

    async def drain(self) -> None:  # pragma: no cover - simple
        return None

    def write_eof(self) -> None:  # pragma: no cover - simple
        return None

    def close(self) -> None:  # pragma: no cover - simple
        self.closed = True

    async def wait_closed(self) -> None:  # pragma: no cover - simple
        return None


@pytest.mark.anyio
async def test_execute_rpc_request(monkeypatch: pytest.MonkeyPatch) -> None:
    writer = _DummyWriter()
    reader = typ.cast("asyncio.StreamReader", object())
    stdout = StringIO()

    async def stream(_: asyncio.StreamReader, __: typ.TextIO) -> bool:
        await asyncio.sleep(0)
        return True

    monkeypatch.setattr(client, "_stream_response", stream)
    error = await client._execute_rpc_request(
        reader,
        typ.cast("asyncio.StreamWriter", writer),
        "m",
        None,
        stdout,
    )
    assert error
    assert writer.closed
