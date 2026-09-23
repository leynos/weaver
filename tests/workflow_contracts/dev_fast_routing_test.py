"""Contracts for Make's platform-specific development build routing.

The tests inspect evaluated Make recipes so conditional branches, injected
Cargo commands, and every Cargo invocation are covered. Temporary Makefiles
exercise both sides of the routing rule without changing the working tree.
"""

from __future__ import annotations

import os
import re
import subprocess
from pathlib import Path

import pytest

REPOSITORY = Path(__file__).resolve().parents[2]
MAKEFILE = REPOSITORY / "Makefile"
DEV_FAST_CONFIG = "tools/dev-fast/config.toml"
CONFIG_ARGUMENT = f"--config {DEV_FAST_CONFIG}"
PROBE_CARGO = "probe-cargo"
PROBE_WHITAKER = "probe-whitaker"
CALLER_RUST_FLAG = "-Cdebuginfo=0"
MAKE_RUST_FLAG = "-Ctarget-cpu=native"
MAKE_RUSTDOC_FLAG = "--cfg docsrs"

PLATFORMS = (
    pytest.param("Linux", True, True, id="linux-cranelift-mold"),
    pytest.param("Darwin", True, False, id="macos-cranelift-native-linker"),
    pytest.param("FreeBSD", False, False, id="freebsd-llvm-native-linker"),
)

DEBUG_TARGETS = (
    pytest.param("build", (), ("build",), id="build"),
    pytest.param("dev-build", (), ("build",), id="dev-build"),
    pytest.param(
        "test",
        ("TEST_CMD=test",),
        ("test --workspace", "test --doc"),
        id="test-cargo",
    ),
    pytest.param(
        "test",
        ("TEST_CMD=nextest run",),
        ("nextest run --workspace", "test --doc"),
        id="test-nextest-run",
    ),
    pytest.param(
        "dev-test",
        ("TEST_CMD=test",),
        ("test --workspace", "test --doc"),
        id="dev-test-cargo",
    ),
    pytest.param(
        "dev-test",
        ("TEST_CMD=nextest run",),
        ("nextest run --workspace", "test --doc"),
        id="dev-test-nextest-run",
    ),
    pytest.param("lint", (), ("doc", "clippy"), id="lint-doc-and-clippy"),
    pytest.param("typecheck", (), ("check",), id="typecheck"),
)

