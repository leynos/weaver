"""Regression cases for Make's development build routing contract."""

from __future__ import annotations

import re
from pathlib import Path

import pytest

from dev_fast_routing_test import (
    CALLER_RUST_FLAG,
    CONFIG_ARGUMENT,
    MAKEFILE,
    MAKE_RUSTDOC_FLAG,
    MAKE_RUST_FLAG,
    PROBE_CARGO,
    _assert_debug_target,
    _cargo_lines,
    _dry_run,
)

DebugRouteMutation = tuple[
    str, str, str, tuple[str, tuple[str, ...], tuple[str, ...]]
]


INVALID_DEBUG_ROUTE_MUTATIONS = (
    pytest.param(
        (
            r"(\$\(CARGO\)\s+)\$\(DEV_FAST_CONFIG\)(\s+build\b)",
            r"\1\2",
            "must omit|must select",
            ("build", (), ("build",)),
        ),
        id="build-without-fragment",
    ),
    pytest.param(
        (
            r"(\$\(TEST_CMD\))\s+\$\(DEV_FAST_NEXTEST_CONFIG\)",
            r"\1",
            "nextest must forward",
            (
                "test",
                ("TEST_CMD=nextest run",),
                ("nextest run", "test --doc"),
            ),
        ),
        id="nextest-without-forwarding",
    ),
    pytest.param(
        (
            r"(\$\(TEST_CMD\))\s+(\$\(DEV_FAST_NEXTEST_CONFIG\))",
            r"\2 \1",
            "after `nextest run`",
            (
                "test",
                ("TEST_CMD=nextest run",),
                ("nextest run", "test --doc"),
            ),
        ),
        id="nextest-before-subcommand",
    ),
    pytest.param(
        (
            r"(\$\(CARGO\))\s+clippy\s+(\$\(DEV_FAST_CONFIG\))",
            r"\1 \2 clippy",
            "Clippy must receive",
            ("lint", (), ("doc", "clippy")),
        ),
        id="clippy-before-subcommand",
    ),
)


@pytest.mark.parametrize("mutation", INVALID_DEBUG_ROUTE_MUTATIONS)
def test_contract_rejects_invalid_debug_configuration(
    tmp_path: Path, mutation: DebugRouteMutation
) -> None:
    """Each pre-dispatch or missing debug configuration route is rejected."""
    pattern, replacement, message, debug_target = mutation
    original = MAKEFILE.read_text(encoding="utf-8")
    mutated, replacements = re.subn(pattern, replacement, original, count=1)
    assert replacements == 1, f"could not create routing mutation for {pattern!r}"
    mutant_makefile = tmp_path / "Makefile"
    mutant_makefile.write_text(mutated, encoding="utf-8")

    with pytest.raises(AssertionError, match=message):
        _assert_debug_target(mutant_makefile, ("Linux", True, True), debug_target)



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
