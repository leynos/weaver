"""Contract tests for per-command target selection in the gate recipes.

Each cargo or Whitaker command resolves its own compilation target from the
arguments it passes: an explicit ``--target`` before any cargo ``--`` wins over
``CARGO_BUILD_TARGET``, and `mold` applies exactly when the host and that target
are Linux. A pooled rule would hand a command the target of another's flags, so
this oracle reads every emitted command and derives its own expectation.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import shlex
import subprocess
from pathlib import Path

import pytest
from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st

ROOT = Path(__file__).resolve().parents[2]
MOLD = "-Clink-arg=-fuse-ld=mold"
TARGETS = ["test", "typecheck", "lint", "build"]
TRIPLES = [
    ("x86_64-unknown-linux-gnu", True),
    ("aarch64-unknown-linux-musl", True),
    ("aarch64-apple-darwin", False),
    ("x86_64-pc-windows-msvc", False),
]


def _commands(target: str, overrides: tuple[str, ...]) -> list[str]:
    """Return the cargo and Whitaker commands ``make -n TARGET`` would run."""
    result = subprocess.run(
        ["make", "-n", "-B", "BUILD_HOST_OS=Linux", *overrides, target],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    lines = result.stdout.replace("\\\n", " ").splitlines()
    return [
        line
        for line in lines
        if re.search(r"\b(cargo|whitaker)\b", line) and not line.startswith("echo")
    ]


def _effective_target(line: str, environment: str | None) -> str | None:
    """Return the compilation target a command's own arguments select.

    The last ``--target`` wins, in either spelling. For cargo, arguments after
    ``--`` belong to the compiler or test binary and are ignored; for Whitaker,
    ``--`` introduces the cargo arguments, so they count.
    """
    words = shlex.split(line)
    start = next(
        i for i, word in enumerate(words) if Path(word).name in {"cargo", "whitaker"}
    )
    is_cargo = Path(words[start]).name == "cargo"
    chosen: str | None = None
    args = words[start + 1 :]
    for i, arg in enumerate(args):
        if arg == "--" and is_cargo:
            break
        if arg == "--target" and i + 1 < len(args):
            chosen = args[i + 1]
        elif arg.startswith("--target="):
            chosen = arg.removeprefix("--target=")
    return chosen or environment


def _expects_mold(target: str | None) -> bool:
    """Return whether `mold` applies for a Linux host and this target."""
    return target is None or "-linux-" in target or target == "host-tuple"


def _problems(overrides: tuple[str, ...], environment: str | None) -> list[str]:
    """Check every command of every development target against its own target."""
    problems = []
    for make_target in TARGETS:
        for line in _commands(make_target, overrides):
            effective = _effective_target(line, environment)
            assigned = re.search(r'(?<![A-Z])RUSTFLAGS="([^"]*)"', line)
            flags = assigned.group(1) if assigned else ""
            if (MOLD in flags) != _expects_mold(effective):
                problems.append(
                    f"`make {make_target}` on Linux, effective target {effective!r}: "
                    f"mold {'present' if MOLD in flags else 'absent'} in "
                    f"{flags!r} for `{line}`"
                )
    return problems


@pytest.mark.parametrize(
    ("overrides", "environment"),
    [
        ((), None),
        (("TEST_FLAGS=--target aarch64-apple-darwin",), None),
        (("CARGO_FLAGS=--target=aarch64-apple-darwin",), None),
        (("CLIPPY_FLAGS=--target aarch64-apple-darwin -- -D warnings",), None),
        ((), "aarch64-apple-darwin"),
        (("TEST_FLAGS=--target aarch64-unknown-linux-gnu",), "aarch64-apple-darwin"),
        (("TEST_FLAGS=-p x -- --target aarch64-apple-darwin",), None),
    ],
    ids=[
        "no target",
        "test flags: macOS",
        "cargo flags: macOS",
        "clippy flags: macOS",
        "environment: macOS",
        "explicit Linux beats environment macOS",
        "after the cargo separator is ignored",
    ],
)
def test_each_command_follows_its_own_target(
    overrides: tuple[str, ...], environment: str | None
) -> None:
    """A target in one command's arguments does not reach the others."""
    env = (f"CARGO_BUILD_TARGET={environment}",) if environment else ()
    problems = _problems(overrides + env, environment)
    assert problems == [], problems


@settings(
    max_examples=25,
    deadline=None,
    suppress_health_check=[HealthCheck.too_slow],
)
@given(
    variable=st.sampled_from(["TEST_FLAGS", "CARGO_FLAGS", "CLIPPY_FLAGS"]),
    spelling=st.sampled_from(["--target {}", "--target={}"]),
    explicit=st.one_of(st.none(), st.sampled_from(TRIPLES)),
    environment=st.one_of(st.none(), st.sampled_from(TRIPLES)),
)
def test_every_command_resolves_its_target_from_its_own_arguments(
    variable: str,
    spelling: str,
    explicit: tuple[str, bool] | None,
    environment: tuple[str, bool] | None,
) -> None:
    """Whichever variable carries ``--target``, each command is checked alone."""
    overrides = []
    if explicit is not None:
        overrides.append(f"{variable}={spelling.format(explicit[0])}")
    env_target = environment[0] if environment else None
    if env_target:
        overrides.append(f"CARGO_BUILD_TARGET={env_target}")
    assert _problems(tuple(overrides), env_target) == []
