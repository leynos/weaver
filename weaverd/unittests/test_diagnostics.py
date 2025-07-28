from __future__ import annotations

import asyncio
import typing as typ

import msgspec.json as msjson
import pytest

from weaver_schemas.diagnostics import Diagnostic
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

    monkeypatch.setattr(server, "create_diagnostics_tool", lambda: StubTool())

    @dispatcher.register("list-diagnostics")
    async def handler(
        severity: str | None = None,
        files: list[str] | None = None,
    ) -> list[Diagnostic]:  # pyright: ignore[reportUnusedFunction]
        tool = server.create_diagnostics_tool()
        items = tool.list_diagnostics()
        if severity:
            items = [d for d in items if d.severity == severity]
        if files:
            items = [d for d in items if d.location.file in files]
        return items

    sock = tmp_path / "d.sock"
    srv = await start_server(sock, dispatcher)
    async with srv:
        reader, writer = await asyncio.open_unix_connection(str(sock))
        writer.write(msjson.encode({"method": "list-diagnostics"}) + b"\n")
        await writer.drain()
        data = await reader.readline()
        diags = msjson.decode(data.rstrip(), type=list[Diagnostic])
        assert len(diags) == 1
        assert diags[0].message == "boom"
        writer.close()
        await writer.wait_closed()
    srv.close()
    await srv.wait_closed()


@pytest.mark.anyio
async def test_list_diagnostics_filtered(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    dispatcher = RPCDispatcher()

    monkeypatch.setattr(server, "create_diagnostics_tool", lambda: StubTool())

    @dispatcher.register("list-diagnostics")
    async def handler(
        severity: str | None = None,
        files: list[str] | None = None,
    ) -> list[Diagnostic]:  # pragma: no cover - stub
        tool = server.create_diagnostics_tool()
        items = tool.list_diagnostics()
        if severity:
            items = [d for d in items if d.severity == severity]
        if files:
            items = [d for d in items if d.location.file in files]
        return items

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
        data = await reader.readline()
        diags = msjson.decode(data.rstrip(), type=list[Diagnostic])
        assert diags == []
        writer.close()
        await writer.wait_closed()
    srv.close()
    await srv.wait_closed()
