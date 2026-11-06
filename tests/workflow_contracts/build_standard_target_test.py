"""Contract tests for target selection in the gate recipes.

Make reads the compilation target from ``CARGO_BUILD_TARGET`` alone (a
``--target`` in a flag variable is invisible to it), and `mold` applies exactly
when the host and that target are Linux; ``host-tuple`` is the host's own
triple and an Android triple is not Linux. Whitaker assigns its own flags and
takes neither standard flag, so only cargo commands are read here.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import pytest
from build_standard_env import controlled_environment

ROOT = Path(__file__).resolve().parents[2]
MOLD = "-Clink-arg=-fuse-ld=mold"
TARGETS = ["test", "typecheck", "lint", "build"]


def _commands(target: str, overrides: tuple[str, ...]) -> list[str]:
    """Return the cargo commands ``make -n TARGET`` would run."""
    result = subprocess.run(
        ["make", "-n", "-B", "BUILD_HOST_OS=Linux", *overrides, target],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
        env=controlled_environment(),
    )
    lines = result.stdout.replace("\\\n", " ").splitlines()
    return [
        line
        for line in lines
        if re.search(r"\bcargo\b", line)
        and "whitaker" not in line
        and not line.startswith("echo")
    ]


def _problems(environment: str | None) -> list[str]:
    """Check every cargo command of every development target."""
    problems = []
    overrides = (f"CARGO_BUILD_TARGET={environment}",) if environment else ()
    expects_mold = (
        environment is None
        or environment == "host-tuple"
        or ("-linux-" in environment and "-android" not in environment)
    )
    for make_target in TARGETS:
        commands = _commands(make_target, overrides)
        assert commands, f"`make {make_target}` runs no cargo command"
        for line in commands:
            assigned = re.search(r'(?<![A-Z])RUSTFLAGS="([^"]*)"', line)
            flags = assigned.group(1) if assigned else ""
            if (MOLD in flags) != expects_mold:
                problems.append(
                    f"`make {make_target}` on Linux, target {environment!r}: "
                    f"mold {'present' if MOLD in flags else 'absent'} in "
                    f"{flags!r} for `{line}`"
                )
    return problems


@pytest.mark.parametrize(
    "environment",
    [
        None,
        "host-tuple",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-musl",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "aarch64-linux-android",
    ],
)
def test_each_command_follows_the_build_target(environment: str | None) -> None:
    """`mold` is present exactly when the compilation target is Linux."""
    problems = _problems(environment)
    assert problems == [], problems
