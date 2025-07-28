import msgspec as ms
import pytest
from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from features.types import Context
from weaver import client
from weaver.cli import app
from weaver_schemas.diagnostics import Diagnostic
from weaver_schemas.primitives import Location, Position, Range
from weaverd import server
from weaverd.rpc import RPCDispatcher

scenarios("../list_diagnostics.feature")


@given("a temporary runtime dir", target_fixture="context")
def runtime_dir(runtime_dir: Context, monkeypatch: pytest.MonkeyPatch) -> Context:
    class StubTool:
        def list_diagnostics(self) -> list[Diagnostic]:
            return [
                Diagnostic(
                    location=Location(
                        file="foo.py",
                        range=Range(start=Position(1, 0), end=Position(1, 1)),
                    ),
                    severity="Error",
                    code="E1",
                    message="boom",
                )
            ]

    monkeypatch.setattr(server, "create_diagnostics_tool", lambda: StubTool())

    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("list-diagnostics")
        async def handler() -> list[Diagnostic]:  # pragma: no cover - stub
            tool = server.create_diagnostics_tool()
            return tool.list_diagnostics()

    runtime_dir["register"](setup)
    return runtime_dir


@given("the daemon is already running")
def daemon_running(context: Context) -> None:
    client.spawn_daemon(context["sock"])


@given("serena-agent is missing")
def missing_dep(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("WEAVER_TEST_MISSING_SERENA", "1")

    def raise_error() -> None:
        raise RuntimeError("serena-agent not found")

    monkeypatch.setattr(server, "create_diagnostics_tool", raise_error)


@given("the server returns malformed output")
def server_malformed(context: Context) -> None:
    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("list-diagnostics")
        async def malformed() -> ms.Raw:  # pragma: no cover - stub
            return ms.Raw(b"MALFORMED OUTPUT")

    context["register"](setup)


@when("I invoke the list-diagnostics command")
def invoke(context: Context) -> None:
    runner = CliRunner()
    result = runner.invoke(app, ["list-diagnostics"])
    context["result"] = result


@then("the output includes a diagnostic line")
def check(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 0
    out = result.stdout
    assert '"message": "boom"' in out or "diagnostic" in out


@then("the daemon is not ready")
def check_not_ready(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 1
    out = (result.stdout + result.stderr).lower()
    assert "serena" in out or "missing" in out


@then("the output is malformed")
def check_malformed(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 0
    assert "malformed output" in result.stdout.lower()
