from __future__ import annotations

import dataclasses
import typing as typ

import pytest
import typer
from typer.testing import CliRunner

import weaver.cli as cli
from weaver.client import JSONObject

if typ.TYPE_CHECKING:
    import pathlib

RPCCall: typ.TypeAlias = typ.Callable[[str, JSONObject | None], object]
RunStub: typ.TypeAlias = typ.Callable[[RPCCall, str, JSONObject | None], None]


def make_run_stub(
    called: dict[str, object] | None = None,
    error: BaseException | None = None,
) -> RunStub:
    """Return an ``anyio.run`` stand-in.

    The stub avoids actual I/O by either recording invocation parameters or
    raising a provided error. Tests can reuse this helper to minimise boilerplate
    while validating how CLI commands interact with the RPC layer.
    """

    def _run(
        func: RPCCall,
        method: str,
        params: JSONObject | None = None,
    ) -> None:
        if called is not None:
            called.update({"func": func, "method": method, "params": params})
        if error is not None:
            raise error

    return _run


def test_cli_hello() -> None:
    runner = CliRunner()
    result = runner.invoke(cli.app, ["hello"])
    assert result.exit_code == 0
    assert "hello from Python" in result.stdout


def test_cli_check_socket(tmp_path: pathlib.Path) -> None:
    runner = CliRunner()
    sock_path = tmp_path / "nope"
    result = runner.invoke(cli.app, ["check-socket", str(sock_path)])
    assert result.exit_code == 0
    assert f"socket unavailable: {sock_path}" in result.stdout


def test_run_rpc_invokes_anyio(monkeypatch: pytest.MonkeyPatch) -> None:
    """Ensure _run_rpc delegates to anyio.run without I/O."""
    called: dict[str, object] = {}
    monkeypatch.setattr(cli.anyio, "run", make_run_stub(called))
    cli._run_rpc("test-method", {"a": 1})

    assert called == {
        "func": cli.rpc_call,
        "method": "test-method",
        "params": {"a": 1},
    }


def test_run_rpc_reports_error(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """_run_rpc should convert exceptions into user-friendly exits."""

    class FakeRunError(RuntimeError):
        """Test-only error to simulate RPC failure."""

    monkeypatch.setattr(cli.anyio, "run", make_run_stub(error=FakeRunError("boom")))

    with pytest.raises(typer.Exit):
        cli._run_rpc("broken")

    assert "Error: boom" in capsys.readouterr().err


@dataclasses.dataclass
class CLITestCase:
    cli_command: str
    rpc_method: str
    args: list[str]
    params: JSONObject | None


@pytest.mark.parametrize(
    "test_case",
    [
        CLITestCase("project-status", "project-status", [], None),
        CLITestCase("onboard-project", "onboard-project", [], None),
        CLITestCase(
            "get-definition",
            "get-definition",
            [__file__, "1", "2"],
            {"file": __file__, "line": 1, "char": 2},
        ),
        CLITestCase(
            "list-references",
            "list-references",
            [__file__, "1", "2"],
            {"file": __file__, "line": 1, "char": 2},
        ),
    ],
)
def test_cli_commands_use_run_rpc(
    test_case: CLITestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    """CLI commands use _run_rpc to contact the daemon."""
    called: dict[str, object] = {}
    monkeypatch.setattr(cli.anyio, "run", make_run_stub(called))

    runner = CliRunner()
    result = runner.invoke(cli.app, [test_case.cli_command, *test_case.args])

    assert result.exit_code == 0
    assert called == {
        "func": cli.rpc_call,
        "method": test_case.rpc_method,
        "params": test_case.params,
    }


def test_cli_list_references_include_definition(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """list-references forwards include-definition flag to RPC."""
    called: dict[str, object] = {}
    monkeypatch.setattr(cli.anyio, "run", make_run_stub(called))

    runner = CliRunner()
    result = runner.invoke(
        cli.app,
        ["list-references", "--include-definition", __file__, "1", "2"],
    )

    assert result.exit_code == 0
    assert called == {
        "func": cli.rpc_call,
        "method": "list-references",
        "params": {
            "file": __file__,
            "line": 1,
            "char": 2,
            "include_definition": True,
        },
    }


def test_cli_get_definition_handles_empty_response(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """get-definition exits cleanly when the RPC stream is empty."""
    monkeypatch.setattr(cli.anyio, "run", make_run_stub())

    runner = CliRunner()
    result = runner.invoke(cli.app, ["get-definition", __file__, "1", "2"])

    assert result.exit_code == 0
    assert result.stdout == ""


def test_cli_onboard_project_reports_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """onboard-project surfaces RPC errors."""

    class FakeRunError(RuntimeError):
        """Test-only error to simulate RPC failure."""

    monkeypatch.setattr(cli.anyio, "run", make_run_stub(error=FakeRunError("boom")))

    runner = CliRunner()
    result = runner.invoke(cli.app, ["onboard-project"])

    assert result.exit_code == 1
    assert "Error: boom" in result.stderr
