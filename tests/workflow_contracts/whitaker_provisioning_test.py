"""Contract tests for how CI provisions Whitaker (concordat QG-002).

The estate provisions Whitaker one way: the shared-actions ``install-whitaker``
action, pinned to a listed revision, with the lint suite left to the rolling
release. These tests hold ``ci.yml`` to that and reject the source-revision pin
Weaver used while whitaker#311 was open.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
INSTALL_ACTION = "leynos/shared-actions/.github/actions/install-whitaker@"
PINNED_REVISION = "6cec89bac47a21cf756d68d638a9a510998e57f8"
#: Text that marks a hand-rolled Whitaker install or a suite pin.
HAND_ROLLED_RE = re.compile(r"whitaker-installer|WHITAKER_REV")


def _steps(source: str) -> list[dict]:
    """Return the ``build-test`` steps of a workflow given as YAML text."""
    workflow = yaml.safe_load(source) or {}
    return list((workflow.get("jobs") or {}).get("build-test", {}).get("steps") or [])


def _is_pinned_install(step: dict) -> bool:
    """Report whether a step is the pinned action with ``cranelift: true``."""
    uses = str(step.get("uses", ""))
    inputs = step.get("with") or {}
    return uses == f"{INSTALL_ACTION}{PINNED_REVISION}" and inputs.get("cranelift") is True


def _provisions_before_lint(source: str) -> bool:
    """Report whether the pinned install precedes the first ``make lint`` step."""
    steps = _steps(source)
    install_at = next((n for n, step in enumerate(steps) if _is_pinned_install(step)), None)
    lint_at = next(
        (n for n, step in enumerate(steps) if "make lint" in str(step.get("run", ""))),
        None,
    )
    return install_at is not None and lint_at is not None and install_at < lint_at


def _installs_by_hand(source: str) -> bool:
    """Report whether any workflow step installs Whitaker by hand or pins the suite."""
    workflow = yaml.safe_load(source) or {}
    for job in (workflow.get("jobs") or {}).values():
        env_text = yaml.dump(job.get("env") or {})
        if HAND_ROLLED_RE.search(env_text):
            return True
        if any(HAND_ROLLED_RE.search(str(step.get("run", ""))) for step in job.get("steps") or []):
            return True
    return False


def _ci_source() -> str:
    """Return the committed CI workflow text."""
    return (ROOT / ".github" / "workflows" / "ci.yml").read_text("utf-8")


def test_ci_provisions_whitaker_through_the_pinned_action_before_linting() -> None:
    """``build-test`` runs the pinned action with Cranelift before ``make lint``."""
    assert _provisions_before_lint(_ci_source()), (
        f"build-test must run {INSTALL_ACTION}{PINNED_REVISION} with cranelift: true "
        "before make lint"
    )


def test_ci_has_no_hand_rolled_install_or_suite_pin() -> None:
    """No workflow installs Whitaker by hand or pins the suite revision."""
    assert not _installs_by_hand(_ci_source()), (
        "no job may run whitaker-installer or set WHITAKER_REV"
    )


_ACTION = f"{INSTALL_ACTION}{PINNED_REVISION}"


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
            f"jobs:\n  build-test:\n    steps:\n      - uses: {_ACTION}\n      - run: make lint\n",
        ),
    ],
)
def test_each_way_of_missing_the_install_fails_the_contract(case: str, source: str) -> None:
    """A missing, late, differently pinned or Cranelift-less install is refused."""
    assert not _provisions_before_lint(source), case


@pytest.mark.parametrize(
    "fragment",
    [
        "    env:\n      WHITAKER_REV: 2bc0c3fafb86914f14bd73461a511588841f0a34\n    steps: []\n",
        "    steps:\n      - run: cargo install whitaker-installer\n",
        "    steps:\n      - run: whitaker-installer --build-only --no-update\n",
    ],
    ids=["suite pin", "cargo install", "direct run"],
)
def test_hand_rolled_installs_and_suite_pins_are_reported(fragment: str) -> None:
    """The source-revision pin and any direct installer use are caught."""
    assert _installs_by_hand(f"jobs:\n  build-test:\n{fragment}")
