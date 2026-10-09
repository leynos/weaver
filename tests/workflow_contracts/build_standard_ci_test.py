"""Contract tests for where CI installs `mold` relative to the gate commands.

The Makefile restates the build standard's `mold` flag for its gate targets,
and the boundary-manifest step composes it by hand around a direct ``cargo
test``, so every Linux job must install `mold` before the first of either runs.
The Makefile and configuration clauses live in ``build_standard_test.py``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import TypedDict

import pytest
import yaml
from hypothesis import given, settings
from hypothesis import strategies as st

ROOT = Path(__file__).resolve().parents[2]
THREADS_FLAG = "-Zthreads=8"
LINKER_FLAG = "-Clink-arg=-fuse-ld=mold"
#: Make targets whose CI invocation builds Rust, so the job needs `mold`.
GATE_TARGETS = {"test", "lint", "typecheck", "build", "all"}
MAKE_TARGET_RE = re.compile(r"\bmake\s+(?:-\S+\s+)*([\w-]+)")
#: A direct cargo invocation that builds, which needs `mold` like a gate target.
CARGO_GATE_RE = re.compile(r"\bcargo\s+(?:\+\S+\s+)?(?:test|build|check|clippy)\b")
BOUNDARY_STEP = "Boundary manifest gate"


#: The fields of a workflow step this contract reads. Functional syntax,
#: because ``with`` is a keyword.
WorkflowStep = TypedDict(
    "WorkflowStep",
    {
        "name": str,
        "run": str,
        "uses": str,
        "env": dict[str, str],
        "with": dict[str, object],
    },
    total=False,
)


def _linux_jobs() -> list[tuple[str, dict]]:
    """Return every CI job not placed on Windows or macOS, named by file."""
    jobs = []
    for path in sorted((ROOT / ".github" / "workflows").glob("*.y*ml")):
        workflow = yaml.safe_load(path.read_text("utf-8")) or {}
        jobs.extend(
            (f"{path.name}:{name}", job)
            for name, job in (workflow.get("jobs") or {}).items()
            if not re.search(r"windows|macos", str(job.get("runs-on", "")), re.I)
        )
    return jobs


def _first_positions(step: WorkflowStep) -> tuple[int | None, int | None]:
    """Return where a step first installs `mold` and first runs a gate target.

    Positions are offsets into the step's ``run`` text; a setup-rust step that
    installs `mold` through its input counts as installing at offset 0.
    """
    run = str(step.get("run", ""))
    inputs = step.get("with") or {}
    installs = [
        match.start()
        for match in re.finditer(r"apt(-get)?\s+install[^\n]*\bmold\b", run)
    ]
    if "setup-rust" in str(step.get("uses", "")) and (
        str(inputs.get("install-mold", "")).lower() == "true"
    ):
        installs.append(0)
    gates = [
        match.start()
        for match in MAKE_TARGET_RE.finditer(run)
        if match.group(1) in GATE_TARGETS
    ] + [match.start() for match in CARGO_GATE_RE.finditer(run)]
    return min(installs, default=None), min(gates, default=None)


def _gate_precedes_install(install_at: int | None, gate_at: int | None) -> bool:
    """Report whether a step's first gate target comes before its `mold` install."""
    if gate_at is None:
        return False
    return install_at is None or gate_at < install_at


def _runs_a_gate_target_first(job: dict) -> bool:
    """Report whether a job runs a gate target before any step installs `mold`,
    comparing positions within a step that does both."""
    for step in job.get("steps") or []:
        install_at, gate_at = _first_positions(step)
        if _gate_precedes_install(install_at, gate_at):
            return True
        if install_at is not None:
            return False
    return False


def _jobs_missing_the_linker() -> list[str]:
    """Return the Linux CI jobs that run a gate target without installing
    `mold` first."""
    return [name for name, job in _linux_jobs() if _runs_a_gate_target_first(job)]


def test_ci_installs_the_linker_before_gate_targets() -> None:
    """The gate targets restate `mold`, so a Linux job must install it first."""
    missing = _jobs_missing_the_linker()
    assert missing == [], f"jobs run a gate target before installing `mold`: {missing}"


COVERAGE_STEP: WorkflowStep = {
    "uses": "o/shared-actions/.github/actions/generate-coverage@x"
}
RUST_STEP: WorkflowStep = {"uses": "o/shared-actions/.github/actions/setup-rust@x"}
MOLD_RUST_STEP: WorkflowStep = {**RUST_STEP, "with": {"install-mold": "true"}}
APT_STEP: WorkflowStep = {"run": "sudo apt-get install -y mold"}
OTHER_STEP: WorkflowStep = {"run": "echo unrelated"}


