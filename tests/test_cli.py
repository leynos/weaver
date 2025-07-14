from typer.testing import CliRunner

from weaver.cli import app


def test_cli_hello() -> None:
    runner = CliRunner()
    result = runner.invoke(app, ["hello"])
    assert result.exit_code == 0
    assert "hello from Python" in result.stdout
