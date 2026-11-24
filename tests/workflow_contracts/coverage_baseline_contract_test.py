"""Contract tests for the coverage ratchet's baseline path.

The ratchet compares this run's coverage against a baseline cached under
``baseline-rust-file``. The cache is keyed by that path, so every step that
runs the ratchet must name the same one. ``ci.yml`` checks against the
baseline on pull requests and ``coverage-main.yml`` writes it on main, so a
rename in one file alone would leave a pull request comparing against a
figure that main never wrote, and nothing would fail.

Four things are asserted. The selector must find exactly one ratchet step in
each workflow, because an empty selection would let the path checks pass
without examining anything. Both workflows must name the committed path. The
committed file must hold one percentage. And no step may name the retired
default path.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import pytest
from workflow_loader import load_workflow

REPO_ROOT: typ.Final = Path(__file__).resolve().parents[2]
WORKFLOW_DIR: typ.Final = REPO_ROOT / ".github" / "workflows"

#: The two workflows that run the ratchet.
WORKFLOWS: typ.Final = ("ci.yml", "coverage-main.yml")

#: The committed baseline, and the path both workflows must name for it.
BASELINE_FILE: typ.Final = ".coverage-baseline-v2.rust"

#: The default path this one replaced. No step may pass it.
RETIRED_BASELINE_FILE: typ.Final = ".coverage-baseline.rust"

#: Matched as a substring of ``uses`` so a repin of the SHA does not need
#: this file edited.
COVERAGE_ACTION: typ.Final = "generate-coverage"


def _steps(name: str) -> list[dict[object, object]]:
    """Return every step of every job in one workflow, in file order.

    A step that is not a mapping cannot carry a ``with`` block, so it is
    skipped rather than read.
    """
    document = load_workflow((WORKFLOW_DIR / name).read_text(encoding="utf-8"))
    jobs = document.get("jobs") or {}
    assert isinstance(jobs, dict), f"{name} should map job ids to jobs"
    steps: list[dict[object, object]] = []
    for job in jobs.values():
        if isinstance(job, dict):
            steps.extend(
                step for step in job.get("steps") or [] if isinstance(step, dict)
            )
    return steps


def _with(step: dict[object, object]) -> dict[object, object]:
    """Return a step's ``with`` mapping, or an empty one when it has none."""
    supplied = step.get("with")
    return supplied if isinstance(supplied, dict) else {}


def _ratchet_steps(name: str) -> list[dict[object, object]]:
    """Return the steps in one workflow that run the ratchet.

    A step is selected when it calls the coverage action and sets
    ``with-ratchet`` to the string ``'true'``. The quotes matter: an unquoted
    ``true`` parses as a boolean, is not selected, and so is caught by the
    guard test rather than passing unchecked.
    """
    return [
        step
        for step in _steps(name)
        if COVERAGE_ACTION in str(step.get("uses", ""))
        and _with(step).get("with-ratchet") == "true"
    ]


def _baseline_path(name: str) -> object:
    """Return the baseline path the one ratchet step in a workflow passes."""
    selected = _ratchet_steps(name)
    assert len(selected) == 1, (
        f"{name} should hold exactly one ratchet step; found {len(selected)}"
    )
    return _with(selected[0]).get("baseline-rust-file")


@pytest.mark.parametrize("name", WORKFLOWS)
def test_the_selector_finds_one_ratchet_step(name: str) -> None:
    """Scenario: the ratchet step is renamed, or its ``with`` changes shape.

    Invariant: each workflow selects exactly one step. Zero selected steps
    would make every path assertion vacuously true, and two would leave it
    unclear which path applies.
    """
    selected = _ratchet_steps(name)
    assert len(selected) == 1, (
        f"{name} should hold exactly one {COVERAGE_ACTION} step with "
        f"with-ratchet: 'true'; found {len(selected)}"
    )


@pytest.mark.parametrize("name", WORKFLOWS)
def test_the_ratchet_step_names_the_committed_baseline(name: str) -> None:
    """Scenario: a step is pointed at a new path and the committed file stays.

    Invariant: the ratchet step passes exactly the committed baseline path.
    A step naming any other path reads a cache entry that nothing writes.
    """
    declared = _baseline_path(name)
    assert declared == BASELINE_FILE, (
        f"{name}'s ratchet step must pass baseline-rust-file: {BASELINE_FILE}, "
        f"got {declared!r}"
    )


def test_the_workflows_share_one_baseline_path() -> None:
    """Scenario: one workflow is renamed and the other is not.

    Invariant: the main-push save and the pull-request restore name the same
    path, so they share one cache version.
    """
    declared = {name: _baseline_path(name) for name in WORKFLOWS}
    assert len(set(declared.values())) == 1, (
        "the ratchet workflows must share one baseline-rust-file so that the "
        f"save and the restore share a cache version; got {declared!r}"
    )


def test_the_committed_baseline_holds_one_percentage() -> None:
    """Scenario: the baseline is edited into a multi-line or malformed file.

    Invariant: the file is one line holding a number from 0 to 100 with at
    most one decimal place. A second line, or a value with more precision,
    would be read by the ratchet as something other than the figure recorded.
    """
    path = REPO_ROOT / BASELINE_FILE
    assert path.is_file(), f"{BASELINE_FILE} must be committed at the repository root"
    lines = path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1, f"{BASELINE_FILE} should hold one line; found {len(lines)}"
    line = lines[0]
    assert re.fullmatch(r"\d{1,3}(\.\d)?", line), (
        f"{BASELINE_FILE} should be a plain number with at most one decimal "
        f"place, got {line!r}"
    )
    value = float(line)
    assert 0 <= value <= 100, (
        f"{BASELINE_FILE} should hold a percentage from 0 to 100, got {line!r}"
    )


@pytest.mark.parametrize("name", WORKFLOWS)
def test_no_step_names_the_retired_baseline_path(name: str) -> None:
    """Scenario: a step is copied from before the baseline was renamed.

    Invariant: no step in the workflow passes the retired default path, so a
    stale spelling cannot be taken for the current baseline.
    """
    retired = [
        step.get("name", "<unnamed step>")
        for step in _steps(name)
        if _with(step).get("baseline-rust-file") == RETIRED_BASELINE_FILE
    ]
    assert not retired, (
        f"{name} passes the retired {RETIRED_BASELINE_FILE!r} in step(s) {retired}"
    )
