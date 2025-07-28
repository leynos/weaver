from __future__ import annotations

# pyright: reportMissingImports=false  # Serena optional dependency
import asyncio
import getpass
import logging
import os
import resource
import sys
import tempfile
from importlib import import_module
from pathlib import Path

import msgspec as ms
import msgspec.json as msjson

from weaver_schemas.diagnostics import Diagnostic
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


def create_diagnostics_tool():
    """Return an instance of Serena's diagnostics tool."""
    try:
        wf_tools = import_module("serena.tools.workflow_tools")
        prompt_mod = import_module("serena.prompt_factory")
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dep
        msg = (
            "serena-agent is required for diagnostics; install it via "
            "'uv add serena-agent'."
        )
        raise RuntimeError(msg) from exc

    prompt_factory = prompt_mod.SerenaPromptFactory
    diag_tool = getattr(wf_tools, "ListDiagnosticsTool", None)
    if diag_tool is None:  # pragma: no cover - optional dep
        raise RuntimeError("ListDiagnosticsTool not found in serena")
    return diag_tool(_BareAgent(prompt_factory()))


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


def _get_rss_mb() -> float:
    """Return process memory usage in megabytes."""
    try:
        usage = resource.getrusage(resource.RUSAGE_SELF)
    except OSError:  # pragma: no cover - unsupported platform
        return 0.0

    rss = float(usage.ru_maxrss)
    if sys.platform == "darwin":
        return rss / (1024 * 1024)
    return rss / 1024


async def handle_project_status() -> ProjectStatus:
    """Return daemon PID, memory usage, and Serena availability."""

    try:
        import_module("serena")
        ready = True
    except ModuleNotFoundError:
        ready = False

    rss_mb = _get_rss_mb()

    return ProjectStatus(
        pid=os.getpid(),
        rss_mb=rss_mb,
        ready=ready,
        message="ok" if ready else "serena missing",
    )


async def handle_onboard_project() -> OnboardingReport:
    """Run the onboarding tool and return its report."""

    tool = create_onboarding_tool()
    try:
        details = await asyncio.to_thread(tool.apply)
    except Exception as exc:  # pragma: no cover - unexpected failures
        raise RuntimeError(f"Onboarding failed: {exc}") from exc
    return OnboardingReport(details=details)


async def handle_list_diagnostics(
    severity: str | None = None,
    files: list[str] | None = None,
) -> list[Diagnostic]:
    """List diagnostics, optionally filtered by severity and files."""

    tool = create_diagnostics_tool()
    try:
        data = await asyncio.to_thread(tool.list_diagnostics)
    except RuntimeError as exc:
        raise RuntimeError(f"Diagnostics failed: {exc}") from exc
    diags = [ms.convert(d, Diagnostic) for d in data]
    if severity:
        diags = [d for d in diags if d.severity == severity]
    if files:
        diags = [d for d in diags if d.location.file in files]
    return diags


async def main(socket_path: Path | None = None) -> None:
    dispatcher = RPCDispatcher()
    dispatcher.register("project-status")(handle_project_status)
    dispatcher.register("onboard-project")(handle_onboard_project)
    dispatcher.register("list-diagnostics")(handle_list_diagnostics)

    path = socket_path or default_socket_path()
    server = await start_server(path, dispatcher)
    async with server:
        await server.serve_forever()
