"""Integration-contract tests for the spelling-policy rollout.

This module verifies Python syntax and the Makefile generated-configuration
drift contract. Run it with pytest from the repository root.
"""

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
    spelling_config_recipe = makefile.split("spelling-config:", maxsplit=1)[1].split(
        "\n\n", maxsplit=1
    )[0]
    required_commands = (
        "$(UV_ENV) $(UV) run scripts/generate_typos_config.py",
        "git ls-files --error-unmatch typos.toml >/dev/null",
        "git diff --exit-code -- typos.toml",
    )
    command_positions = tuple(
        spelling_config_recipe.find(command) for command in required_commands
    )
    assert all(position >= 0 for position in command_positions), (
        "spelling-config no longer contains every generated-config safeguard"
    )
    assert command_positions == tuple(sorted(command_positions)), (
        "spelling-config safeguards no longer follow generation"
    )
