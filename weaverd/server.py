from __future__ import annotations

# pyright: reportMissingImports=false
import asyncio
import getpass
import os
import tempfile
from importlib import import_module
from pathlib import Path

from msgspec import json

from weaver_schemas.error import SchemaError
from weaver_schemas.reports import OnboardingReport
from weaver_schemas.status import ProjectStatus

from .rpc import RPCDispatcher


class _BareAgent:
    """Minimal agent providing only the prompt factory."""

    def __init__(self) -> None:
        spf = import_module("serena.prompt_factory").SerenaPromptFactory  # pyright: ignore[reportAttributeAccessIssue]
        self.prompt_factory = spf()


def create_onboarding_tool():
    """Return an instance of Serena's onboarding tool."""
    onboarding_tool = import_module("serena.tools.workflow_tools").OnboardingTool  # pyright: ignore[reportAttributeAccessIssue]
    return onboarding_tool(_BareAgent())


def default_socket_path() -> Path:
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR", tempfile.gettempdir())
    user = getpass.getuser()
    return Path(runtime_dir) / f"weaverd-{user}.sock"


async def handle_client(
    reader: asyncio.StreamReader,
    writer: asyncio.StreamWriter,
    dispatcher: RPCDispatcher,
) -> None:
    try:
        while data := await reader.readline():
            try:
                response = await dispatcher.handle(data.rstrip())
            except Exception as exc:  # noqa: BLE001 pragma: no cover - fallback
                response = json.encode(SchemaError(message=str(exc)))
            writer.write(response + b"\n")
            await writer.drain()
    finally:
        writer.close()
        await writer.wait_closed()


async def start_server(path: Path, dispatcher: RPCDispatcher) -> asyncio.AbstractServer:
    if path.exists():
        path.unlink()
    return await asyncio.start_unix_server(
        lambda r, w: handle_client(r, w, dispatcher), path=str(path)
    )


async def main(socket_path: Path | None = None) -> None:
    dispatcher = RPCDispatcher()
    onboarding_tool = create_onboarding_tool()

    @dispatcher.register("project-status")
    async def project_status() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(message="ok")

    @dispatcher.register("onboard-project")
    async def onboard_project() -> OnboardingReport:  # pyright: ignore[reportUnusedFunction]
        details = await asyncio.to_thread(onboarding_tool.apply)
        return OnboardingReport(details=details)

    path = socket_path or default_socket_path()
    server = await start_server(path, dispatcher)
    async with server:
        await server.serve_forever()
