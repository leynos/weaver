from __future__ import annotations

# pyright: reportMissingImports=false  # Serena optional dependency
import asyncio
import getpass
import logging
import os
import resource
import sys
import tempfile
import typing as typ
from importlib import import_module
from pathlib import Path

import msgspec as ms
import msgspec.json as msjson

from weaver_schemas.diagnostics import Diagnostic
from weaver_schemas.error import SchemaError
from weaver_schemas.reports import OnboardingReport
from weaver_schemas.status import ProjectStatus

from .rpc import Handler, RPCDispatcher

logger = logging.getLogger(__name__)


class _BareAgent:
    """Minimal agent providing only the prompt factory."""

    def __init__(self, prompt_factory) -> None:
        self.prompt_factory = prompt_factory


HANDLERS: list[tuple[str, Handler]] = []


def rpc_handler(name: str) -> typ.Callable[[Handler], Handler]:
    """Register ``func`` as an RPC handler with ``name``."""

    def decorator(func: Handler) -> Handler:
        if any(existing == name for existing, _ in HANDLERS):
            raise ValueError(f"handler '{name}' already registered")
        HANDLERS.append((name, func))
        return func

    return decorator


def _load_serena_tool(tool_attr: str):
    """Return the requested Serena tool and prompt factory.

    Raises:
        RuntimeError: if the ``serena-agent`` package is missing or the tool
            attribute cannot be imported.
    """
    try:
        wf_tools = import_module("serena.tools.workflow_tools")
        prompt_mod = import_module("serena.prompt_factory")
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dep
        msg = "serena-agent is required; install it via 'uv add serena-agent'."
        raise RuntimeError(msg) from exc

    tool_cls = getattr(wf_tools, tool_attr, None)
    if tool_cls is None:  # pragma: no cover - optional dep
        raise RuntimeError(f"{tool_attr} not found in serena")

    return tool_cls, prompt_mod.SerenaPromptFactory


def create_onboarding_tool():
    """Return an instance of Serena's onboarding tool."""

    tool_cls, prompt_factory = _load_serena_tool("OnboardingTool")
    return tool_cls(_BareAgent(prompt_factory()))


def create_diagnostics_tool():
    """Return an instance of Serena's diagnostics tool."""

    tool_cls, prompt_factory = _load_serena_tool("ListDiagnosticsTool")
    return tool_cls(_BareAgent(prompt_factory()))


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
                results = dispatcher.handle(data.rstrip())
            except Exception as exc:  # pragma: no cover - fallback
                if isinstance(exc, (asyncio.CancelledError, KeyboardInterrupt)):  # noqa: UP038
                    raise
                logger.exception("Unhandled RPC error")

                async def _err(error: Exception) -> typ.AsyncIterator[bytes]:
                    yield msjson.encode(SchemaError(message=str(error)))

                results = _err(exc)
            async for chunk in results:
                writer.write(chunk + b"\n")
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


@rpc_handler("project-status")
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


@rpc_handler("onboard-project")
async def handle_onboard_project() -> OnboardingReport:
    """Run the onboarding tool and return its report."""

    tool = create_onboarding_tool()
    try:
        details = await asyncio.to_thread(tool.apply)
    except Exception as exc:  # pragma: no cover - unexpected failures
        raise RuntimeError(f"Onboarding failed: {exc}") from exc
    return OnboardingReport(details=details)


@rpc_handler("list-diagnostics")
async def handle_list_diagnostics(
    severity: str | None = None,
    files: list[str] | None = None,
) -> typ.AsyncIterator[Diagnostic]:
    """Yield diagnostics, optionally filtered by severity and files."""

    # Normalise filters to make comparisons case- and path-insensitive
    norm_severity = severity.lower() if severity else None
    norm_files = {os.path.normpath(f).lower() for f in files} if files else None

    tool = create_diagnostics_tool()
    try:
        data = await asyncio.to_thread(tool.list_diagnostics)
    except RuntimeError as exc:
        raise RuntimeError(f"Diagnostics failed: {exc}") from exc
    for item in data:
        diag = ms.convert(item, Diagnostic)
        diag_severity = diag.severity.lower() if diag.severity else None
        diag_file = (
            os.path.normpath(diag.location.file).lower()
            if diag.location and diag.location.file
            else None
        )
        if norm_severity and diag_severity != norm_severity:
            continue
        if norm_files and diag_file not in norm_files:
            continue
        yield diag


async def main(socket_path: Path | None = None) -> None:
    dispatcher = RPCDispatcher()
    for name, func in HANDLERS:
        dispatcher.register(name)(func)

    path = socket_path or default_socket_path()
    server = await start_server(path, dispatcher)
    async with server:
        await server.serve_forever()
