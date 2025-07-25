from __future__ import annotations

# pyright: reportMissingImports=false  # Serena optional dependency
import asyncio
import getpass
import logging
import os
import tempfile
from importlib import import_module
from pathlib import Path

import msgspec.json as msjson

from weaver_schemas.error import SchemaError
from weaver_schemas.reports import OnboardingReport
from weaver_schemas.status import ProjectStatus

from .rpc import RPCDispatcher

logger = logging.getLogger(__name__)


class _BareAgent:
    """Minimal agent providing only the prompt factory."""

    def __init__(self, prompt_factory) -> None:
        self.prompt_factory = prompt_factory


def create_onboarding_tool():
    """Return an instance of Serena's onboarding tool.

    Raises ``RuntimeError`` with a helpful message if ``serena-agent`` is not
    installed.
    """
    if os.environ.get("WEAVER_TEST_MISSING_SERENA"):
        raise RuntimeError("serena-agent not found")
    try:
        wf_tools = import_module("serena.tools.workflow_tools")
        prompt_mod = import_module("serena.prompt_factory")
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dep
        msg = (
            "serena-agent is required for onboarding; install it via "
            "'uv add serena-agent'."
        )
        raise RuntimeError(msg) from exc

    onboarding_tool = wf_tools.OnboardingTool
    prompt_factory = prompt_mod.SerenaPromptFactory
    return onboarding_tool(_BareAgent(prompt_factory()))


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
            except Exception as exc:  # pragma: no cover - fallback
                if isinstance(exc, (asyncio.CancelledError, KeyboardInterrupt)):  # noqa: UP038
                    raise
                logger.exception("Unhandled RPC error")
                response = msjson.encode(SchemaError(message=str(exc)))
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

    @dispatcher.register("project-status")
    async def project_status() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(message="ok")

    @dispatcher.register("onboard-project")
    async def onboard_project() -> OnboardingReport:  # pyright: ignore[reportUnusedFunction]
        tool = create_onboarding_tool()
        try:
            details = await asyncio.to_thread(tool.apply)
        except Exception as exc:  # pragma: no cover - unexpected failures
            raise RuntimeError(f"Onboarding failed: {exc}") from exc
        return OnboardingReport(details=details)

    path = socket_path or default_socket_path()
    server = await start_server(path, dispatcher)
    async with server:
        await server.serve_forever()
