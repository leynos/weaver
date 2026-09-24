"""Contracts for Make's platform-specific development build routing.

The tests inspect evaluated Make recipes so conditional branches, injected
Cargo commands, and every Cargo invocation are covered. Temporary Makefiles
exercise both sides of the routing rule without changing the working tree.
"""

from __future__ import annotations

import os
import re
import shlex
import subprocess
from pathlib import Path

import pytest

REPOSITORY = Path(__file__).resolve().parents[2]
MAKEFILE = REPOSITORY / "Makefile"
DEV_FAST_CONFIG = "tools/dev-fast/config.toml"
CONFIG_ARGUMENT = f"--config {DEV_FAST_CONFIG}"
NEXTEST_CONFIG_PAIRS = (
    ("--config", "unstable.codegen-backend=true"),
    ("--config", 'profile.dev.codegen-backend="cranelift"'),
)
PROBE_CARGO = "probe-cargo"
PROBE_WHITAKER = "probe-whitaker"
CALLER_RUST_FLAG = "-Cdebuginfo=0"
MAKE_RUST_FLAG = "-Ctarget-cpu=native"
MAKE_RUSTDOC_FLAG = "--cfg docsrs"

PLATFORMS = (
    pytest.param(("Linux", True, True), id="linux-cranelift-mold"),
    pytest.param(("Darwin", True, False), id="macos-cranelift-native-linker"),
    pytest.param(("FreeBSD", False, False), id="freebsd-llvm-native-linker"),
)

DEBUG_TARGETS = (
    pytest.param(("build", (), ("build",)), id="build"),
    pytest.param(("dev-build", (), ("build",)), id="dev-build"),
    pytest.param(
        ("test", ("TEST_CMD=test",), ("test --workspace", "test --doc")),
        id="test-cargo",
    ),
    pytest.param(
        (
            "test",
            ("TEST_CMD=nextest run",),
            ("nextest run", "test --doc"),
        ),
        id="test-nextest-run",
    ),
    pytest.param(
        ("dev-test", ("TEST_CMD=test",), ("test --workspace", "test --doc")),
        id="dev-test-cargo",
    ),
    pytest.param(
        (
            "dev-test",
            ("TEST_CMD=nextest run",),
            ("nextest run", "test --doc"),
        ),
        id="dev-test-nextest-run",
    ),
    pytest.param(("lint", (), ("doc", "clippy")), id="lint-doc-and-clippy"),
    pytest.param(("typecheck", (), ("check",)), id="typecheck"),
)

FORBIDDEN_TARGETS = (
    pytest.param(("release", 1), id="release"),
    pytest.param(("clean", 1), id="clean"),
    pytest.param(("fmt", 1), id="fmt"),
    pytest.param(("check-fmt", 1), id="check-fmt"),
    pytest.param(("install", 2), id="install"),
)


