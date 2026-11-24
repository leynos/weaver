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


def _flag_tokens(value: str | list[str]) -> list[str]:
    """Split a Cargo ``rustflags`` value into tokens.

    Cargo accepts an array of strings or one space-separated string, and an
    array may split a flag from its value (``["-Z", "embed-metadata=no"]``), so
    every spelling is reduced to the same token list.
    """
    items = [value] if isinstance(value, str) else list(value)
    return [token for item in items for token in str(item).split()]


def _rustflags_tokens(config: dict) -> list[str]:
    """Return every token of every ``rustflags`` source in the Cargo config."""
    tables = (config.get("build", {}), *config.get("target", {}).values())
    return [
        token
        for table in tables
        for token in _flag_tokens(table.get("rustflags", []))
    ]


def _nested_profile_tables(source: dict) -> list[tuple[str, dict]]:
    """Return every profile table in a manifest or config, nested ones included.

    A backend or debug setting can sit in a profile, in one of its per-package
    overrides, or in its ``build-override`` for build scripts and proc macros.
    """
    found: list[tuple[str, dict]] = []
    for name, profile in source.get("profile", {}).items():
        found.append((f"profile.{name}", profile))
        found.append((f"profile.{name}.build-override", profile.get("build-override", {})))
        for package, table in profile.get("package", {}).items():
            found.append((f"profile.{name}.package.{package}", table))
    return found


def test_dev_profile_drops_incremental_caches_and_debug_data() -> None:
    """DF-001 and DF-002: the default dev profile sets both, with no override.

    Cargo reads ``[profile.dev]`` from ``.cargo/config.toml`` as well, and the
    config's values win over the manifest's, so a ``debug = 2`` there would undo
    the manifest. The environment (``CARGO_PROFILE_DEV_DEBUG=2``) is a
    deliberate per-session override and is not read here.
    """
    manifest = _load("Cargo.toml").get("profile", {}).get("dev", {})
    assert manifest.get("incremental") is False, "profile.dev.incremental must be false"
    assert manifest.get("debug") == 0, "profile.dev.debug must be 0"
    config = _load(".cargo/config.toml").get("profile", {}).get("dev", {})
    for where, dev in (("manifest", manifest), ("config", config)):
        assert dev.get("incremental", False) is False, f"{where} re-enables incremental"
        assert dev.get("debug", 0) == 0, f"{where} re-enables debug data"
        assert "package" not in dev, f"{where}: a per-package override can re-enable debug"
        assert "build-override" not in dev, f"{where}: a build override can re-enable debug"


def test_native_metadata_key_matches_the_pinned_nightly() -> None:
    """DF-003: a pin from 2026-07-31 uses ``embed-metadata = false``.

    The raw ``rustc`` metadata flags are refused in every spelling: they broke
    build scripts.
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
    raw = [token for token in _rustflags_tokens(config) if "embed-metadata" in token]
    assert raw == [], f"raw rustc metadata flags in rustflags: {raw}"


def test_cranelift_is_not_selected() -> None:
    """The suite aborts under Cranelift, so no source may select the backend.

    A ``codegen-backend`` key in the config's ``[unstable]`` or ``[profile]``
    tables, in the root manifest's profiles (including per-package and
    build-script overrides), or in a ``rustflags`` source would turn the backend
    back on for development builds. Re-measure on the next toolchain bump and
    change this test with the exception section in the developers' guide.
    """
    config = _load(".cargo/config.toml")
    manifest = _load("Cargo.toml")
    assert "codegen-backend" not in config.get("unstable", {})
    for name, source in (("config", config), ("manifest", manifest)):
        for where, table in _nested_profile_tables(source):
            assert "codegen-backend" not in table, f"{name} {where}"
    flags = [token for token in _rustflags_tokens(config) if "codegen-backend" in token]
    assert flags == [], flags
