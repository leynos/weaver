import typing as typ

import pytest
from pytest_bdd import given, parsers, scenarios, then, when
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
        def get_definition(self, file: str, line: int, char: int) -> list[Symbol]:
            loc = Location(
                file=file,
                range=Range(start=Position(line, char), end=Position(line, char + 1)),
            )
            return [Symbol(name="spam", kind="function", location=loc)]

    monkeypatch.setattr(server, "create_serena_tool", lambda _: StubTool())

    def setup(dispatcher: RPCDispatcher) -> None:
        @dispatcher.register("get-definition")
        async def handler(
            file: str, line: int, char: int
        ) -> typ.AsyncIterator[Symbol]:  # pragma: no cover - stub
            tool = typ.cast(
                typ.Any,  # noqa: TC006
                server.create_serena_tool(SerenaTool.GET_DEFINITION),
            )
            for sym in tool.get_definition(file=file, line=line, char=char):
                yield sym

    runtime_dir["register"](setup)
    return runtime_dir


_INVOKE_RE = (
    r"I invoke the get-definition command for file "
    r'"(?P<file>[^"]+)" line (?P<line>\d+) char (?P<char>\d+)'
)


@when(parsers.re(_INVOKE_RE))
def invoke(context: Context, file: str, line: str, char: str) -> None:
    runner = CliRunner()
    result = runner.invoke(app, ["get-definition", file, line, char])
    context["result"] = result


@then("the output includes a symbol line")
def check(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 0
    out = result.stdout.lower()
    assert "symbol" in out or "spam" in out
