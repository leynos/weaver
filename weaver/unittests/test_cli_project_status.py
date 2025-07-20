from __future__ import annotations

import getpass
import typing as t

import anyio
from anyio import to_thread
from typer.testing import CliRunner

from weaver.cli import app
from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server

if t.TYPE_CHECKING:
    import pathlib

    import pytest
    from click.testing import Result


def test_cli_project_status(
    tmp_path: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    runner = CliRunner()
    monkeypatch.setenv("XDG_RUNTIME_DIR", str(tmp_path))
    monkeypatch.setattr(getpass, "getuser", lambda: "user")

    dispatcher = RPCDispatcher()

    @dispatcher.register("project-status")
    async def handler() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
        return ProjectStatus(message="ok")

    sock = tmp_path / "weaverd-user.sock"

    async def run() -> None:
        server = await start_server(sock, dispatcher)
        async with server:
            result: Result = await to_thread.run_sync(
                lambda: runner.invoke(app, ["project-status"])
            )
            assert result.exit_code == 0
            assert result.stdout.strip() == '{"message":"ok","type":"project-status"}'

    anyio.run(run)
