import json
import os
import sys
import typing as t
from pathlib import Path

from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from weaver.cli import app
from weaver_schemas.reports import OnboardingReport
from weaverd.rpc import RPCDispatcher
from weaverd.server import create_onboarding_tool

# Ensure the Serena sources are discoverable for tests
SERENA_VERSION = os.environ.get("SERENA_VERSION", "0.1.2")
default_dir = Path.home() / "git" / f"serena-{SERENA_VERSION}"
SERENA_DIR = Path(os.environ.get("SERENA_DIR", default_dir))
sys.path.insert(0, str(SERENA_DIR / "src"))

scenarios("../onboard_project.feature")


@given("a temporary runtime dir", target_fixture="context")
def runtime_dir(runtime_dir: dict[str, t.Any]) -> dict[str, t.Any]:
    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("onboard-project")
        async def onboard() -> OnboardingReport:  # pragma: no cover - stub
            tool = create_onboarding_tool()
            return OnboardingReport(details=tool.apply())

    runtime_dir["set_handlers"](setup)
    return runtime_dir


@given("an invalid project structure")
def invalid_project(context: dict[str, t.Any], monkeypatch) -> None:
    def fail_spawn(_: Path) -> None:  # pragma: no cover - stub
        pass

    monkeypatch.setattr("weaver.client.spawn_daemon", fail_spawn)


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

    context["set_handlers"](setup)


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
