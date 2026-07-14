"""Integration-contract tests for the spelling-policy rollout."""

from __future__ import annotations

import ast
from pathlib import Path

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]


def test_rollout_integration_contract() -> None:
    """Scripts parse and the spelling gate checks indexed configuration drift."""
    for script in SCRIPT_DIRECTORY.glob("*.py"):
        ast.parse(
            script.read_text(encoding="utf-8"),
            filename=str(script),
            feature_version=(3, 13),
        )
    makefile = SCRIPT_DIRECTORY.parent.joinpath("Makefile").read_text(encoding="utf-8")
    assert "git ls-files --error-unmatch typos.toml >/dev/null" in makefile, (
        "spelling gate no longer requires an indexed generated config"
    )
    assert "git diff --exit-code -- typos.toml" in makefile, (
        "spelling gate no longer rejects generated-config drift"
    )