def _dry_run(
    makefile: Path,
    target: str,
    host_os: str,
    assignments: tuple[str, ...] = (),
) -> list[str]:
    """Return one target's evaluated recipe lines with probe tools injected."""
    result = subprocess.run(
        [
            "make",
            "--dry-run",
            "--always-make",
            "--file",
            str(makefile),
            target,
            f"CARGO={PROBE_CARGO}",
            f"WHITAKER={PROBE_WHITAKER}",
            f"HOST_OS={host_os}",
            *assignments,
        ],
        cwd=REPOSITORY,
        env=os.environ | {"RUSTFLAGS": f"-D warnings {CALLER_RUST_FLAG}"},
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, (
        f"make --dry-run failed for {target!r} on {host_os}:\n"
        f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    )
    return [line for line in result.stdout.splitlines() if line.strip()]


def _cargo_lines(lines: list[str]) -> list[str]:
    """Return Cargo recipe lines and reject any invocation bypassing CARGO."""
    cargo_command = re.compile(r"(?<![A-Za-z0-9_-])cargo(?:\s|$)")
    invocations = [
        line for line in lines if PROBE_CARGO in line or cargo_command.search(line)
    ]
    assert all(PROBE_CARGO in line for line in invocations), (
        "Cargo invocations must use the injected CARGO command:\n"
        + "\n".join(line for line in invocations if PROBE_CARGO not in line)
    )
    return invocations


def _assert_effective_rustflags(
    line: str, target: str, host_os: str, expects_mold: bool
) -> None:
    """Verify warning and linker flags for one evaluated Cargo invocation."""
    flags_match = re.search(r'(?:^|\s)RUSTFLAGS="([^"]*)"', line)
    if flags_match is None:
        assert not expects_mold, (
            "Linux must make mold RUSTFLAGS explicit because they "
            f"override Cargo's target rustflags: {line}"
        )
        has_mold = False
    else:
        rustflags = flags_match.group(1)
        assert "-D warnings" in rustflags, f"warning denial was lost: {line}"
        assert CALLER_RUST_FLAG in rustflags, f"caller Rust flags were lost: {line}"
        has_mold = "-Clink-arg=-fuse-ld=mold" in rustflags
    assert has_mold is expects_mold, (
        f"{target} on {host_os} must "
        f"{'retain' if expects_mold else 'omit'} the mold linker flag: {line}"
    )
    if not expects_mold:
        assert "mold" not in line, f"{host_os} must retain its native linker: {line}"



def _assert_cargo_routing(
    lines: list[str],
    target: str,
    platform: tuple[str, bool, bool],
    expected_subcommands: tuple[str, ...],
) -> None:
    """Check fragment selection and effective linker flags on every invocation."""
    host_os, should_select_fragment, expects_mold = platform
    expected_count = len(expected_subcommands)
    invocations = _cargo_lines(lines)
    assert len(invocations) == expected_count, (
        f"{target} on {host_os} should emit {expected_count} Cargo invocations, "
        f"found {len(invocations)}:\n" + "\n".join(lines)
    )

    for line in invocations:
        _assert_effective_rustflags(line, target, host_os, expects_mold)

    matched_lines: list[str] = []
    for subcommand in expected_subcommands:
        match = next(
            (
                line
                for line in invocations
                if re.search(
                    rf"\b{re.escape(PROBE_CARGO)}\b.*\b{re.escape(subcommand)}\b",
                    line,
                )
            ),
            None,
        )
        assert match is not None, (
            f"{target} on {host_os} omitted its {subcommand!r} Cargo invocation"
        )
        _assert_subcommand_routing(match, subcommand, should_select_fragment)
        matched_lines.append(match)
    assert len(set(matched_lines)) == len(invocations), (
        f"{target} on {host_os} has unexpected or unmatched Cargo invocations: "
        f"{invocations}"
    )


def _assert_subcommand_routing(
    line: str, subcommand: str, should_select_fragment: bool
) -> None:
    """Check the interface through which this Cargo subcommand receives config."""
    tokens = shlex.split(line)
    if subcommand.startswith("nextest run"):
        nextest_index = tokens.index("nextest")
        forwarded_config = tuple(
            tuple(tokens[index : index + 2])
            for index in range(nextest_index + 2, len(tokens) - 1, 2)
        )
        assert tokens[nextest_index + 1] == "run", (
            f"nextest must run through its Cargo subcommand: {line}"
        )
        if should_select_fragment:
            assert forwarded_config[: len(NEXTEST_CONFIG_PAIRS)] == NEXTEST_CONFIG_PAIRS, (
                f"nextest must forward its configuration after `nextest run`: {line}"
            )
        else:
            assert not _has_nextest_config(line), (
                f"nextest must omit development configuration: {line}"
            )
        assert DEV_FAST_CONFIG not in tokens, (
            f"nextest cannot forward the development config file path: {line}"
        )
        assert "--workspace" in tokens, f"nextest lost workspace coverage: {line}"
        return

    has_fragment = DEV_FAST_CONFIG in tokens
    assert has_fragment is should_select_fragment, (
        f"{subcommand} must {'select' if should_select_fragment else 'omit'} "
        f"{CONFIG_ARGUMENT}: {line}"
    )
    if subcommand == "clippy" and should_select_fragment:
        cargo_index = tokens.index(PROBE_CARGO)
        assert tokens[cargo_index + 1 : cargo_index + 4] == [
            "clippy",
            "--config",
            DEV_FAST_CONFIG,
        ], f"Clippy must receive the fragment after its subcommand: {line}"


def _has_nextest_config(line: str) -> bool:
    """Return whether a command carries a development Nextest config pair."""
    tokens = shlex.split(line)
    return any(
        tokens[index : index + 2] == list(config_pair)
        for config_pair in NEXTEST_CONFIG_PAIRS
        for index in range(len(tokens) - 1)
    )


def _assert_debug_target(
    makefile: Path,
    platform: tuple[str, bool, bool],
    debug_target: tuple[str, tuple[str, ...], tuple[str, ...]],
) -> None:
    """Require every debug Cargo line to use the selected platform route."""
    host_os, uses_fragment, _ = platform
    target, assignments, expected_subcommands = debug_target
    if uses_fragment:
        assert (REPOSITORY / DEV_FAST_CONFIG).is_file(), (
            f"the explicitly selected development fragment is missing: {DEV_FAST_CONFIG}"
        )
    lines = _dry_run(makefile, target, host_os, assignments)
    _assert_cargo_routing(
        lines,
        target,
        platform,
        expected_subcommands=expected_subcommands,
    )


@pytest.mark.parametrize("platform", PLATFORMS)
@pytest.mark.parametrize("debug_target", DEBUG_TARGETS)
def test_every_debug_cargo_invocation_obeys_the_routing_matrix(
    platform: tuple[str, bool, bool],
    debug_target: tuple[str, tuple[str, ...], tuple[str, ...]],
) -> None:
    """Each evaluated debug Cargo line selects its backend and linker."""
    _assert_debug_target(MAKEFILE, platform, debug_target)


@pytest.mark.parametrize("platform", PLATFORMS)
@pytest.mark.parametrize("forbidden_target", FORBIDDEN_TARGETS)
def test_release_and_maintenance_targets_do_not_select_dev_fast(
    platform: tuple[str, bool, bool], forbidden_target: tuple[str, int]
) -> None:
    """Release, clean-up, formatting, and installation use ordinary Cargo."""
    host_os, _uses_fragment, _expects_mold = platform
    target, expected_count = forbidden_target
    lines = _dry_run(MAKEFILE, target, host_os)
    invocations = _cargo_lines(lines)
    assert len(invocations) == expected_count, (
        f"{target} should emit {expected_count} Cargo invocations, found "
        f"{len(invocations)}:\n" + "\n".join(lines)
    )
    assert all(CONFIG_ARGUMENT not in line for line in invocations), (
        f"{target} on {host_os} must not select {CONFIG_ARGUMENT}: {invocations}"
    )
    assert not any(_has_nextest_config(line) for line in invocations), (
        f"{target} on {host_os} must not select Nextest development config: "
        f"{invocations}"
    )


def _declared_make_targets(makefile: Path) -> set[str]:
    """Find simple explicit declarations for sensitive future targets."""
    return {
        target
        for line in makefile.read_text(encoding="utf-8").splitlines()
        if line and not line[0].isspace() and not line.startswith("#") and ":" in line
        for target in line.partition(":")[0].split()
        if re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", target)
    }


def test_any_verification_or_coverage_target_stays_on_the_default_backend() -> None:
    """Future verification and coverage targets cannot inherit dev-fast."""
    sensitive_targets = {
        target
        for target in _declared_make_targets(MAKEFILE)
        if re.search(r"coverage|cover|verify|verification|miri|kani|verus", target)
    }
    for target in sorted(sensitive_targets):
        lines = _dry_run(MAKEFILE, target, "Linux")
        invocations = _cargo_lines(lines)
        assert all(CONFIG_ARGUMENT not in line for line in invocations), (
            f"verification or coverage target {target} must not select "
            f"{CONFIG_ARGUMENT}: {invocations}"
        )
        assert not any(_has_nextest_config(line) for line in invocations), (
            f"verification or coverage target {target} must not select "
            f"Nextest development config: {invocations}"
        )


@pytest.mark.parametrize("platform", PLATFORMS)
def test_whitaker_never_receives_dev_fast_or_mold_flags(
    platform: tuple[str, bool, bool]
) -> None:
    """Whitaker keeps its separately pinned toolchain on every host."""
    host_os, _, _ = platform
    lines = _dry_run(MAKEFILE, "lint", host_os)
    whitaker_lines = [line for line in lines if PROBE_WHITAKER in line]
    assert len(whitaker_lines) == 1, (
        f"lint on {host_os} should invoke Whitaker once, found {len(whitaker_lines)}"
    )
    assert all(CONFIG_ARGUMENT not in line for line in whitaker_lines), whitaker_lines
    assert not any(_has_nextest_config(line) for line in whitaker_lines), whitaker_lines
    assert all("mold" not in line for line in whitaker_lines), whitaker_lines
