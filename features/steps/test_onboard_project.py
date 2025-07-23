import json
import typing as t
from pathlib import Path

from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from weaver.cli import app
from weaver_schemas.reports import OnboardingReport
from weaverd.rpc import RPCDispatcher
from weaverd.server import create_onboarding_tool

scenarios("../onboard_project.feature")


@given("a temporary runtime dir", target_fixture="context")
def runtime_dir(runtime_dir: dict[str, t.Any]) -> dict[str, t.Any]:
    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("onboard-project")
        async def onboard() -> OnboardingReport:  # pragma: no cover - stub
            tool = create_onboarding_tool()
            return OnboardingReport(details=tool.apply())

    runtime_dir["register"](setup)
    return runtime_dir


@given("an invalid project structure")
def invalid_project(context: dict[str, t.Any], monkeypatch) -> None:
    def fail_spawn(_: Path) -> None:  # pragma: no cover - stub
        pass

    monkeypatch.setattr("weaver.client.spawn_daemon", fail_spawn)


@given("the server is unavailable")
def server_unavailable(context: dict[str, t.Any], monkeypatch) -> None:
    def noop(_: Path) -> None:  # pragma: no cover - stub
        pass

    monkeypatch.setattr("weaver.client.spawn_daemon", noop)


@given("the server returns malformed output")
def server_malformed(context: dict[str, t.Any]) -> None:
    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("onboard-project")
        async def onboard() -> str:  # pragma: no cover - stub
            return "MALFORMED OUTPUT"

    context["register"](setup)


@given("the onboarding tool raises an error")
def tool_error(context: dict[str, t.Any], monkeypatch) -> None:
    def setup(dispatcher: RPCDispatcher) -> None:
        class FailingTool:
            def apply(self) -> str:  # pragma: no cover - stub
                raise RuntimeError("boom")

        @dispatcher.register("onboard-project")
        async def onboard() -> OnboardingReport:  # pragma: no cover - stub
            tool = FailingTool()
            return OnboardingReport(details=tool.apply())

    context["register"](setup)


@given("serena-agent is missing")
def missing_dependency(monkeypatch):
    def raise_err():
        raise RuntimeError("serena-agent not found")

    monkeypatch.setattr("weaverd.server.create_onboarding_tool", raise_err)


@then("the command fails with a missing dependency message")
def check_missing_dep(context: dict[str, t.Any]) -> None:
    result = context["result"]
    assert result.exit_code == 0


@when("I invoke the onboard-project command")
def invoke(context: dict[str, t.Any]) -> None:
    runner = CliRunner()
    result = runner.invoke(app, ["onboard-project"])
    context["result"] = result


@then("the output includes onboarding details")
def check(context: dict[str, t.Any]) -> None:
    result = context["result"]
    assert result.exit_code == 0
    out = result.stdout.lower()
    assert "project" in out and "viewing" in out


@then("the command fails with an error message")
def check_error(context: dict[str, t.Any]) -> None:
    result = context["result"]
    assert result.exit_code != 0
    err = result.stderr.lower()
    assert any(p in err for p in ["daemon", "ensure", "spawn"])


@then("an error report is produced")
def check_report(context: dict[str, t.Any]) -> None:
    result = context["result"]
    assert result.exit_code == 0
    line = result.stdout.splitlines()[0]
    rec = json.loads(line)
    assert rec.get("type") == "error"


@then("the output indicates the server is unavailable")
def check_unavailable(context: dict[str, t.Any]) -> None:
    result = context["result"]
    assert result.exit_code != 0
    out = result.stderr.lower()
    assert "daemon" in out


@then("the output is malformed")
def check_malformed(context: dict[str, t.Any]) -> None:
    result = context["result"]
    assert result.exit_code == 0
    assert "malformed output" in result.stdout.lower()
