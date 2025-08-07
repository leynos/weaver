from __future__ import annotations

import asyncio
import builtins
import typing as typ

import msgspec.json as msjson
import pytest

from weaver_schemas.diagnostics import Diagnostic
from weaver_schemas.error import SchemaError
from weaver_schemas.primitives import Location, Position, Range
from weaverd import server
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server

if typ.TYPE_CHECKING:
    from pathlib import Path


class StubTool:
    def list_diagnostics(self) -> list[Diagnostic]:
        loc = Location(
            file="foo.py", range=Range(start=Position(1, 0), end=Position(1, 1))
        )
        return [Diagnostic(location=loc, severity="Error", code="E1", message="boom")]


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_list_diagnostics(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    dispatcher = RPCDispatcher()

    monkeypatch.setattr(server, "create_serena_tool", lambda _: StubTool())

    @dispatcher.register("list-diagnostics")
    async def handler(
        severity: str | None = None,
        files: list[str] | None = None,
    ) -> typ.AsyncIterator[Diagnostic]:
        tool = server.create_serena_tool("ListDiagnosticsTool")
        for d in tool.list_diagnostics():
            if severity and d.severity != severity:
                continue
            if files and d.location.file not in files:
                continue
            yield d

    sock = tmp_path / "d.sock"
    srv = await start_server(sock, dispatcher)
    async with srv:
        reader, writer = await asyncio.open_unix_connection(str(sock))
        writer.write(msjson.encode({"method": "list-diagnostics"}) + b"\n")
        await writer.drain()
        writer.write_eof()
        data = await reader.readline()
        diag = msjson.decode(data.rstrip(), type=Diagnostic)
        assert diag.message == "boom"
        writer.close()
        await writer.wait_closed()
    srv.close()
    await srv.wait_closed()


def test_unknown_tool_attribute(monkeypatch: pytest.MonkeyPatch) -> None:
    """create_serena_tool should error for unknown tool attributes."""

    from weaverd import serena_tools

    class ToolsMod:  # pragma: no cover - simple stub
        pass

    class PromptMod:  # pragma: no cover - simple stub
        class SerenaPromptFactory:  # pragma: no cover - simple stub
            def __call__(self) -> None:  # pragma: no cover - simple stub
                return None

    def fake_import(name: str) -> typ.Any:  # pragma: no cover - simple stub
        if name == "serena.tools.workflow_tools":
            return ToolsMod()
        if name == "serena.prompt_factory":
            return PromptMod
        raise ModuleNotFoundError

    monkeypatch.setattr(serena_tools, "import_module", fake_import)

    with pytest.raises(RuntimeError, match="NoSuchTool"):
        serena_tools.create_serena_tool("NoSuchTool")


@pytest.mark.anyio
async def test_list_diagnostics_filtered(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    dispatcher = RPCDispatcher()

    monkeypatch.setattr(server, "create_serena_tool", lambda _: StubTool())

    @dispatcher.register("list-diagnostics")
    async def handler(
        severity: str | None = None,
        files: list[str] | None = None,
    ) -> typ.AsyncIterator[Diagnostic]:  # pragma: no cover - stub
        tool = server.create_serena_tool("ListDiagnosticsTool")
        for d in tool.list_diagnostics():
            if severity and d.severity != severity:
                continue
            if files and d.location.file not in files:
                continue
            yield d

    sock = tmp_path / "f.sock"
    srv = await start_server(sock, dispatcher)
    async with srv:
        reader, writer = await asyncio.open_unix_connection(str(sock))
        writer.write(
            msjson.encode(
                {
                    "method": "list-diagnostics",
                    "params": {"severity": "Warning", "files": ["foo.py"]},
                }
            )
            + b"\n"
        )
        await writer.drain()
        writer.write_eof()
        data = await reader.readline()
        assert data == b""  # no results
        writer.close()
        await writer.wait_closed()
    srv.close()
    await srv.wait_closed()


@pytest.mark.anyio
async def test_missing_diagnostics_dependency(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    dispatcher = RPCDispatcher()

    def raise_error(_: str) -> StubTool:
        raise RuntimeError("serena-agent not found")

    monkeypatch.setattr(server, "create_serena_tool", raise_error)

    @dispatcher.register("list-diagnostics")
    async def handler() -> typ.AsyncIterator[Diagnostic]:  # pragma: no cover - stub
        tool = server.create_serena_tool("ListDiagnosticsTool")
        for d in tool.list_diagnostics():
            yield d

    sock = tmp_path / "e.sock"
    srv = await start_server(sock, dispatcher)
    async with srv:
        reader, writer = await asyncio.open_unix_connection(str(sock))
        writer.write(msjson.encode({"method": "list-diagnostics"}) + b"\n")
        await writer.drain()
        writer.write_eof()
        data = await reader.readline()
        assert data, "no response"
        err = msjson.decode(data.rstrip(), type=SchemaError)
        assert "serena-agent" in err.message
        writer.close()
        await writer.wait_closed()
    srv.close()
    await srv.wait_closed()


@pytest.mark.anyio
async def test_list_diagnostics_case_insensitive(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class Tool:
        def list_diagnostics(self) -> list[Diagnostic]:
            loc = Location(
                file="Foo.Py",
                range=Range(start=Position(1, 0), end=Position(1, 1)),
            )
            return [
                Diagnostic(location=loc, severity="Error", code="E1", message="boom")
            ]

    monkeypatch.setattr(server, "create_serena_tool", lambda _: Tool())

    results = server.handle_list_diagnostics(severity="error", files=["foo.py"])
    diag = await builtins.anext(results)
    with pytest.raises(StopAsyncIteration):
        await builtins.anext(results)
    assert diag.message == "boom"
