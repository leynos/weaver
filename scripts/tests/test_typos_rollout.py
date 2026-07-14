"""Dictionary, generator, and atomic-write tests for spelling policy."""

from __future__ import annotations

import importlib
import json
import os
import tomllib
import types
import urllib.error
from pathlib import Path

import pytest

from typos_rollout_test_support import dictionary_text as _dictionary_text

type RolloutModules = tuple[
    types.ModuleType,
    types.ModuleType,
    types.ModuleType,
    types.ModuleType,
]


@pytest.fixture(name="rollout_modules")
def rollout_modules_fixture(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
) -> RolloutModules:
    """Add the generator module to the shared rollout fixture."""
    generator = importlib.import_module("generate_typos_config")
    return (*rollout_modules, generator)


def test_rollout_generates_oxford_corrections(
    rollout_modules: RolloutModules,
) -> None:
    """The renderer accepts Oxford forms and corrects plain-British ones."""
    _, _, rollout, _ = rollout_modules

    mappings = rollout.generate_word_mappings(
        rollout.Dictionary(stems=("organ", "recogn"))
    )

    assert mappings["organize"] == "organize", "Oxford verb was not accepted"
    assert mappings["organise"] == "organize", "plain-British verb was not corrected"
    assert mappings["recognizably"] == "recognizably", "Oxford adverb was not accepted"
    assert mappings["recognisably"] == "recognizably", (
        "plain-British adverb was not corrected"
    )


