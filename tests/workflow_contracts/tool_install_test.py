"""Contracts keeping CI's tool installs on prebuilt, pinned releases.

`build-test` used to compile two tools on every run. `cargo install
merman-cli` built Merman from crates.io, a median 2.6 minutes, and the
Whitaker step built `whitaker-installer` from a pinned git revision, cloned
the Whitaker source and compiled the lint suite with Cranelift, a median 1.9
minutes. Both now come from `leynos/shared-actions`: `install-nixie` fetches
Merman's checksum-verified release archive, and `install-whitaker` fetches
the prebuilt, digest-verified lint suite and Dylint tools.

The prohibition runs over every step in every workflow, so a source build
cannot come back under a different step name. The requirements then pin the
replacement down, because a prohibition alone is satisfied by deleting the
install and letting `make lint` or `make nixie` fail on a missing tool, or by
an action that quietly builds from source anyway:

- each install is a shared action pinned to a full commit SHA, so the code
  that runs is the code that was reviewed;
- `install-whitaker` asks for an installer at or above 0.2.7, the first to
  take the prebuilt Dylint path, and neither pins the lint suite nor turns off
  `ci-mode`, since either forces or permits a source build;
- each install runs before the step that needs its tool.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ

import pytest
from workflow_loader import repository_workflows as workflows

#: A `cargo install`, with or without a `+toolchain` override. Matched as text
#: because this is a prohibition: a false match fails loudly.
CARGO_INSTALL: typ.Final = re.compile(r"\bcargo\s+(?:\+\S+\s+)?install\b")

#: A clone of the Whitaker source, which only a source build needs.
WHITAKER_CLONE: typ.Final = re.compile(r"\bgit\s+clone\b[^\n]*leynos/whitaker")

#: A full commit SHA, the only pin that names one immutable revision.
COMMIT_SHA: typ.Final = re.compile(r"^[0-9a-f]{40}$")

#: The first installer release that takes the prebuilt Dylint path.
MINIMUM_INSTALLER: typ.Final = (0, 2, 7)

LANE: typ.Final = ("ci.yml", "build-test")
NIXIE_ACTION: typ.Final = "leynos/shared-actions/.github/actions/install-nixie"
WHITAKER_ACTION: typ.Final = "leynos/shared-actions/.github/actions/install-whitaker"


def _steps(workflow: str, job: str) -> list[dict[str, object]]:
    """Return one job's steps."""
    definition = workflows()[workflow]["jobs"][job]
    assert isinstance(definition, dict), f"{workflow}:{job} is not a job mapping"
    steps = definition.get("steps", [])
    assert isinstance(steps, list), f"{workflow}:{job} steps must be a list"
    return [step for step in steps if isinstance(step, dict)]


def _every_step() -> list[tuple[str, str, dict[str, object]]]:
    """Return ``(workflow, job, step)`` for every step in every workflow."""
    return [
        (workflow, job, step)
        for workflow, document in workflows().items()
        for job, definition in _mapping(document.get("jobs")).items()
        for step in _mapping(definition).get("steps", []) or []
        if isinstance(step, dict)
    ]


def _mapping(value: object) -> dict[str, typ.Any]:
    """Return `value` when it is a mapping, else an empty one."""
    return value if isinstance(value, dict) else {}


def _action_step(action: str) -> tuple[int, dict[str, object]]:
    """Return the index and step in the lane that uses `action`, exactly once."""
    matches = [
        (index, step)
        for index, step in enumerate(_steps(*LANE))
        if str(step.get("uses", "")).partition("@")[0] == action
    ]
    assert len(matches) == 1, (
        f"{LANE[0]}:{LANE[1]} must use {action} exactly once, found {len(matches)}"
    )
    return matches[0]


def _run_index(command: str) -> int:
    """Return the index of the lane step whose whole command is `command`."""
    matches = [
        index
        for index, step in enumerate(_steps(*LANE))
        if str(step.get("run", "")).strip() == command
    ]
    assert len(matches) == 1, (
        f"{LANE[0]}:{LANE[1]} must run `{command}` as exactly one step's whole "
        f"command, found {len(matches)}"
    )
    return matches[0]


def _version(text: str) -> tuple[int, ...]:
    """Parse a dotted numeric version, refusing anything else."""
    assert re.fullmatch(r"\d+\.\d+\.\d+", text), f"{text!r} is not MAJOR.MINOR.PATCH"
    return tuple(int(part) for part in text.split("."))


def test_no_step_compiles_a_tool_from_source() -> None:
    """No workflow step runs `cargo install` or clones the Whitaker source."""
    offenders = [
        f"{workflow}:{job} step {step.get('name', '')!r}"
        for workflow, job, step in _every_step()
        if CARGO_INSTALL.search(str(step.get("run", "")))
        or WHITAKER_CLONE.search(str(step.get("run", "")))
    ]
    assert not offenders, (
        "these steps build a tool from source instead of installing a "
        f"prebuilt release: {', '.join(offenders)}"
    )


@pytest.mark.parametrize("action", [NIXIE_ACTION, WHITAKER_ACTION])
def test_each_install_is_a_shared_action_pinned_to_a_commit(action: str) -> None:
    """The install is pinned to a full SHA, so the reviewed code is what runs."""
    _, step = _action_step(action)
    ref = str(step["uses"]).partition("@")[2]
    assert COMMIT_SHA.fullmatch(ref), (
        f"{action} must be pinned to a commit SHA, not {ref!r}"
    )


def test_merman_comes_from_the_verified_release() -> None:
    """`install-nixie` installs the Merman release it holds a checksum for."""
    _, step = _action_step(NIXIE_ACTION)
    inputs = step.get("with", {})
    assert isinstance(inputs, dict), "install-nixie must declare its inputs"
    assert inputs.get("merman-version") == "0.7.0", (
        "install-nixie must install Merman 0.7.0, the release whose archive "
        f"checksum the action pins; got {inputs.get('merman-version')!r}"
    )


def test_whitaker_takes_the_prebuilt_path() -> None:
    """The installer is new enough, and nothing forces or permits a source build."""
    _, step = _action_step(WHITAKER_ACTION)
    inputs = step.get("with", {})
    assert isinstance(inputs, dict), "install-whitaker must declare its inputs"
    version = str(inputs.get("installer-version", ""))
    assert _version(version) >= MINIMUM_INSTALLER, (
        f"installer-version {version} predates the prebuilt Dylint path "
        f"({'.'.join(map(str, MINIMUM_INSTALLER))})"
    )
    assert not inputs.get("suite-version"), (
        "suite-version pins the lint suite, which forces a source build"
    )
    assert str(inputs.get("ci-mode", "true")).lower() == "true", (
        "ci-mode must stay on, so a fallback to a source build fails the step"
    )
    assert str(inputs.get("allow-suite-pin", "false")).lower() == "false", (
        "allow-suite-pin permits the source build ci-mode exists to refuse"
    )


@pytest.mark.parametrize(
    ("action", "command"),
    [(NIXIE_ACTION, "make nixie"), (WHITAKER_ACTION, "make lint")],
)
def test_each_install_runs_before_the_step_that_needs_it(
    action: str, command: str
) -> None:
    """The tool is on the runner before the gate that calls it."""
    install, _ = _action_step(action)
    assert install < _run_index(command), (
        f"{action} must run before `{command}` in {LANE[0]}:{LANE[1]}"
    )
