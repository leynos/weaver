"""Keep the disposable caller experiment confined to its reviewed branch push."""

from pathlib import Path

from workflow_loader import load_workflow

WORKFLOW = Path(__file__).resolve().parents[2] / ".github/workflows/macos-sandbox-caller-boundary-diagnostic.yml"
BRANCH = "diagnostic-macos-sandbox-caller-boundary-oct14"


def test_only_exact_branch_push_starts_experiment() -> None:
    """A PR, schedule, dispatch or tag must never activate the experiment."""
    workflow = load_workflow(WORKFLOW.read_text())
    assert workflow["on"] == {"push": {"branches": [BRANCH]}}, "diagnostic event scope changed"
    assert workflow["permissions"] == {"contents": "read"}, "diagnostic needs read-only contents"
    jobs = workflow["jobs"]
    assert set(jobs) == {"observe"}, "unexpected diagnostic jobs"
    job = jobs["observe"]
    assert job["runs-on"] == "macos-15", "unreviewed runner change"
    assert job["timeout-minutes"] == 15, "diagnostic budget changed"
    condition = " ".join(job["if"].split())
    assert condition == (
        "github.repository == 'leynos/weaver' && github.actor == 'leynos' "
        "&& github.event_name == 'push' && github.event.deleted == false "
        f"&& github.ref == 'refs/heads/{BRANCH}'"
    ), "owner, actor, live branch and event guards must all hold"


def test_checkout_is_immutable_and_upload_is_narrow() -> None:
    """The pushed reviewed code is used and only diagnostic evidence escapes."""
    job = load_workflow(WORKFLOW.read_text())["jobs"]["observe"]
    checkout, setup, *rest = job["steps"]
    assert checkout["uses"] == "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1", "checkout pin changed"
    assert checkout["with"] == {"ref": "${{ github.sha }}", "fetch-depth": 2, "persist-credentials": False}, "checkout provenance changed"
    assert setup["uses"] == "leynos/shared-actions/.github/actions/setup-rust@cfcc379e29be516160f499864ae82720842359af", "setup pin changed"
    upload = rest[-1]
    assert upload["if"] == "always()", "failed observations must retain evidence"
    assert upload["uses"] == "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02", "uploader pin changed"
    assert upload["with"]["path"] == "${{ runner.temp }}/macos-sandbox-caller-boundary", "artifact scope expanded"
    runs = "\n".join(step.get("run", "") for step in rest)
    assert "--no-run --message-format=json -vv" in runs, "build must not activate the probe"
    assert "tools/dev-fast" not in runs, "verification must not select development config"
    assert "continue-on-error" not in WORKFLOW.read_text(), "diagnostic findings must propagate"


def test_routing_preflight_precedes_build_and_observation() -> None:
    """Ambient development routing must be refused before compiling the probe."""
    steps = load_workflow(WORKFLOW.read_text())["jobs"]["observe"]["steps"]
    scripts = [step["run"] for step in steps if "run" in step]
    assert len(scripts) == 5, "unexpected experiment commands"
    assert "python3 -m unittest discover -s tests/diagnostics" in scripts[1], "controller safety contracts must run first"
    assert "-p 'macos_caller_boundary*_test.py' -v" in scripts[1], "all sixteen preparation contracts must execute"
    assert "--check-routing-only" in scripts[2], "preflight must precede compilation"
    assert "cargo +nightly-2026-03-26 test --locked" in scripts[3], "build order changed"
    assert "--cargo-artifact-log" in scripts[4], "observation must follow compilation"


def test_log_directory_is_initialized_in_runner_shell() -> None:
    """Runner paths must be resolved in a step and persisted for later steps."""
    job = load_workflow(WORKFLOW.read_text())["jobs"]["observe"]
    assert "RUN_LOG_DIR" not in job.get("env", {}), "runner context is unavailable in job env"
    record = job["steps"][2]["run"]
    assert record.startswith(
        "set -euo pipefail\n"
        'RUN_LOG_DIR="$RUNNER_TEMP/macos-sandbox-caller-boundary"\n'
        "printf 'RUN_LOG_DIR=%s\\n' \"$RUN_LOG_DIR\" >> \"$GITHUB_ENV\"\n"
        'mkdir -p "$RUN_LOG_DIR"\n'
    ), "initialize the local log path and persist it before recording evidence"
