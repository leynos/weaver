import collections.abc as cabc

import pytest
from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from features.types import Context
from weaver.cli import app
from weaver_schemas.primitives import Location, Position, Range
from weaver_schemas.references import Symbol
from weaverd import server
from weaverd.rpc import RPCDispatcher
from weaverd.serena_tools import SerenaTool

scenarios("../get_definition.feature")


@given("a temporary runtime dir", target_fixture="context")
def runtime_dir(runtime_dir: Context, monkeypatch: pytest.MonkeyPatch) -> Context:
    class StubTool:
        def get_definition(self, *, file: str, line: int, char: int) -> list[Symbol]:
            loc = Location(
                file=file,
                range=Range(start=Position(line, char), end=Position(line, char + 1)),
            )
            return [Symbol(name="foo", kind="function", location=loc)]

    monkeypatch.setattr(server, "create_serena_tool", lambda _: StubTool())

    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("get-definition")
        async def handler(
            file: str, line: int, char: int
        ) -> cabc.AsyncIterator[Symbol]:  # pragma: no cover - stub
            tool = server.create_serena_tool(SerenaTool.GET_DEFINITION)
            for sym in tool.get_definition(file=file, line=line, char=char):
                yield sym

    runtime_dir["register"](setup)
    return runtime_dir


@given("serena-agent is missing")
def missing_dep(monkeypatch: pytest.MonkeyPatch) -> None:
    def raise_error(_: SerenaTool) -> None:
        raise RuntimeError("serena-agent not found")

    monkeypatch.setattr(server, "create_serena_tool", raise_error)


@when("I invoke the get-definition command")
def invoke(context: Context) -> None:
    runner = CliRunner()
    result = runner.invoke(app, ["get-definition", "foo.py", "1", "0"])
    context["result"] = result


@then("the output includes a symbol line")
def check(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 0
    out = result.stdout
    assert '"symbol"' in out or '"foo"' in out


@then("the daemon is not ready")
def check_not_ready(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 1
    out = (result.stdout + result.stderr).lower()
    assert "serena" in out or "missing" in out
