import sys

from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from weaver.cli import app
from weaver_schemas.reports import OnboardingReport
from weaverd.rpc import RPCDispatcher
from weaverd.server import create_onboarding_tool

# Ensure the Serena sources are discoverable for tests
sys.path.insert(0, "/root/git/serena-0.1.2/src")

scenarios("../onboard_project.feature")


@given("a temporary runtime dir", target_fixture="context")
def runtime_dir(runtime_dir: dict) -> dict:
    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("onboard-project")
        async def onboard() -> OnboardingReport:  # pragma: no cover - stub
            tool = create_onboarding_tool()
            return OnboardingReport(details=tool.apply())

    runtime_dir["set_handlers"](setup)
    return runtime_dir


@when("I invoke the onboard-project command")
def invoke(context: dict) -> None:
    runner = CliRunner()
    result = runner.invoke(app, ["onboard-project"])
    context["result"] = result


@then("the output includes onboarding details")
def check(context: dict) -> None:
    result = context["result"]
    assert result.exit_code == 0
    assert "You are viewing the project" in result.stdout
