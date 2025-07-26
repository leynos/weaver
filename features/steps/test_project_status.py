import os

import msgspec as ms
from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from features.types import Context
from weaver import client
from weaver.cli import app
from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher

scenarios("../project_status.feature")


@given("a temporary runtime dir", target_fixture="context")
def runtime_dir(runtime_dir: Context) -> Context:
    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("project-status")
        async def status() -> ProjectStatus:  # pragma: no cover - stub
            ready = "WEAVER_TEST_MISSING_SERENA" not in os.environ
            msg = "ok" if ready else "serena missing"
            return ProjectStatus(pid=321, rss_mb=1.0, ready=ready, message=msg)

    runtime_dir["register"](setup)
    return runtime_dir


@given("the daemon is already running")
def daemon_running(context: Context) -> None:
    client.spawn_daemon(context["sock"])


@given("serena-agent is missing")
def missing_dep(monkeypatch) -> None:
    monkeypatch.setenv("WEAVER_TEST_MISSING_SERENA", "1")


@given("the server returns malformed output")
def server_malformed(context: Context) -> None:
    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("project-status")
        async def malformed() -> ms.Raw:  # pragma: no cover - stub
            return ms.Raw(b"MALFORMED OUTPUT")

    context["register"](setup)


@when("I invoke the project-status command")
def invoke(context: Context) -> None:
    runner = CliRunner()
    result = runner.invoke(app, ["project-status"])
    context["result"] = result


@then("the output includes a project status line")
def check(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 0
    assert '"pid":321' in result.stdout


@then("the daemon is not ready")
def check_not_ready(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 0
    out = result.stdout.lower()
    assert '"ready": false' in out or "serena missing" in out


@then("the output is malformed")
def check_malformed(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 0
    assert "malformed output" in result.stdout.lower()
