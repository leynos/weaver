import pathlib

from typer.testing import CliRunner

from weaver.cli import app


def test_cli_hello() -> None:
    runner = CliRunner()
    result = runner.invoke(app, ["hello"])
    assert result.exit_code == 0
    assert "hello from Python" in result.stdout


def test_cli_check_socket(tmp_path: pathlib.Path) -> None:
    runner = CliRunner()
    sock_path = tmp_path / "nope"
    result = runner.invoke(app, ["check-socket", str(sock_path)])
    assert result.exit_code == 0
    assert f"socket unavailable: {sock_path}" in result.stdout
