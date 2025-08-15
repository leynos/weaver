import collections.abc as cabc
import json
from pathlib import Path

import pytest
from pytest_bdd import given, scenarios, then, when
from typer.testing import CliRunner

from features.types import Context
from weaver.cli import app
from weaver_schemas.primitives import Location, Position, Range
from weaver_schemas.references import Reference
from weaverd import server
from weaverd.rpc import RPCDispatcher
from weaverd.serena_tools import SerenaTool

scenarios("../list_references.feature")


def _configure_list_refs(
    runtime_dir: Context,
    monkeypatch: pytest.MonkeyPatch,
    list_refs_impl: cabc.Callable[..., list[Reference]],
) -> Context:
    """Register the production handler with a stubbed Serena tool."""

    class StubTool:
        def list_references(
            self,
            *,
            file: str,
            line: int,
            char: int,
            include_definition: bool = False,
        ) -> list[Reference]:
            return list_refs_impl(
                file, line, char, include_definition=include_definition
            )

    monkeypatch.setattr(server, "create_serena_tool", lambda _: StubTool())

    def setup(dispatcher: RPCDispatcher) -> None:
        dispatcher.register("list-references")(server.handle_list_references)

    runtime_dir["register"](setup)
    return runtime_dir


def _return_ref(
    file: str, line: int, char: int, *, include_definition: bool
) -> list[Reference]:
    loc = Location(
        file="def.py" if include_definition else "ref.py",
        range=Range(start=Position(line, char), end=Position(line, char + 1)),
    )
    return [Reference(location=loc)]


def _return_empty(
    file: str, line: int, char: int, *, include_definition: bool
) -> list[Reference]:
    return []


@given("a temporary runtime dir", target_fixture="context")
def runtime_dir(runtime_dir: Context, monkeypatch: pytest.MonkeyPatch) -> Context:
    return _configure_list_refs(runtime_dir, monkeypatch, _return_ref)


@given("a temporary runtime dir with no references", target_fixture="context")
def runtime_dir_empty(runtime_dir: Context, monkeypatch: pytest.MonkeyPatch) -> Context:
    return _configure_list_refs(runtime_dir, monkeypatch, _return_empty)


@given("serena-agent is missing")
def missing_dep(monkeypatch: pytest.MonkeyPatch) -> None:
    def raise_error(_: SerenaTool) -> None:
        raise RuntimeError("serena-agent not found")

    monkeypatch.setattr(server, "create_serena_tool", raise_error)


@when("I invoke the list-references command")
def invoke(context: Context, tmp_path: Path) -> None:
    file = tmp_path / "foo.py"
    file.write_text("pass")
    runner = CliRunner()
    try:
        result = runner.invoke(app, ["list-references", str(file), "1", "0"])
        context["result"] = result
    finally:
        file.unlink(missing_ok=True)


@when("I invoke the list-references command with include-definition")
def invoke_include(context: Context, tmp_path: Path) -> None:
    file = tmp_path / "foo.py"
    file.write_text("pass")
    runner = CliRunner()
    try:
        result = runner.invoke(
            app,
            ["list-references", "--include-definition", str(file), "1", "0"],
        )
        context["result"] = result
    finally:
        file.unlink(missing_ok=True)


@when("I invoke the list-references command with a missing file")
def invoke_missing(context: Context) -> None:
    Path("nope.py").unlink(missing_ok=True)
    runner = CliRunner()
    result = runner.invoke(app, ["list-references", "nope.py", "1", "0"])
    context["result"] = result


@then("the output includes a reference line")
def check_reference(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 0
    lines = [line for line in result.stdout.splitlines() if line.strip()]
    assert lines
    record = json.loads(lines[0])
    reference = record.get("reference", record)
    assert reference.get("type") == "reference"


@then("no output is produced")
def check_empty(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 0
    assert result.stdout.strip() == ""


@then("the output includes a definition reference")
def check_definition_reference(context: Context) -> None:
    result = context["result"]
    lines = [line for line in result.stdout.splitlines() if line.strip()]
    record = json.loads(lines[0])
    reference = record.get("reference", record)
    assert reference.get("location", {}).get("file") == "def.py"


@then("the daemon is not ready")
def check_not_ready(context: Context) -> None:
    result = context["result"]
    assert result.exit_code == 1
    out = (result.stdout + result.stderr).lower()
    assert "serena" in out or "missing" in out


@then("the command fails with a missing file error")
def check_missing_file(context: Context) -> None:
    result = context["result"]
    assert result.exit_code != 0
    out = (result.stdout + result.stderr).lower()
    assert "no such file" in out or "does not exist" in out
