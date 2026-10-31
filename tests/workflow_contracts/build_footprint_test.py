"""Contract tests for the Rust development-build footprint and backend exception.

The Concordat ``rust-dev-build-footprint`` rule keeps a development ``target``
directory small: the dev profile turns off incremental compilation and debug
data, and the dated nightly pin selects the native Cargo key that stops rlibs
embedding metadata. The ``rust-build-defaults`` rule makes Cranelift the
development backend only where the repository's own suite passes under it.
Weaver's does not (developers' guide, "The Cranelift codegen backend
exception"), so the configuration must carry no ``codegen-backend`` key.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[2]
#: First dated nightly whose Cargo spells the key ``embed-metadata``.
NATIVE_KEY_SINCE = "2026-07-31"
PIN_RE = re.compile(r"nightly-(\d{4}-\d{2}-\d{2})")


def _load(relative: str) -> dict:
    """Parse a TOML file at the repository root."""
    return tomllib.loads((ROOT / relative).read_text("utf-8"))


def test_dev_profile_drops_incremental_caches_and_debug_data() -> None:
    """DF-001 and DF-002: the default dev profile sets both, with no override."""
    dev = _load("Cargo.toml").get("profile", {}).get("dev", {})
    assert dev.get("incremental") is False, "profile.dev.incremental must be false"
    assert dev.get("debug") == 0, "profile.dev.debug must be 0"
    assert "package" not in dev, "a per-package override can re-enable debug data"


def test_native_metadata_key_matches_the_pinned_nightly() -> None:
    """DF-003: a pin from 2026-07-31 uses ``embed-metadata = false``.

    The raw ``rustc`` metadata flags are refused: they broke build scripts.
    """
    channel = _load("rust-toolchain.toml")["toolchain"]["channel"]
    match = PIN_RE.fullmatch(channel)
    assert match, f"the pin {channel!r} is not a dated nightly"
    assert match.group(1) >= NATIVE_KEY_SINCE, (
        f"{channel} predates the native key; use no-embed-metadata = true"
    )
    config = _load(".cargo/config.toml")
    assert config.get("unstable", {}).get("embed-metadata") is False
    assert "no-embed-metadata" not in config.get("unstable", {})
    rendered = (ROOT / ".cargo" / "config.toml").read_text("utf-8")
    assert "-Zembed-metadata" not in rendered
    assert "-Zno-embed-metadata" not in rendered


def test_cranelift_is_not_selected() -> None:
    """The suite aborts under Cranelift, so no source may select the backend.

    A ``codegen-backend`` key in the config's ``[unstable]`` or ``[profile]``
    tables, or in the root manifest's profiles, would turn the backend back on
    for development builds. Re-measure on the next toolchain bump and change
    this test with the exception section in the developers' guide.
    """
    config = _load(".cargo/config.toml")
    manifest = _load("Cargo.toml")
    assert "codegen-backend" not in config.get("unstable", {})
    for name, source in (("config", config), ("manifest", manifest)):
        for profile, table in source.get("profile", {}).items():
            assert "codegen-backend" not in table, f"{name} profile.{profile}"
    flags = [
        flag
        for table in (config.get("build", {}), *config.get("target", {}).values())
        for flag in table.get("rustflags", [])
    ]
    assert not any("codegen-backend" in flag for flag in flags), flags
