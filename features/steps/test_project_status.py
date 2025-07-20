import asyncio
import multiprocessing as mp
import os
from pathlib import Path

from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from weaver import client
from weaver.cli import app
from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server

scenarios("../project_status.feature")


@given("a temporary runtime dir", target_fixture="runtime_dir")
def runtime_dir(tmp_path: Path, monkeypatch):
    os.environ["XDG_RUNTIME_DIR"] = str(tmp_path)
    sock = client.discover_socket()

    started: dict[str, mp.Process] = {}

    def wrapper(p: Path) -> mp.Process:
        def _run() -> None:
            async def _serve() -> None:
                dispatcher = RPCDispatcher()

                @dispatcher.register("project-status")
                async def status() -> ProjectStatus:  # pyright: ignore[reportUnusedFunction]
                    return ProjectStatus(message="ok")

                server = await start_server(p, dispatcher)
                async with server:
                    await server.serve_forever()

            asyncio.run(_serve())

        proc = mp.Process(target=_run)
        proc.start()
        started["proc"] = proc
        return proc

    monkeypatch.setattr(client, "spawn_daemon", wrapper)
    return {"sock": sock, "proc": started}


@when("I invoke the project-status command")
def invoke(runtime_dir: dict):
    runner = CliRunner()
    result = runner.invoke(app, ["project-status"])
    runtime_dir["result"] = result


@then("the output includes a project status line")
def check(runtime_dir: dict):
    result = runtime_dir["result"]
    assert result.exit_code == 0
    assert "project-status" in result.stdout
    proc = runtime_dir["proc"].get("proc")
    if proc:
        proc.terminate()
        proc.join()