FORBIDDEN_TARGETS = (
    pytest.param("release", 1, id="release"),
    pytest.param("clean", 1, id="clean"),
    pytest.param("fmt", 1, id="fmt"),
    pytest.param("check-fmt", 1, id="check-fmt"),
    pytest.param("install", 2, id="install"),
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


def _assert_cargo_routing(
    lines: list[str],
    target: str,
    host_os: str,
    *,
    should_select_fragment: bool,
    expects_mold: bool,
    expected_count: int,
    expected_subcommands: tuple[str, ...],
) -> None:
    """Check fragment selection and effective linker flags on every invocation."""
    invocations = _cargo_lines(lines)
    assert len(invocations) == expected_count, (
        f"{target} on {host_os} should emit {expected_count} Cargo invocations, "
        f"found {len(invocations)}:\n" + "\n".join(lines)
    )

    for line in invocations:
        has_fragment = CONFIG_ARGUMENT in line
        assert has_fragment is should_select_fragment, (
            f"{target} on {host_os} must "
            f"{'select' if should_select_fragment else 'omit'} "
            f"{CONFIG_ARGUMENT}: {line}"
        )
        flags_match = re.search(r'(?:^|\s)RUSTFLAGS="([^"]*)"', line)
        if flags_match is None:
            assert not expects_mold, (
                f"Linux must make mold RUSTFLAGS explicit because they "
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
            assert "mold" not in line, (
                f"{host_os} must retain its native linker: {line}"
            )

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
        matched_lines.append(match)
    assert len(set(matched_lines)) == len(invocations), (
        f"{target} on {host_os} has unexpected or unmatched Cargo invocations: "
        f"{invocations}"
    )


def _assert_debug_target(
    makefile: Path,
    target: str,
    host_os: str,
    assignments: tuple[str, ...],
    expected_subcommands: tuple[str, ...],
    uses_fragment: bool,
    expects_mold: bool,
) -> None:
    """Require every debug Cargo line to use the selected platform route."""
    if uses_fragment:
        assert (REPOSITORY / DEV_FAST_CONFIG).is_file(), (
            f"the explicitly selected development fragment is missing: {DEV_FAST_CONFIG}"
        )
    lines = _dry_run(makefile, target, host_os, assignments)
    _assert_cargo_routing(
        lines,
        target,
        host_os,
        should_select_fragment=uses_fragment,
        expects_mold=expects_mold,
        expected_count=len(expected_subcommands),
        expected_subcommands=expected_subcommands,
    )


@pytest.mark.parametrize("host_os,uses_fragment,expects_mold", PLATFORMS)
@pytest.mark.parametrize(
    "target,assignments,expected_subcommands", DEBUG_TARGETS
)
def test_every_debug_cargo_invocation_obeys_the_routing_matrix(
    host_os: str,
    uses_fragment: bool,
    expects_mold: bool,
    target: str,
    assignments: tuple[str, ...],
    expected_subcommands: tuple[str, ...],
) -> None:
    """Each evaluated debug Cargo line selects its backend and linker."""
    _assert_debug_target(
        MAKEFILE,
        target,
        host_os,
        assignments,
        expected_subcommands,
        uses_fragment,
        expects_mold,
    )


@pytest.mark.parametrize("host_os,_uses_fragment,_expects_mold", PLATFORMS)
@pytest.mark.parametrize("target,expected_count", FORBIDDEN_TARGETS)
def test_release_and_maintenance_targets_do_not_select_dev_fast(
    host_os: str,
    _uses_fragment: bool,
    _expects_mold: bool,
    target: str,
    expected_count: int,
) -> None:
    """Release, clean-up, formatting, and installation use ordinary Cargo."""
    lines = _dry_run(MAKEFILE, target, host_os)
    invocations = _cargo_lines(lines)
    assert len(invocations) == expected_count, (
        f"{target} should emit {expected_count} Cargo invocations, found "
        f"{len(invocations)}:\n" + "\n".join(lines)
    )
    assert all(CONFIG_ARGUMENT not in line for line in invocations), (
        f"{target} on {host_os} must not select {CONFIG_ARGUMENT}: {invocations}"
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


@pytest.mark.parametrize("host_os,_uses_fragment,_expects_mold", PLATFORMS)
def test_whitaker_never_receives_dev_fast_or_mold_flags(
    host_os: str, _uses_fragment: bool, _expects_mold: bool
) -> None:
    """Whitaker keeps its separately pinned toolchain on every host."""
    lines = _dry_run(MAKEFILE, "lint", host_os)
    whitaker_lines = [line for line in lines if PROBE_WHITAKER in line]
    assert len(whitaker_lines) == 1, (
        f"lint on {host_os} should invoke Whitaker once, found {len(whitaker_lines)}"
    )
    assert all(CONFIG_ARGUMENT not in line for line in whitaker_lines), whitaker_lines
    assert all("mold" not in line for line in whitaker_lines), whitaker_lines


def test_contract_rejects_a_debug_invocation_without_the_fragment(tmp_path: Path) -> None:
    """Removing the fragment from a debug build makes the contract fail."""
    original = MAKEFILE.read_text(encoding="utf-8")
    mutated, replacements = re.subn(
        r"(\$\(CARGO\)\s+)\$\(DEV_FAST_CONFIG\)(\s+build\b)",
        r"\1\2",
        original,
        count=1,
    )
    assert replacements == 1, "could not create the debug-routing mutation"
    mutant_makefile = tmp_path / "Makefile"
    mutant_makefile.write_text(mutated, encoding="utf-8")

    with pytest.raises(AssertionError, match="must omit|must select"):
        _assert_debug_target(
            mutant_makefile, "build", "Linux", (), ("build",), True, True
        )


def test_contract_rejects_dev_fast_on_a_release_target(tmp_path: Path) -> None:
    """Adding the fragment to release makes the contract fail."""
    original = MAKEFILE.read_text(encoding="utf-8")
    mutated, replacements = re.subn(
        r"(\$\(CARGO\))(\s+build\b[^\n]*--release)",
        rf"\1 ${{DEV_FAST_CONFIG}}\2",
        original,
        count=1,
    )
    assert replacements == 1, "could not create the release-routing mutation"
    mutant_makefile = tmp_path / "Makefile"
    mutant_makefile.write_text(mutated, encoding="utf-8")

    lines = _dry_run(mutant_makefile, "release", "Linux")
    invocations = _cargo_lines(lines)
    assert invocations, "release mutation did not retain its Cargo invocation"
    with pytest.raises(AssertionError, match="must not select"):
        assert all(CONFIG_ARGUMENT not in line for line in invocations), (
            f"release on Linux must not select {CONFIG_ARGUMENT}: {invocations}"
        )


def test_all_runs_full_gates_sequentially() -> None:
    """The aggregate target does not overlap its repository gates."""
    lines = _dry_run(MAKEFILE, "all", "Linux")
    gate_commands = [
        line.strip()
        for line in lines
        if re.fullmatch(r"make\s+(?:check-fmt|lint|test|spelling)", line.strip())
    ]
    assert gate_commands == [
        "make check-fmt",
        "make lint",
        "make test",
        "make spelling",
    ], f"all must run its gates sequentially in policy order: {gate_commands}"


def test_custom_make_flags_keep_warning_denial_and_linker_selection() -> None:
    """Rust compiler flags survive warning and Linux linker additions."""
    lines = _dry_run(
        MAKEFILE,
        "typecheck",
        "Linux",
        (f"RUST_FLAGS={MAKE_RUST_FLAG}",),
    )
    invocations = _cargo_lines(lines)
    assert invocations, "typecheck emitted no Cargo invocations"
    for line in invocations:
        flags_match = re.search(r'(?:^|\s)RUSTFLAGS="([^"]*)"', line)
        assert flags_match is not None, f"Linux typecheck must make RUSTFLAGS explicit: {line}"
        rustflags = flags_match.group(1)
        assert "-D warnings" in rustflags, f"caller flags removed warning denial: {line}"
        assert CALLER_RUST_FLAG in rustflags, f"ambient Rust flags were lost: {line}"
        assert MAKE_RUST_FLAG in rustflags, f"Make caller Rust flags were lost: {line}"
        assert "-Clink-arg=-fuse-ld=mold" in rustflags, (
            f"Linux typecheck dropped the approved linker flag: {line}"
        )


def test_custom_rustdoc_flags_keep_warning_denial() -> None:
    """The caller's Rustdoc flags do not remove the warning denial."""
    lines = _dry_run(
        MAKEFILE,
        "lint",
        "Linux",
        (f"RUSTDOC_FLAGS={MAKE_RUSTDOC_FLAG}",),
    )
    doc_lines = [line for line in lines if f"{PROBE_CARGO} {CONFIG_ARGUMENT} doc" in line]
    assert len(doc_lines) == 1, f"expected one Cargo documentation line: {lines}"
    rustdoc_match = re.search(r'(?:^|\s)RUSTDOCFLAGS="([^"]*)"', doc_lines[0])
    assert rustdoc_match is not None, f"documentation flags were not explicit: {doc_lines[0]}"
    assert "-D warnings" in rustdoc_match.group(1), (
        f"caller Rustdoc flags removed warning denial: {doc_lines[0]}"
    )
    assert MAKE_RUSTDOC_FLAG in rustdoc_match.group(1), (
        f"Make caller Rustdoc flags were lost: {doc_lines[0]}"
    )
