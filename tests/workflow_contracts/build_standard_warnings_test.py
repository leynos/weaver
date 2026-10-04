"""Contract tests for the default warning policy of the gate recipes.

``build_standard_test.py`` proves the recipes keep a sentinel ``RUST_FLAGS``
override. This module proves the default: with no override, the compiler,
rustdoc and Clippy paths each deny warnings, so a recipe cannot quietly drop
the policy while still propagating whatever a caller supplies.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
DENY = "-D warnings"


def _dry_run(target: str) -> list[str]:
    """Return the commands ``make -n TARGET`` would run, continuations joined."""
    result = subprocess.run(
        ["make", "-n", "-B", "BUILD_HOST_OS=Linux", target],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.replace("\\\n", " ").splitlines()


def _cargo_lines(target: str) -> list[str]:
    """Return the lines of a dry run that run cargo or Whitaker."""
    return [
        line
        for line in _dry_run(target)
        if re.search(r"\b(cargo|whitaker)\b", line)
        and not line.lstrip().startswith("echo")
    ]


@pytest.mark.parametrize("target", ["test", "typecheck", "lint"])
def test_every_compiler_path_denies_warnings_by_default(target: str) -> None:
    """Each cargo or Whitaker command assigns a RUSTFLAGS that denies warnings."""
    lines = _cargo_lines(target)
    assert lines, f"`make {target}` runs no cargo command"
    for line in lines:
        assignments = re.findall(
            r'RUSTFLAGS="([^"]*)"', line.replace("RUSTDOCFLAGS", "DOC")
        )
        assert assignments, f"`make {target}` assigns no RUSTFLAGS: {line}"
        assert all(DENY in value for value in assignments), line


def test_rustdoc_and_clippy_deny_warnings_by_default() -> None:
    """Rustdoc takes its own flag, and Clippy takes the lint argument."""
    lines = _cargo_lines("lint")
    docs = [line for line in lines if " doc " in line]
    clippy = [line for line in lines if " clippy " in line]
    assert docs, "`make lint` runs no cargo doc"
    assert all(re.search(r'RUSTDOCFLAGS="[^"]*-D warnings', line) for line in docs), (
        docs
    )
    assert clippy, "`make lint` runs no clippy"
    assert all(re.search(r"--\s+[^\"]*-D warnings", line) for line in clippy), clippy
