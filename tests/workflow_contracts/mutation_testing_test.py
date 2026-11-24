"""Contract tests for the mutation-testing caller workflow.

The executable logic lives in the ``leynos/shared-actions`` reusable
workflow, which carries its own unit and integration tests; weaver's
caller is declarative configuration. These tests parse the caller with
PyYAML and assert the contract it must uphold: the caller references the
correct reusable workflow at a commit SHA (not a mutable branch or tag),
and retains its permissions, triggers, and `with:` configuration. Drift
(repointing the pin at a branch, widening permissions, or losing the
workspace paths, scaffolding excludes, or feature-flag configuration)
fails CI on the pull request rather than surfacing in a scheduled or
manual run. Dependabot owns the pinned SHA's value; these tests do not
assert which SHA is pinned.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

import yaml

WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / "mutation-testing.yml"
)

USES_RE = re.compile(
    r"^leynos/shared-actions/\.github/workflows/mutation-cargo\.yml@[0-9a-f]{40}$"
)

#: The exact caller configuration: the workspace lives under crates/
#: (no root src/), the end-to-end harness and proc-macro fixture crates
#: plus the feature-gated test-support modules are noise, and the CI
#: baseline runs --all-features across the whole workspace (so mutants
#: face the dependent crates' tests too).
EXPECTED_WITH = {
    "paths": "crates/",
    "exclude-globs": (
        "crates/weaver-e2e/**,"
        "crates/weaver-test-macros/**,"
        "crates/sempai-core/src/test_support.rs,"
        "crates/weaver-plugins/src/capability/test_support.rs"
    ),
    "extra-args": "--all-features --test-workspace=true",
}


def _load() -> dict[str, object]:
    """Parse the workflow file."""
    return yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))


def _triggers(workflow: dict[str, object]) -> dict[str, object]:
    """Return the ``on:`` mapping (PyYAML parses the bare key as True)."""
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the workflow must declare an on: mapping"
    return triggers


def _mutation_job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the single calling job."""
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the workflow must declare a jobs mapping"
    assert jobs, "the workflow must declare at least one job"
    assert list(jobs) == ["mutation"], (
        f"expected a single job named 'mutation', found {sorted(jobs)}"
    )
    return jobs["mutation"]


def test_uses_reference_is_pinned_to_a_commit_sha() -> None:
    """The job must call mutation-cargo.yml pinned to a 40-hex commit SHA.

    Dependabot owns the pinned SHA's value, so this only asserts the
    shape of the pin (correct reusable workflow path, full commit SHA,
    not a mutable branch or tag), not which SHA is pinned.
    """
    uses = _mutation_job(_load()).get("uses")
    assert uses is not None, "jobs.mutation.uses is missing"
    assert USES_RE.match(uses), (
        f"jobs.mutation.uses must reference mutation-cargo.yml pinned to a "
        f"full 40-character lowercase hex commit SHA, not a branch or tag: "
        f"{uses!r}"
    )


def test_job_permissions_are_exactly_least_privilege() -> None:
    """The job grants contents: read and id-token: write, nothing broader."""
    permissions = _mutation_job(_load()).get("permissions")
    assert permissions == {"contents": "read", "id-token": "write"}, (
        "jobs.mutation.permissions must be exactly "
        f"{{'contents': 'read', 'id-token': 'write'}}, got {permissions!r}"
    )


def test_workflow_default_permissions_are_empty() -> None:
    """The workflow-level default token scope is empty."""
    workflow = _load()
    assert workflow.get("permissions") == {}, (
        f"top-level permissions must be an empty mapping, got "
        f"{workflow.get('permissions')!r}"
    )


def test_concurrency_serializes_per_ref_without_cancelling() -> None:
    """Runs queue per ref instead of cancelling one another."""
    concurrency = _load().get("concurrency")
    assert isinstance(concurrency, dict), "the workflow must declare concurrency"
    assert concurrency.get("group") == "mutation-testing-${{ github.ref }}", (
        f"concurrency.group must key on the triggering ref, got "
        f"{concurrency.get('group')!r}"
    )
    assert concurrency.get("cancel-in-progress") is False, (
        f"concurrency.cancel-in-progress must be false, got "
        f"{concurrency.get('cancel-in-progress')!r}"
    )


def test_triggers_keep_schedule_and_plain_dispatch() -> None:
    """The daily schedule stays; dispatch has no legacy branch input."""
    triggers = _triggers(_load())
    schedule = triggers.get("schedule")
    assert schedule == [{"cron": "20 3 * * *"}], (
        f"on.schedule must be the daily 03:20 UTC cron, got {schedule!r}"
    )
    assert "workflow_dispatch" in triggers, "on.workflow_dispatch is missing"
    dispatch = triggers.get("workflow_dispatch") or {}
    inputs = dispatch.get("inputs") or {}
    assert "branch" not in inputs, (
        "on.workflow_dispatch must not declare a branch input; the Actions "
        "run-workflow control selects the ref"
    )


def test_with_block_carries_the_caller_configuration() -> None:
    """The caller passes the workspace paths, excludes, and feature args."""
    with_block = _mutation_job(_load()).get("with")
    assert isinstance(with_block, dict), "jobs.mutation.with is missing"
    assert with_block == EXPECTED_WITH, (
        f"jobs.mutation.with must be exactly {EXPECTED_WITH!r} (workspace "
        f"paths, scaffolding excludes, and the CI feature baseline), got "
        f"{with_block!r}"
    )
