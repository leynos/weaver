"""Contract tests for per-command target selection in the gate recipes.

Each cargo or Whitaker command resolves its own compilation target from the
arguments it passes: an explicit ``--target`` before any cargo ``--`` wins over
``CARGO_BUILD_TARGET``, and `mold` applies exactly when the host and that target
are Linux. A pooled rule would hand a command the target of another's flags, so
this oracle reads every emitted command and derives its own expectation.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import os
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


def _controlled_environment() -> dict[str, str]:
    """Return the process environment without anything that selects a target.

    A case that expects no environment target must not receive one the caller
    exported, so every variable the Makefile reads for the target or the flags
    is removed; each case sets what it needs on the command line.
    """
    dropped = {
        "CARGO_BUILD_TARGET",
        "TEST_FLAGS",
        "CARGO_FLAGS",
        "CLIPPY_FLAGS",
        "RUSTFLAGS",
        "MAKEFLAGS",
        "MFLAGS",
        "MAKELEVEL",
    }
    return {key: val for key, val in os.environ.items() if key not in dropped}


def _commands(target: str, overrides: tuple[str, ...]) -> list[str]:
    """Return the cargo and Whitaker commands ``make -n TARGET`` would run."""
    result = subprocess.run(
        ["make", "-n", "-B", "BUILD_HOST_OS=Linux", *overrides, target],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
        env=_controlled_environment(),
    )
    lines = result.stdout.replace("\\\n", " ").splitlines()
    return [
        line
        for line in lines
        if re.search(r"\b(cargo|whitaker)\b", line) and not line.startswith("echo")
    ]


def _command_words(line: str) -> tuple[bool, list[str]]:
    """Return whether a line runs cargo, and the words after the tool name."""
    words = shlex.split(line)
    start = next(
        i for i, word in enumerate(words) if Path(word).name in {"cargo", "whitaker"}
    )
    return Path(words[start]).name == "cargo", words[start + 1 :]


def _explicit_targets(args: list[str]) -> list[str]:
    """Return the ``--target`` values in the order the arguments give them."""
    targets = []
    for i, arg in enumerate(args):
        if arg == "--target" and i + 1 < len(args):
            targets.append(args[i + 1])
        elif arg.startswith("--target="):
            targets.append(arg.removeprefix("--target="))
    return targets


def _effective_targets(line: str, environment: str | None) -> list[str | None]:
    """Return the compilation targets a command's own arguments select.

    Each ``--target`` is one target, in either spelling, and Cargo builds for all
    of them. For cargo, arguments after ``--`` belong to the compiler or test
    binary and are ignored; for Whitaker, ``--`` introduces the cargo arguments,
    so they count. With none, the command takes the environment's target.
    """
    is_cargo, args = _command_words(line)
    if is_cargo and "--" in args:
        args = args[: args.index("--")]
    return list(_explicit_targets(args)) or [environment]


def _expects_mold(targets: list[str | None]) -> bool:
    """Return whether `mold` applies for a Linux host: every target is Linux."""
    return all(t is None or "-linux-" in t or t == "host-tuple" for t in targets)


def _problems(overrides: tuple[str, ...], environment: str | None) -> list[str]:
    """Check every command of every development target against its own target."""
    problems = []
    for make_target in TARGETS:
        commands = _commands(make_target, overrides)
        assert commands, f"`make {make_target}` runs no cargo or Whitaker command"
        for line in commands:
            effective = _effective_targets(line, environment)
            assigned = re.search(r'(?<![A-Z])RUSTFLAGS="([^"]*)"', line)
            flags = assigned.group(1) if assigned else ""
            if (MOLD in flags) != _expects_mold(effective):
                problems.append(
                    f"`make {make_target}` on Linux, effective targets {effective!r}: "
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
        (
            (
                "TEST_FLAGS=--target aarch64-apple-darwin",
                "CARGO_FLAGS=--target aarch64-unknown-linux-gnu",
            ),
            None,
        ),
        (
            (
                "CARGO_FLAGS=--target=aarch64-apple-darwin --target aarch64-unknown-linux-gnu",
            ),
            None,
        ),
        (
            (
                "CARGO_FLAGS=--target aarch64-unknown-linux-gnu --target=aarch64-apple-darwin",
            ),
            None,
        ),
        (
            (
                "TEST_FLAGS=--target aarch64-unknown-linux-gnu --target x86_64-unknown-linux-musl",
            ),
            None,
        ),
    ],
    ids=[
        "no target",
        "test flags: macOS",
        "cargo flags: macOS",
        "clippy flags: macOS",
        "environment: macOS",
        "explicit Linux beats environment macOS",
        "after the cargo separator is ignored",
        "conflicting variables each win for their own commands",
        "mixed spellings: macOS then Linux drops mold",
        "mixed spellings: Linux then macOS drops mold",
        "several Linux targets keep mold",
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
    flags=st.lists(
        st.tuples(
            st.sampled_from(["TEST_FLAGS", "CARGO_FLAGS", "CLIPPY_FLAGS"]),
            st.sampled_from(["--target {}", "--target={}"]),
            st.sampled_from(TRIPLES),
        ),
        max_size=3,
        unique_by=lambda item: item[0],
    ),
    environment=st.one_of(st.none(), st.sampled_from(TRIPLES)),
)
def test_every_command_resolves_its_target_from_its_own_arguments(
    flags: list[tuple[str, str, tuple[str, bool]]],
    environment: tuple[str, bool] | None,
) -> None:
    """Several variables may carry conflicting targets; each command is checked
    against its own arguments."""
    overrides = [
        f"{variable}={spelling.format(triple[0])}"
        for variable, spelling, triple in flags
    ]
    env_target = environment[0] if environment else None
    if env_target:
        overrides.append(f"CARGO_BUILD_TARGET={env_target}")
    assert _problems(tuple(overrides), env_target) == []
