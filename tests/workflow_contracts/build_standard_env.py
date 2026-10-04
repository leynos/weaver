"""The environment every build-standard dry run of the Makefile runs under.

Make reads a recipe's inputs from the environment as well as the command line,
so a variable the caller exported (a target, a flag set, a Make option) can
change what ``make -n`` prints and make a contract pass or fail for a reason
the case did not state. Every dry run therefore starts from this environment,
with those variables removed, and each case supplies what it needs explicitly.
"""

from __future__ import annotations

import os

#: Every variable the Makefile or Make itself reads that selects a target, a
#: flag set or a recipe's behaviour. Each is removed unless a case supplies it.
DROPPED = frozenset(
    {
        "CARGO_BUILD_TARGET",
        "CARGO_FLAGS",
        "CLIPPY_FLAGS",
        "TEST_FLAGS",
        "RUST_FLAGS",
        "RUSTDOC_FLAGS",
        "RUSTFLAGS",
        "RUSTDOCFLAGS",
        "STANDARD_THREADS_FLAG",
        "STANDARD_MOLD_FLAG",
        "BUILD_HOST_OS",
        "MAKEFLAGS",
        "MFLAGS",
        "MAKELEVEL",
        "MAKEOVERRIDES",
    }
)


def controlled_environment(**supplied: str) -> dict[str, str]:
    """Return the process environment without any variable in ``DROPPED``.

    Variables a case supplies are added after the removal, so a case that tests
    an inherited ``RUSTFLAGS`` sets it deliberately rather than receiving the
    caller's.
    """
    environment = {
        key: val for key, val in os.environ.items() if key not in DROPPED
    }
    environment.update(supplied)
    return environment
