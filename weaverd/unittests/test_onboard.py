from __future__ import annotations

import asyncio
import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path

import msgspec.json as msjson
import pytest

from weaver_schemas.reports import OnboardingReport
from weaverd import serena_tools
from weaverd.rpc import RPCDispatcher
from weaverd.serena_tools import SerenaTool, create_serena_tool
from weaverd.server import start_server


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_onboard_project(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    @dispatcher.register("onboard-project")
    async def onboard() -> OnboardingReport:  # pyright: ignore[reportUnusedFunction]
        tool = create_serena_tool(SerenaTool.ONBOARDING)
        return OnboardingReport(details=tool.apply())

    sock = tmp_path / "o.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        try:
            reader, writer = await asyncio.wait_for(
                asyncio.open_unix_connection(str(sock)), timeout=5.0
            )
            writer.write(msjson.encode({"method": "onboard-project"}) + b"\n")
            await writer.drain()
            data = await asyncio.wait_for(reader.readline(), timeout=5.0)
            report = msjson.decode(data.rstrip(), type=OnboardingReport)
            text = report.details.lower()
            assert "viewing" in text and "project" in text
            assert len(report.details) > 20
        finally:
            writer.close()
            await writer.wait_closed()
    server.close()
    await server.wait_closed()


@pytest.mark.anyio
async def test_onboard_failure(tmp_path: Path) -> None:
    dispatcher = RPCDispatcher()

    class FailingTool:
        def apply(self) -> str:  # pragma: no cover - stub
            raise RuntimeError("boom")

    @dispatcher.register("onboard-project")
    async def onboard() -> OnboardingReport:  # pyright: ignore[reportUnusedFunction]
        return OnboardingReport(
            details=FailingTool().apply()
        )  # pragma: no cover - stub

    sock = tmp_path / "f.sock"
    server = await start_server(sock, dispatcher)
    async with server:
        try:
            reader, writer = await asyncio.wait_for(
                asyncio.open_unix_connection(str(sock)), timeout=5.0
            )
            writer.write(msjson.encode({"method": "onboard-project"}) + b"\n")
            await writer.drain()
            data = await asyncio.wait_for(reader.readline(), timeout=5.0)
            error = msjson.decode(data.rstrip())
            assert error.get("type") == "error"
        finally:
            writer.close()
            await writer.wait_closed()
    server.close()
    await server.wait_closed()


def test_create_serena_tool_with_invalid_tool_name(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """create_serena_tool should raise for unknown tools."""

    class ToolsMod:  # pragma: no cover - simple stub
        class OnboardingTool:  # pragma: no cover - simple stub
            def __init__(self, _: typ.Any) -> None:  # pragma: no cover - stub
                pass

    class PromptMod:  # pragma: no cover - simple stub
        class SerenaPromptFactory:  # pragma: no cover - simple stub
            def __call__(self) -> None:  # pragma: no cover - stub
                return None

    def fake_import(name: str) -> typ.Any:  # pragma: no cover - simple stub
        if name == "serena.tools.workflow_tools":
            return ToolsMod
        if name == "serena.prompt_factory":
            return PromptMod
        raise ModuleNotFoundError

    monkeypatch.setattr(serena_tools, "import_module", fake_import)
    serena_tools.clear_serena_imports()

    with pytest.raises(RuntimeError, match="NonExistentTool"):
        create_serena_tool("NonExistentTool")
    serena_tools.clear_serena_imports()
