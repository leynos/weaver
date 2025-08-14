import dataclasses
import pathlib
import typing as typ

import pytest
import typer
from typer.testing import CliRunner

import weaver.cli as cli


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

    def fake_run(func, method, params=None):
        # The helper should forward rpc_call and user arguments verbatim.
        called.update({"func": func, "method": method, "params": params})

    monkeypatch.setattr(cli.anyio, "run", fake_run)
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

    def fake_run(func, method, params=None):
        raise RuntimeError("boom")

    monkeypatch.setattr(cli.anyio, "run", fake_run)

    with pytest.raises(typer.Exit):
        cli._run_rpc("broken")

    assert "Error: boom" in capsys.readouterr().err


@dataclasses.dataclass
class CLITestCase:
    cli_command: str
    rpc_method: str
    args: list[str]
    params: dict[str, typ.Any] | None


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
    ],
)
def test_cli_commands_use_run_rpc(
    test_case: CLITestCase, monkeypatch: pytest.MonkeyPatch
) -> None:
    """CLI commands use _run_rpc to contact the daemon."""
    called: dict[str, object] = {}

    def fake_run(func, method, params=None):
        # Avoid network access while verifying parameters.
        called.update({"func": func, "method": method, "params": params})

    monkeypatch.setattr(cli.anyio, "run", fake_run)

    runner = CliRunner()
    result = runner.invoke(cli.app, [test_case.cli_command, *test_case.args])

    assert result.exit_code == 0
    assert called == {
        "func": cli.rpc_call,
        "method": test_case.rpc_method,
        "params": test_case.params,
    }


def test_cli_get_definition_handles_empty_response(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """get-definition exits cleanly when the RPC stream is empty."""

    def fake_run(func, method, params=None):
        # Simulate RPC call producing no output.
        return None

    monkeypatch.setattr(cli.anyio, "run", fake_run)

    runner = CliRunner()
    result = runner.invoke(cli.app, ["get-definition", __file__, "1", "2"])

    assert result.exit_code == 0
    assert result.stdout == ""


def test_cli_onboard_project_reports_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """onboard-project surfaces RPC errors."""

    def fake_run(func, method, params=None):
        raise RuntimeError("boom")

    monkeypatch.setattr(cli.anyio, "run", fake_run)

    runner = CliRunner()
    result = runner.invoke(cli.app, ["onboard-project"])

    assert result.exit_code == 1
    assert "Error: boom" in result.stderr