def test_connectivity_failure_without_cache_raises_domain_error(
    rollout_modules: RolloutModules,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """A tracked config cannot hide an authority outage without a valid cache."""
    _, _, rollout, generator = rollout_modules
    tracked_config = tmp_path / "typos.toml"
    reviewed = '[default]\nlocale = "en-gb"\n'
    tracked_config.write_text(reviewed, encoding="utf-8")

    def unavailable(*_args: object, **_kwargs: object) -> None:
        """Model an unavailable HTTPS authority."""
        message = "offline"
        raise rollout.NetworkUnavailableError(message) from urllib.error.URLError(
            message
        )

    monkeypatch.setattr(rollout, "refresh_base", unavailable)

    with pytest.raises(rollout.NetworkUnavailableError, match="offline"):
        generator.main(
            repository=tmp_path,
            source="https://example.invalid/base",
        )

    assert tracked_config.read_text(encoding="utf-8") == reviewed, (
        "failed refresh unexpectedly changed the tracked config"
    )


@pytest.mark.parametrize(
    "error",
    [
        pytest.param(
            urllib.error.HTTPError(
                "https://example.test/missing",
                404,
                "Not Found",
                hdrs=None,
                fp=None,
            ),
            id="http-status",
        ),
        pytest.param(PermissionError("cache is read-only"), id="persistence"),
    ],
)
def test_generator_propagates_non_connectivity_failures(
    rollout_modules: RolloutModules,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    error: OSError,
) -> None:
    """HTTP status and persistence failures cannot become tracked success."""
    _, _, rollout, generator = rollout_modules
    (tmp_path / "typos.toml").write_text(
        '[default]\nlocale = "en-gb"\n',
        encoding="utf-8",
    )

    def fail(*_args: object, **_kwargs: object) -> None:
        """Raise the selected non-connectivity failure."""
        raise error

    monkeypatch.setattr(rollout, "refresh_base", fail)

    with pytest.raises(type(error)) as raised:
        generator.main(
            repository=tmp_path,
            source="https://example.test/base",
        )

    assert raised.value is error, (
        "generator replaced a non-connectivity failure with fallback success"
    )


def test_generator_writes_config_from_valid_untracked_cache(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """The generator renders a real validated cache into tracked configuration."""
    _, _, _, generator = rollout_modules
    (tmp_path / ".typos-oxendict-base.toml").write_text(
        _dictionary_text(),
        encoding="utf-8",
    )

    result = generator.main(repository=tmp_path, offline=True)

    rendered = tomllib.loads((tmp_path / "typos.toml").read_text(encoding="utf-8"))
    assert result.status == "offline-cache", "generator did not reuse the valid cache"
    assert rendered["default"]["extend-words"]["organise"] == "organize", (
        "generated configuration omitted the cache's Oxford correction"
    )


def test_render_and_write_are_deterministic_valid_toml(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """Rendering is stable, parseable and atomically installed."""
    _, _, rollout, _ = rollout_modules
    dictionary = rollout.Dictionary(
        stems=("organ",),
        accepted=("proper-name",),
        ignore_patterns=("https?://",),
        excluded_files=("target",),
    )
    output = tmp_path / "nested" / "typos.toml"

    first = rollout.render_typos_config(dictionary)
    rollout.write_config(output, dictionary)

    assert first == rollout.render_typos_config(dictionary), (
        "identical dictionaries rendered differently"
    )
    assert output.read_text(encoding="utf-8") == first, (
        "atomic write changed rendered configuration"
    )
    parsed = tomllib.loads(first)
    assert parsed["default"]["locale"] == "en-gb", (
        "rendered configuration lost the British locale"
    )
    assert parsed["default"]["extend-ignore-re"] == ["https?://"], (
        "rendered configuration lost the ignore patterns"
    )
    assert parsed["files"]["extend-exclude"] == ["target"], (
        "rendered configuration lost the file exclusions"
    )
    assert list(output.parent.glob(".typos.toml.*")) == [], (
        "successful atomic write left a temporary file"
    )


@pytest.mark.parametrize("failure_stage", ["write", "close", "replace"])
def test_atomic_write_cleans_temporary_file_after_failure(
    rollout_modules: RolloutModules,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    failure_stage: str,
) -> None:
    """Write, close, and replacement failures remove the temporary file."""
    cache_module, _, _, _ = rollout_modules
    temporary = tmp_path / ".typos.toml.failure"
    temporary.touch()

    class FailingStream:
        """Model a named temporary stream with one selected failure."""

        name = str(temporary)

        def __enter__(self) -> FailingStream:
            """Enter the fake stream context."""
            return self

        def write(self, content: bytes) -> None:
            """Write bytes unless this case models a write failure."""
            if failure_stage == "write":
                raise OSError("write failure")
            temporary.write_bytes(content)

        def __exit__(self, *_args: object) -> None:
            """Close unless this case models a close failure."""
            if failure_stage == "close":
                raise OSError("close failure")

    monkeypatch.setattr(
        cache_module.tempfile,
        "NamedTemporaryFile",
        lambda **_kwargs: FailingStream(),
    )
    if failure_stage == "replace":

        def fail_replace(_path: Path, _destination: Path) -> None:
            """Model an atomic replacement failure."""
            raise OSError("replace failure")

        monkeypatch.setattr(cache_module.pathlib.Path, "replace", fail_replace)

    with pytest.raises(OSError, match=f"{failure_stage} failure"):
        cache_module.atomic_write(tmp_path / "typos.toml", b"content")

    assert not temporary.exists(), f"temporary file survived {failure_stage} failure"


def test_local_refresh_keeps_a_newer_cache(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """An older local authority cannot replace a newer untracked cache."""
    _, _, rollout, _ = rollout_modules
    source = tmp_path / "shared.toml"
    cache = tmp_path / ".typos-base.toml"
    metadata = tmp_path / ".typos-base.json"
    source.write_text(_dictionary_text(), encoding="utf-8")
    source.touch()
    rollout.refresh_base(
        source,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )
    cache.write_text(_dictionary_text("newer"), encoding="utf-8")
    cache_mtime = max(cache.stat().st_mtime_ns, source.stat().st_mtime_ns + 1)
    os.utime(cache, ns=(cache_mtime, cache_mtime))

    result = rollout.refresh_base(
        source,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )

    assert result.status == "current", "older local authority replaced a newer cache"
    assert rollout.load_dictionary(cache).stems == ("newer",), (
        "current local refresh changed cached policy"
    )


def test_offline_refresh_requires_and_reuses_valid_cache(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """Offline mode fails closed before reusing a validated cache."""
    _, _, rollout, _ = rollout_modules
    cache = tmp_path / "base.toml"
    metadata = tmp_path / "base.json"

    with pytest.raises(FileNotFoundError, match="no cached shared dictionary"):
        rollout.refresh_base(
            "https://example.invalid/base",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
                offline=True,
            ),
        )

    cache.write_text(_dictionary_text(), encoding="utf-8")
    result = rollout.refresh_base(
        "https://example.invalid/base",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            offline=True,
        ),
    )

    assert result.status == "offline-cache", (
        "offline refresh did not reuse a valid cache"
    )


def test_local_refresh_switches_authority_and_records_metadata(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """A different explicit authority replaces a cache regardless of mtime."""
    _, _, rollout, _ = rollout_modules
    first = tmp_path / "first.toml"
    second = tmp_path / "second.toml"
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    first.write_text(_dictionary_text("first"), encoding="utf-8")
    second.write_text(_dictionary_text("second"), encoding="utf-8")
    os.utime(first, ns=(3_000_000_000, 3_000_000_000))
    os.utime(second, ns=(1_000_000_000, 1_000_000_000))
    rollout.refresh_base(
        first,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )

    result = rollout.refresh_base(
        second,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )

    assert result.status == "refreshed", (
        "authority switch did not refresh the local cache"
    )
    assert rollout.load_dictionary(cache).stems == ("second",), (
        "authority switch retained the previous dictionary"
    )
    saved = json.loads(metadata.read_text(encoding="utf-8"))
    assert saved["source"] == str(second.resolve()), (
        "authority switch persisted the wrong source identity"
    )