def _runs_coverage_before_the_linker(job: dict[str, list[WorkflowStep]]) -> bool:
    """Report whether a job runs `generate-coverage` before installing `mold`."""
    # Coverage builds Rust through the action, not a gate target, so the gate
    # ordering check cannot see it; its nested cargo runs (the trybuild cases)
    # read `.cargo/config.toml` and link with `mold`.
    for step in job.get("steps") or []:
        if "generate-coverage" in str(step.get("uses", "")):
            return True
        if _first_positions(step)[0] is not None:
            return False
    return False


def _covers(job: dict) -> bool:
    """Report whether a job has a `generate-coverage` step."""
    return any(
        "generate-coverage" in str(step.get("uses", ""))
        for step in job.get("steps") or []
    )


def test_coverage_jobs_install_the_linker_before_generating_coverage() -> None:
    """A coverage lane without `mold` fails the trybuild link on the runner."""
    missing = [
        name for name, job in _linux_jobs() if _runs_coverage_before_the_linker(job)
    ]
    assert missing == [], f"jobs run coverage before installing `mold`: {missing}"


def test_the_main_coverage_lane_is_held_to_the_linker_contract() -> None:
    """The check covers `coverage-main.yml`, so it cannot pass by omission."""
    names = {name for name, job in _linux_jobs() if _covers(job)}
    assert "coverage-main.yml:coverage-upload" in names, (
        f"coverage-main.yml:coverage-upload is not a coverage job; found {names}"
    )


@pytest.mark.parametrize(
    ("steps", "unsafe", "case"),
    [
        ([COVERAGE_STEP], True, "no install at all"),
        ([RUST_STEP, COVERAGE_STEP], True, "setup-rust without the input"),
        ([MOLD_RUST_STEP, COVERAGE_STEP], False, "install-mold input"),
        ([APT_STEP, COVERAGE_STEP], False, "apt install"),
        ([COVERAGE_STEP, MOLD_RUST_STEP], True, "install after coverage"),
    ],
)
def test_the_coverage_ordering_check_reads_each_way_to_install_the_linker(
    steps: list[WorkflowStep], unsafe: bool, case: str
) -> None:
    """Only an install before the coverage step makes the job safe."""
    assert _runs_coverage_before_the_linker({"steps": steps}) is unsafe, case


@settings(max_examples=200, deadline=None)
@given(
    st.lists(
        st.sampled_from([COVERAGE_STEP, RUST_STEP, MOLD_RUST_STEP, APT_STEP, OTHER_STEP]),
        max_size=8,
    )
)
def test_the_coverage_ordering_check_matches_a_first_step_oracle(
    steps: list[WorkflowStep],
) -> None:
    """Unsafe exactly when coverage appears and no installer precedes the first."""
    installers = [MOLD_RUST_STEP, APT_STEP]
    coverage_at = next((n for n, s in enumerate(steps) if s is COVERAGE_STEP), None)
    install_at = next((n for n, s in enumerate(steps) if s in installers), None)
    expected = coverage_at is not None and (
        install_at is None or coverage_at < install_at
    )
    assert _runs_coverage_before_the_linker({"steps": steps}) is expected, steps


def _job_with_step(name: str) -> dict | None:
    """Return the first CI job holding a step called ``name``."""
    for _, job in _linux_jobs():
        if any(step.get("name") == name for step in job.get("steps") or []):
            return job
    return None


def test_the_boundary_gate_composes_the_standard_flags() -> None:
    """The step runs cargo directly under setup-rust's exported ``RUSTFLAGS``,
    which displaces the configuration, so it must compose both standard flags
    onto the inherited value itself."""
    job = _job_with_step(BOUNDARY_STEP)
    assert job is not None, f"no `{BOUNDARY_STEP}` step"
    (step,) = [s for s in job["steps"] if s.get("name") == BOUNDARY_STEP]
    env = " ".join(str(value) for value in (step.get("env") or {}).values())
    assert THREADS_FLAG in env, f"`{BOUNDARY_STEP}` lost {THREADS_FLAG}"
    assert LINKER_FLAG in env, f"`{BOUNDARY_STEP}` lost `mold`"
    run = str(step.get("run", ""))
    assert re.search(r'RUSTFLAGS="\$\{RUSTFLAGS:\+\$RUSTFLAGS \}\$\w+"\s+cargo', run), (
        f"`{BOUNDARY_STEP}` does not compose the inherited RUSTFLAGS: {run}"
    )
