"""Property contracts for workflow source-build shell matching."""

from __future__ import annotations

from hypothesis import given, strategies as st

from tool_install_test import CARGO_INSTALL, WHITAKER_CLONE, _matches_source_build

WORD_GAP = st.sampled_from((" ", "\t", " \\\n  ", "\\\n  ", "\n", " \n"))
URL_GAP = st.sampled_from(("", "\\\n", "\n"))


def _reference_commands(script: str) -> list[list[str]]:
    """Parse a bounded Bash-like command stream without using the matchers."""
    commands: list[list[str]] = []
    pending = ""
    for line in script.splitlines():
        pending += line
        if pending.endswith("\\"):
            pending = pending[:-1]
        else:
            commands.append(pending.split())
            pending = ""
    if pending:
        commands.append(pending.split())
    return commands


def _reference_cargo_install(script: str) -> bool:
    """Identify the Cargo source-build token sequence in reference commands."""
    return any(
        tokens[:2] == ["cargo", "install"]
        or (
            len(tokens) > 2
            and tokens[:3] == ["cargo", "+nightly", "install"]
        )
        for tokens in _reference_commands(script)
    )


def _reference_whitaker_clone(script: str) -> bool:
    """Identify the Whitaker source-clone token sequence in reference commands."""
    return any(
        tokens[:2] == ["git", "clone"]
        and "https://github.com/leynos/whitaker" in tokens[2:]
        for tokens in _reference_commands(script)
    )


@given(gap=WORD_GAP, toolchain=st.sampled_from(("", "+nightly")))
def test_cargo_detector_agrees_with_shell_token_reference(
    gap: str, toolchain: str
) -> None:
    """Cargo matching agrees with the bounded shell-token reference."""
    script = (
        f"cargo{gap}{toolchain}{gap}install cargo-dylint"
        if toolchain
        else f"cargo{gap}install cargo-dylint"
    )
    assert _matches_source_build(script, CARGO_INSTALL) is _reference_cargo_install(
        script
    )


@given(git_gap=WORD_GAP, clone_gap=WORD_GAP, path_gap=URL_GAP)
def test_whitaker_detector_agrees_with_shell_token_reference(
    git_gap: str, clone_gap: str, path_gap: str
) -> None:
    """Whitaker matching agrees with the bounded shell-token reference."""
    script = (
        f"git{git_gap}clone{clone_gap}"
        f"https://github.com/leynos/{path_gap}whitaker"
    )
    assert _matches_source_build(script, WHITAKER_CLONE) is _reference_whitaker_clone(
        script
    )


@given(gap=WORD_GAP, command=st.sampled_from(("cargo build", "git status")))
def test_source_build_detectors_reject_unrelated_shell_commands(
    gap: str, command: str
) -> None:
    """Unrelated commands cannot become source builds through continuation."""
    script = f"{command}{gap}https://github.com/leynos/whitaker"
    assert _matches_source_build(script, CARGO_INSTALL) is _reference_cargo_install(
        script
    )
    assert _matches_source_build(script, WHITAKER_CLONE) is _reference_whitaker_clone(
        script
    )
