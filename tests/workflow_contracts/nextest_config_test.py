"""Contract tests for the nextest configuration.

Tests that start a child ``cargo`` or ``rustc`` must run one at a time and under
their own time limit, so a cold build blocked on Cargo's locks is bounded work
and not a hang. The Concordat ``nested-cargo-nextest`` rule (NC-004, NC-005)
holds the repository to this; these tests read the configuration with
``tomllib`` and spawn nothing.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[2]
GROUP = "nested-cargo-builds"
#: Test binaries that start a compiler, by nextest binary name.
SPAWNING_BINARIES = ("load_manifest", "compile_time")


def _config() -> dict:
    """Parse the repository's nextest configuration."""
    return tomllib.loads((ROOT / ".config" / "nextest.toml").read_text("utf-8"))


def test_the_nested_cargo_group_is_serial() -> None:
    """NC-004: a test group exists and runs one test at a time."""
    group = _config().get("test-groups", {}).get(GROUP)
    assert group is not None, f"no [test-groups.{GROUP}]"
    assert group.get("max-threads") == 1


def test_an_override_places_every_spawning_binary_in_the_group() -> None:
    """NC-004 and NC-005: the override's filter names each binary that spawns."""
    overrides = _config()["profile"]["default"].get("overrides", [])
    grouped = [o for o in overrides if o.get("test-group") == GROUP]
    assert grouped, f"no override places tests in {GROUP}"
    named = {
        binary
        for override in grouped
        for binary in SPAWNING_BINARIES
        if f"binary(={binary})" in override["filter"]
    }
    assert named == set(SPAWNING_BINARIES), f"unnamed spawning binaries: {named}"


def test_slow_and_global_timeouts_are_set() -> None:
    """NC-004: tests are bounded individually and as a run."""
    profile = _config()["profile"]["default"]
    slow = profile.get("slow-timeout")
    assert slow and slow.get("terminate-after"), "default profile never terminates"
    assert profile.get("global-timeout"), "no global-timeout"
    grouped = [
        o for o in profile.get("overrides", []) if o.get("test-group") == GROUP
    ]
    assert all(o.get("slow-timeout", {}).get("terminate-after") for o in grouped)
