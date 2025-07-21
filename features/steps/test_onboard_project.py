import asyncio
import multiprocessing as mp
import os
import sys
from pathlib import Path

from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from weaver import client
from weaver.cli import app
from weaver_schemas.reports import OnboardingReport
from weaverd.rpc import RPCDispatcher
from weaverd.server import create_onboarding_tool, start_server

sys.path.insert(0, "/root/git/serena-0.1.2/src")

scenarios("../onboard_project.feature")


@given("a temporary runtime dir", target_fixture="runtime_dir")
def runtime_dir(tmp_path: Path, monkeypatch):
    os.environ["XDG_RUNTIME_DIR"] = str(tmp_path)
    sock = client.discover_socket()

    processes: list[mp.Process] = []

    def wrapper(p: Path) -> mp.Process:
        def _run() -> None:
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:

                async def _serve() -> None:
                    dispatcher = RPCDispatcher()
                    tool = create_onboarding_tool()

                    @dispatcher.register("project-status")
                    async def status() -> (
                        OnboardingReport
                    ):  # pragma: no cover - test stub
                        return OnboardingReport(details="ok")

                    @dispatcher.register("onboard-project")
                    async def onboard() -> (
                        OnboardingReport
                    ):  # pragma: no cover - test stub
                        return OnboardingReport(details=tool.apply())

                    server = await start_server(p, dispatcher)
                    async with server:
                        await server.serve_forever()

                loop.run_until_complete(_serve())
            finally:
                loop.close()

        proc = mp.Process(target=_run)
        proc.start()
        processes.append(proc)
        return proc

    monkeypatch.setattr(client, "spawn_daemon", wrapper)
    return {"sock": sock, "processes": processes}


@when("I invoke the onboard-project command")
def invoke(runtime_dir: dict):
    runner = CliRunner()
    result = runner.invoke(app, ["onboard-project"])
    runtime_dir["result"] = result


@then("the output includes onboarding details")
def check(runtime_dir: dict):
    result = runtime_dir["result"]
    assert result.exit_code == 0
    assert "You are viewing the project" in result.stdout
    for proc in runtime_dir.get("processes", []):
        if proc and proc.is_alive():
            proc.terminate()
            try:
                proc.join(timeout=5)
            finally:
                if proc.is_alive():
                    proc.kill()
