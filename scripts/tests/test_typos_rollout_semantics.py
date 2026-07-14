"""Security contracts for authority and local spelling policy."""

from __future__ import annotations

import email.message
import types
import urllib.error
from pathlib import Path

import pytest

from typos_rollout_test_support import dictionary_text

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
type RolloutModules = tuple[types.ModuleType, types.ModuleType, types.ModuleType]


@pytest.mark.parametrize("schema", ["true", "1.0"])
def test_authority_requires_integer_schema(
    rollout_modules: RolloutModules,
    tmp_path: Path,
    schema: str,
) -> None:
    """Boolean and floating-point schema values cannot validate as version 1."""
    _, _, rollout = rollout_modules
    authority = tmp_path / "base.toml"
    authority.write_text(
        dictionary_text().replace("schema = 1", f"schema = {schema}"),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="unsupported dictionary schema"):
        rollout.load_dictionary(authority)


@pytest.mark.parametrize(
    ("fragment", "message"),
    [
        ("[phrases.corrections]\n\n", "missing required table 'phrases'"),
        ("exclude = []\n", "missing required field files.exclude"),
    ],
)
def test_authority_requires_complete_policy(
    rollout_modules: RolloutModules,
    tmp_path: Path,
    fragment: str,
    message: str,
) -> None:
    """Every authority table and expected field is mandatory."""
    _, _, rollout = rollout_modules
    authority = tmp_path / "base.toml"
    authority.write_text(dictionary_text().replace(fragment, ""), encoding="utf-8")

    with pytest.raises(ValueError, match=message):
        rollout.load_dictionary(authority)


def test_explicit_sparse_local_overlay_remains_valid(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """Only explicitly sparse local loads may omit unrelated policy tables."""
    _, _, rollout = rollout_modules
    overlay = tmp_path / "typos.local.toml"
    overlay.write_text(
        'schema = 1\n\n[words]\naccepted = ["Weaver"]\n', encoding="utf-8"
    )

    parsed = rollout.load_dictionary(overlay, local_overlay=True)

    assert parsed.accepted == ("Weaver",), "sparse local vocabulary was lost"
    assert parsed.stems == (), "sparse local load invented authority policy"


@pytest.mark.parametrize(
    ("ignore_patterns", "excluded_files"),
    [
        pytest.param((".*",), (), id="empty-matching-ignore"),
        pytest.param((".+",), (), id="generic-prose-ignore"),
        pytest.param((), ("**/*",), id="universal-exclusion"),
    ],
)
def test_merge_rejects_broad_local_exceptions(
    rollout_modules: RolloutModules,
    ignore_patterns: tuple[str, ...],
    excluded_files: tuple[str, ...],
) -> None:
    """Local exceptions cannot disable the estate policy broadly."""
    _, _, rollout = rollout_modules
    local = rollout.Dictionary(
        ignore_patterns=ignore_patterns,
        excluded_files=excluded_files,
    )

    with pytest.raises(ValueError, match="too broad"):
        rollout.merge_dictionaries(rollout.Dictionary(), local)


def test_merge_accepts_weaver_local_exceptions(
    rollout_modules: RolloutModules,
) -> None:
    """Weaver's exact identifiers and anchored lines remain narrow exceptions."""
    _, _, rollout = rollout_modules
    local = rollout.load_dictionary(
        REPOSITORY_ROOT / "typos.local.toml", local_overlay=True
    )

    merged = rollout.merge_dictionaries(rollout.Dictionary(), local)

    assert merged.ignore_patterns == local.ignore_patterns, (
        "narrow Weaver exceptions changed during merge"
    )


def test_http_304_requires_valid_cache(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """The production HTTPError 304 path cannot accept a missing cache."""
    _, _, rollout = rollout_modules
    not_modified = urllib.error.HTTPError(
        "https://example.test/base",
        304,
        "not modified",
        email.message.Message(),
        None,
    )

    with pytest.raises(urllib.error.HTTPError) as raised:
        rollout.refresh_base(
            "https://example.test/base",
            tmp_path / "cache.toml",
            rollout.RefreshOptions(
                metadata=tmp_path / "cache.json",
                opener=lambda *_args, **_kwargs: (_ for _ in ()).throw(not_modified),
            ),
        )

    assert raised.value is not_modified, "HTTP 304 accepted a missing cache"
