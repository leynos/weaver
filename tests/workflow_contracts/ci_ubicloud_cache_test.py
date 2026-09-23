"""Every Ubicloud lane that compiles must publish the cache proxy first.

Ubicloud runs a cache proxy on the runner's private network. The runner
exposes its URL and token to action steps only, so a shell step that starts an
sccache server cannot learn where the cache lives.
``export-ubicloud-cache-credentials`` republishes both through the job
environment and clears ``ACTIONS_CACHE_SERVICE_V2``, which sccache's backend
reads to select the v2 cache service that Ubicloud's proxy does not implement.

Without that step the lane still succeeds, which is the whole problem. sccache
falls through to GitHub's cache endpoint from an Ubicloud runner, so the
symptom is a slower build and metered egress rather than a failure anybody
notices.

Two orderings matter and both are asserted.

The credentials step must come before the step that configures sccache,
because sccache reads its cache configuration when its server starts and keeps
it for that server's life. Publishing afterwards changes nothing.

The step must carry a guard, because the action fails closed on a
GitHub-hosted runner rather than exporting that runner's endpoint under
Ubicloud's name. Lanes here can land on either: ``build-test`` has a fork
fallback, and ``build-and-package.yml`` takes its runner from the caller.

``runner.environment`` is the guard rather than a reading of the label. It is
the runtime fact, it resolves to ``self-hosted`` on Ubicloud, and it keeps
working when a lane's label moves.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import pytest
from workflow_loader import read_workflows

REPO_ROOT: typ.Final = Path(__file__).resolve().parents[2]
WORKFLOW_DIR: typ.Final = REPO_ROOT / ".github" / "workflows"

#: The action that publishes the proxy credentials, matched on its path so a
#: repin of the SHA does not need this file edited.
CREDENTIALS_ACTION: typ.Final = (
    "leynos/shared-actions/.github/actions/export-ubicloud-cache-credentials"
)

#: The actions that configure and start sccache, directly or through a nested
#: ``setup-rust``. Each must be preceded by the credentials step.
SCCACHE_CONSUMERS: typ.Final = (
    "leynos/shared-actions/.github/actions/setup-rust",
    "leynos/shared-actions/.github/actions/rust-build-release",
)

#: The only guard that is correct here, and why it is not a label test: the
#: action fails closed on a GitHub-hosted runner, and this is the runtime
#: fact rather than a reading of ``runs-on``.
EXPECTED_GUARD: typ.Final = "runner.environment == 'self-hosted'"

#: Jobs that compile on a runner which can be Ubicloud, by ``(workflow, job)``.
#: ``build-and-package.yml``'s job is here although its ``runs-on`` is an
#: input: the release workflow passes it an Ubicloud label for the two Linux
#: legs, so the lane reaches Ubicloud even though this file cannot see that.
COMPILING_JOBS: typ.Final = (
    ("ci.yml", "build-test"),
    ("coverage-main.yml", "coverage-upload"),
    ("build-and-package.yml", "build"),
)


@pytest.fixture(scope="session")
def workflows() -> dict[str, dict[str, object]]:
    """Read and parse every workflow once per session.

    Session scope rather than a module-level cache: pytest owns the lifetime,
    so the parse is visible in a fixture teardown or an ``--setup-show`` run
    instead of hiding in a decorator, and a test that needs a different tree
    can override it.

    The tree is read through ``workflow_loader``, which every workflow
    contract here shares: it refuses a duplicated mapping key, so a step
    declared twice cannot have its first half discarded unseen.

    Returns
    -------
    dict[str, dict[str, object]]
        Each workflow's file name mapped to its parsed document.
    """
    documents = read_workflows(WORKFLOW_DIR)
    assert documents, "the repository should define at least one workflow"
    return documents


def _steps(
    documents: dict[str, dict[str, object]],
    coordinate: tuple[str, str],
) -> list[dict[str, object]]:
    """Return one job's mapping steps, in order.

    Parameters
    ----------
    documents
        Every parsed workflow, keyed by file name.
    coordinate
        The job's ``(workflow, job id)`` coordinate.

    Returns
    -------
    list[dict[str, object]]
        The job's steps, in declaration order.
    """
    workflow, job_id = coordinate
    assert workflow in documents, f"{workflow} must exist"
    jobs = documents[workflow].get("jobs") or {}
    assert job_id in jobs, f"{workflow} must define a job {job_id!r}"
    steps = (jobs[job_id] or {}).get("steps") or []
    return [step for step in steps if isinstance(step, dict)]


def _indices(steps: list[dict[str, object]], action: str) -> list[int]:
    """Return the positions of every step using ``action``.

    Parameters
    ----------
    steps
        One job's steps, in order.
    action
        An action path, compared without its ``@sha`` suffix.

    Returns
    -------
    list[int]
        Positions in ``steps``, ascending.
    """
    return [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")).split("@")[0] == action
    ]


def case_id(value: object) -> str:
    """Render one parametrized coordinate.

    Parameters
    ----------
    value
        A ``(workflow, job id)`` coordinate, or any other value.

    Returns
    -------
    str
        The coordinate joined by ``-``, or ``str(value)``.
    """
    if isinstance(value, tuple):
        return "-".join(str(item) for item in value)
    return str(value)


@pytest.mark.parametrize("coordinate", COMPILING_JOBS, ids=case_id)
def test_the_lane_publishes_the_cache_proxy(
    workflows: dict[str, dict[str, object]],
    coordinate: tuple[str, str],
) -> None:
    """Scenario: a compiling lane runs on Ubicloud without the proxy.

    Invariant: every job that can compile on an Ubicloud runner declares
    exactly one credentials step. Omitting it does not fail the lane; sccache
    quietly reaches GitHub's cache endpoint instead, so nothing reports the
    loss.
    """
    found = _indices(_steps(workflows, coordinate), CREDENTIALS_ACTION)
    assert len(found) == 1, (
        f"{coordinate[0]}:{coordinate[1]} compiles on a runner that can be "
        f"Ubicloud, so it must declare exactly one {CREDENTIALS_ACTION} "
        f"step; found {len(found)}"
    )


@pytest.mark.parametrize("coordinate", COMPILING_JOBS, ids=case_id)
def test_the_proxy_is_published_before_sccache_is_configured(
    workflows: dict[str, dict[str, object]],
    coordinate: tuple[str, str],
) -> None:
    """Scenario: the credentials step is added after the Rust setup.

    Invariant: the credentials step precedes every step that configures or
    starts sccache. sccache reads its cache configuration when its server
    starts and keeps it for that server's life, so credentials published
    afterwards change nothing and the lane still looks correct.
    """
    steps = _steps(workflows, coordinate)
    credentials = _indices(steps, CREDENTIALS_ACTION)
    consumers = [
        index
        for action in SCCACHE_CONSUMERS
        for index in _indices(steps, action)
    ]
    assert consumers, (
        f"{coordinate[0]}:{coordinate[1]} should configure sccache through "
        f"one of {SCCACHE_CONSUMERS}; if that stopped being true this "
        "contract no longer describes the lane"
    )
    assert credentials, (
        f"{coordinate[0]}:{coordinate[1]} declares no {CREDENTIALS_ACTION} "
        "step, so there is nothing for the ordering to be right about"
    )
    assert credentials[0] < min(consumers), (
        f"{coordinate[0]}:{coordinate[1]} publishes the cache proxy at "
        f"{credentials} but configures sccache at {min(consumers)}; sccache "
        "reads its configuration when its server starts, so the credentials "
        "must come first"
    )


@pytest.mark.parametrize("coordinate", COMPILING_JOBS, ids=case_id)
def test_the_proxy_step_is_guarded_to_ubicloud(
    workflows: dict[str, dict[str, object]],
    coordinate: tuple[str, str],
) -> None:
    """Scenario: the credentials step runs on a GitHub-hosted runner.

    Invariant: the credentials step carries exactly the reviewed guard. The
    action fails closed off Ubicloud rather than exporting a GitHub-hosted
    endpoint under Ubicloud's name, so an unguarded step turns the fork arm
    and the macOS build legs red.
    """
    steps = _steps(workflows, coordinate)
    found = _indices(steps, CREDENTIALS_ACTION)
    # Stated here rather than leaned on from the presence test. Without it,
    # zero steps raise IndexError instead of failing with a reason, and two
    # steps silently guard only the first; either way this test stops being
    # independently meaningful.
    assert len(found) == 1, (
        f"{coordinate[0]}:{coordinate[1]} must declare exactly one "
        f"{CREDENTIALS_ACTION} step for this guard to describe; found "
        f"{len(found)}"
    )
    index = found[0]
    assert steps[index].get("if") == EXPECTED_GUARD, (
        f"{coordinate[0]}:{coordinate[1]}'s credentials step guards on "
        f"{steps[index].get('if')!r}, not the reviewed {EXPECTED_GUARD!r}"
    )
