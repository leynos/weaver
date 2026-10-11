"""Contract tests for how CI provisions Whitaker (concordat QG-002).

The estate provisions Whitaker one way: the shared-actions ``install-whitaker``
action, pinned to a listed revision, with the lint suite left to the rolling
release. These tests hold ``ci.yml`` to that and reject the source-revision pin
Weaver used while whitaker#311 was open. Changing the action pin therefore also
means changing ``PINNED_REVISION`` here, to another revision that concordat's
QG-002 rule lists.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ

import pytest
from workflow_loader import WORKFLOW_DIR, Document, load_workflow

INSTALL_ACTION: typ.Final = "leynos/shared-actions/.github/actions/install-whitaker@"
PINNED_REVISION: typ.Final = "6cec89bac47a21cf756d68d638a9a510998e57f8"
_ACTION: typ.Final = f"{INSTALL_ACTION}{PINNED_REVISION}"
#: Text that marks a hand-rolled Whitaker install or a suite pin.
HAND_ROLLED_RE: typ.Final = re.compile(r"whitaker-installer|WHITAKER_REV")


def _steps(workflow: Document) -> list[dict]:
    """Return the ``build-test`` steps of a parsed workflow."""
    jobs = typ.cast("dict", workflow.get("jobs") or {})
    job = typ.cast("dict", jobs.get("build-test") or {})
    return list(job.get("steps") or [])


def _is_pinned_install(step: dict) -> bool:
    """Report whether a step is the unconditional pinned action with Cranelift."""
    inputs = step.get("with") or {}
    return (
        step.get("uses") == _ACTION
        and "if" not in step
        and inputs.get("cranelift") is True
    )


def _first_index(steps: list[dict], matches: typ.Callable[[dict], bool]) -> int | None:
    """Return the index of the first step satisfying *matches*, if any."""
    for index, step in enumerate(steps):
        if matches(step):
            return index
    return None


def _provisions_before_lint(workflow: Document) -> bool:
    """Report whether the pinned install precedes the first ``make lint`` step."""
    steps = _steps(workflow)
    install_at = _first_index(steps, _is_pinned_install)
    lint_at = _first_index(steps, lambda step: "make lint" in str(step.get("run", "")))
    return install_at is not None and lint_at is not None and install_at < lint_at


def _scopes(workflow: Document) -> typ.Iterator[object]:
    """Yield every place a Whitaker install or suite pin could be written.

    That is the workflow's and each job's ``env`` mapping, and each step's
    ``env`` mapping and ``run`` text.
    """
    yield workflow.get("env") or {}
    for job in typ.cast("dict", workflow.get("jobs") or {}).values():
        yield job.get("env") or {}
        for step in job.get("steps") or []:
            yield step.get("env") or {}
            yield step.get("run") or ""


def _installs_by_hand(workflow: Document) -> bool:
    """Report whether any scope installs Whitaker by hand or pins the suite."""
    return any(HAND_ROLLED_RE.search(str(scope)) for scope in _scopes(workflow))


def _ci() -> Document:
    """Return the committed CI workflow, parsed strictly."""
    return load_workflow((WORKFLOW_DIR / "ci.yml").read_text("utf-8"))


def test_ci_provisions_whitaker_through_the_pinned_action_before_linting() -> None:
    """``build-test`` runs the pinned action with Cranelift before ``make lint``."""
    assert _provisions_before_lint(_ci()), (
        f"build-test must run {_ACTION} unconditionally with cranelift: true "
        "before make lint"
    )


def test_ci_has_no_hand_rolled_install_or_suite_pin() -> None:
    """No workflow scope installs Whitaker by hand or pins the suite revision."""
    assert not _installs_by_hand(_ci()), (
        "no env or run text may mention whitaker-installer or WHITAKER_REV"
    )


@pytest.mark.parametrize(
    ("case", "source"),
    [
        ("no install", "jobs:\n  build-test:\n    steps:\n      - run: make lint\n"),
        (
            "install after lint",
            "jobs:\n  build-test:\n    steps:\n      - run: make lint\n"
            f"      - uses: {_ACTION}\n        with:\n          cranelift: true\n",
        ),
        (
            "a different pin",
            f"jobs:\n  build-test:\n    steps:\n      - uses: {INSTALL_ACTION}{'0' * 40}\n"
            "        with:\n          cranelift: true\n      - run: make lint\n",
        ),
        (
            "no cranelift",
            f"jobs:\n  build-test:\n    steps:\n      - uses: {_ACTION}\n"
            "      - run: make lint\n",
        ),
        (
            "a skipped install",
            f"jobs:\n  build-test:\n    steps:\n      - uses: {_ACTION}\n        if: false\n"
            "        with:\n          cranelift: true\n      - run: make lint\n",
        ),
    ],
)
def test_each_way_of_missing_the_install_fails_the_contract(case: str, source: str) -> None:
    """A missing, late, skipped, differently pinned or Cranelift-less install fails."""
    assert not _provisions_before_lint(load_workflow(source)), case


@pytest.mark.parametrize(
    "source",
    [
        "env:\n  WHITAKER_REV: abc\njobs: {}\n",
        "jobs:\n  build-test:\n    env:\n      WHITAKER_REV: abc\n    steps: []\n",
        "jobs:\n  build-test:\n    steps:\n      - env:\n          WHITAKER_REV: abc\n",
        "jobs:\n  build-test:\n    steps:\n      - run: cargo install whitaker-installer\n",
        "jobs:\n  build-test:\n    steps:\n      - run: whitaker-installer --build-only\n",
    ],
    ids=["workflow env", "job env", "step env", "cargo install", "direct run"],
)
def test_hand_rolled_installs_and_suite_pins_are_reported_in_every_scope(source: str) -> None:
    """The source-revision pin and any direct installer use are caught anywhere."""
    assert _installs_by_hand(load_workflow(source))
