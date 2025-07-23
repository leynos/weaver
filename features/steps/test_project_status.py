import typing as t

from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from weaver.cli import app
from weaver_schemas.status import ProjectStatus
from weaverd.rpc import RPCDispatcher

scenarios("../project_status.feature")


@given("a temporary runtime dir", target_fixture="context")
def runtime_dir(runtime_dir: dict[str, t.Any]) -> dict[str, t.Any]:
    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("project-status")
        async def status() -> ProjectStatus:  # pragma: no cover - stub
            return ProjectStatus(message="ok")

    runtime_dir["register"](setup)
    return runtime_dir


@when("I invoke the project-status command")
def invoke(context: dict[str, t.Any]) -> None:
    runner = CliRunner()
    result = runner.invoke(app, ["project-status"])
    context["result"] = result


@then("the output includes a project status line")
def check(context: dict[str, t.Any]) -> None:
    result = context["result"]
    assert result.exit_code == 0
    assert '"message":"ok"' in result.stdout
