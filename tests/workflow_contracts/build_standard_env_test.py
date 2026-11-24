"""Contract tests that the build-standard dry runs ignore the caller's exports.

Make reads a recipe's inputs from the environment, so a target, a flag set or a
Make option the caller has exported could change what ``make -n`` prints and
decide a contract for a reason the case never stated. Each dry-run helper of the
build-standard contracts starts from ``controlled_environment``; these tests
export a contaminating value in the parent environment and require every helper
to print exactly what it prints on a clean one.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from collections.abc import Callable

import build_standard_target_test as target_contract
import build_standard_test as flag_contract
import build_standard_warnings_test as warning_contract
import pytest
from build_standard_env import DROPPED

#: A value for each variable, each one that would change a dry run if it leaked.
CONTAMINATION = {
    "CARGO_BUILD_TARGET": "aarch64-apple-darwin",
    "CARGO_FLAGS": "--target aarch64-apple-darwin",
    "CLIPPY_FLAGS": "--target aarch64-apple-darwin -- --cfg contaminated",
    "TEST_FLAGS": "--target aarch64-apple-darwin",
    "RUST_FLAGS": "--cfg contaminated",
    "RUSTDOC_FLAGS": "--cfg contaminated",
    "RUSTFLAGS": "-Ccontaminated",
    "RUSTDOCFLAGS": "--cfg contaminated",
    "STANDARD_THREADS_FLAG": "-Zcontaminated",
    "STANDARD_MOLD_FLAG": "-Ccontaminated",
    "BUILD_HOST_OS": "Darwin",
    "MAKEFLAGS": "BUILD_HOST_OS=Darwin",
    "MFLAGS": "-n",
    "MAKELEVEL": "1",
    "MAKEOVERRIDES": "BUILD_HOST_OS=Darwin",
}

#: Each dry-run helper, called for a target whose output the contamination
#: would change.
HELPERS: dict[str, Callable[[], object]] = {
    "target contract": lambda: target_contract._commands("test", ()),
    "warning contract": lambda: warning_contract._dry_run("lint"),
    "flag contract": lambda: flag_contract._make_rustflags("lint", "Linux"),
    "flag contract expansion": lambda: flag_contract._expanded("$RUSTFLAGS", None),
}


def test_every_dropped_variable_has_a_contaminating_value() -> None:
    """The table covers the whole drop list, so a new entry needs a case."""
    assert set(CONTAMINATION) == set(DROPPED)


@pytest.mark.parametrize("helper", sorted(HELPERS))
@pytest.mark.parametrize("variable", sorted(CONTAMINATION))
def test_a_dry_run_ignores_an_exported_variable(
    helper: str, variable: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Exporting ``variable`` in the parent leaves the helper's output unchanged."""
    clean = HELPERS[helper]()
    monkeypatch.setenv(variable, CONTAMINATION[variable])
    assert HELPERS[helper]() == clean, f"{helper} followed exported {variable}"
