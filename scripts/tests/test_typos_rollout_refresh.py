"""Refresh, transport-security, and cache-failure spelling-policy tests."""

from __future__ import annotations

import email.message
import json
import tomllib
import types
import urllib.error
import urllib.request
from pathlib import Path

import pytest

from typos_rollout_test_support import dictionary_text as _dictionary_text

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]


class ValidResponse:
    """Provide configurable valid dictionary bytes at the HTTP boundary."""

    status = 200

    def __init__(
        self,
        *,
        stem: str = "organ",
        headers: dict[str, str] | None = None,
    ) -> None:
        self._stem = stem
        self.headers = {} if headers is None else headers

    def read(self) -> bytes:
        """Return valid shared dictionary bytes."""
        return _dictionary_text(self._stem).encode()

    def __enter__(self) -> ValidResponse:
        """Enter the fake response context."""
        return self

    def __exit__(self, *_args: object) -> None:
        """Leave the fake response context."""


def test_remote_source_switch_drops_previous_validators(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Conditional validators remain scoped to their original authority."""
    _, _, rollout = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    requests: list[urllib.request.Request] = []
    first_source = "https://example.test/base.toml"

    def open_response(
        request: urllib.request.Request,
        *,
        timeout: float,
    ) -> ValidResponse:
        """Capture requests and return a valid dictionary."""
        assert timeout == pytest.approx(30.0), (
            "remote refresh changed its bounded timeout"
        )
        requests.append(request)
        return ValidResponse(
            headers={
                "ETag": '"estate-v1"',
                "Last-Modified": "Fri, 10 Jul 2026 08:00:00 GMT",
            }
        )

    rollout.refresh_base(
        first_source,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            opener=open_response,
        ),
    )
    same = rollout.refresh_base(
        first_source,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            opener=open_response,
        ),
    )
    replacement = rollout.refresh_base(
        "https://example.test/replacement.toml",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            opener=open_response,
        ),
    )

    assert same.status == "current", "matching validators did not preserve cache"
    assert requests[1].get_header("If-none-match") == '"estate-v1"', (
        "matching authority omitted its ETag validator"
    )
    assert replacement.status == "refreshed", "new authority did not refresh"
    assert requests[2].get_header("If-none-match") is None, (
        "replacement authority inherited the previous ETag"
    )
    assert requests[2].get_header("If-modified-since") is None, (
        "replacement authority inherited the previous date validator"
    )


def test_changed_etag_overrides_unchanged_date(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """A changed ETag refreshes even when Last-Modified is unchanged."""
    _, _, rollout = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    source = "https://example.test/base.toml"
    modified = "Fri, 10 Jul 2026 08:00:00 GMT"
    cache.write_text(_dictionary_text("original"), encoding="utf-8")
    metadata.write_text(
        json.dumps(
            {
                "etag": '"estate-v1"',
                "last_modified": modified,
                "source": source,
            }
        ),
        encoding="utf-8",
    )

    result = rollout.refresh_base(
        source,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            opener=lambda *_args, **_kwargs: ValidResponse(
                stem="replacement",
                headers={"ETag": '"estate-v2"', "Last-Modified": modified},
            ),
        ),
    )

    assert result.status == "refreshed", "changed ETag did not refresh the cache"
    assert rollout.load_dictionary(cache).stems == ("replacement",), (
        "date validator overrode a changed ETag"
    )


def test_connectivity_failure_uses_only_valid_stale_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Connectivity loss raises its domain error unless valid cache exists."""
    _, _, rollout = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"

    def unavailable(*_args: object, **_kwargs: object) -> ValidResponse:
        """Model a network-unavailable authority."""
        raise urllib.error.URLError("offline")

    with pytest.raises(rollout.NetworkUnavailableError) as raised:
        rollout.refresh_base(
            "https://example.test/base",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
                opener=unavailable,
            ),
        )
    assert isinstance(raised.value.__context__, urllib.error.URLError), (
        "connectivity domain error lost its URL failure context"
    )

    cache.write_text(_dictionary_text(), encoding="utf-8")
    result = rollout.refresh_base(
        "https://example.test/base",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            opener=unavailable,
        ),
    )

    assert result.status == "stale-cache", "valid stale cache was not reused"


def test_http_status_and_persistence_errors_propagate(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """HTTP status and local writes never become network fallback successes."""
    cache_module, _, rollout = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    cache.write_text(_dictionary_text(), encoding="utf-8")
    not_found = urllib.error.HTTPError(
        "https://example.test/missing",
        404,
        "Not Found",
        hdrs=None,
        fp=None,
    )

    def missing(*_args: object, **_kwargs: object) -> ValidResponse:
        """Raise the authority's HTTP response."""
        raise not_found

    with pytest.raises(urllib.error.HTTPError) as raised:
        rollout.refresh_base(
            "https://example.test/missing",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
                opener=missing,
            ),
        )
    assert raised.value is not_found, "HTTP status became stale-cache success"

    def denied(*_args: object, **_kwargs: object) -> None:
        """Model denied persistence at the atomic-write boundary."""
        raise PermissionError("cache is read-only")

    monkeypatch.setattr(cache_module, "atomic_write", denied)
    with pytest.raises(PermissionError, match="cache is read-only"):
        rollout.refresh_base(
            "https://example.test/base",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
                opener=lambda *_args, **_kwargs: ValidResponse(stem="replacement"),
            ),
        )


@pytest.mark.parametrize(
    "source",
    ["http://example.test/base.toml", "ftp://example.test/base.toml"],
)
def test_remote_source_requires_https(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
    source: str,
) -> None:
    """Remote dictionary authorities cannot bypass HTTPS."""
    _, _, rollout = rollout_modules
    with pytest.raises(rollout.InsecureSourceError, match="URL must use HTTPS"):
        rollout.refresh_base(
            source,
            tmp_path / "cache.toml",
            rollout.RefreshOptions(
                metadata=tmp_path / "cache.json",
                opener=lambda *_args, **_kwargs: ValidResponse(),
            ),
        )


@pytest.mark.parametrize(
    "target",
    ["http://example.test/base.toml", "ftp://example.test/base.toml"],
)
def test_redirect_target_requires_https(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    target: str,
) -> None:
    """Redirects cannot downgrade or leave HTTPS transport."""
    _, refresh, _ = rollout_modules
    handler = refresh._HttpsRedirectHandler()

    with pytest.raises(
        refresh.InsecureSourceError,
        match="redirect must use HTTPS",
    ):
        handler.redirect_request(
            urllib.request.Request("https://example.test/base.toml"),
            None,
            302,
            "Found",
            {},
            target,
        )


def test_default_refresh_uses_guarded_https_opener(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Production refresh delegates through the guarded redirect opener."""
    _, refresh, rollout = rollout_modules
    requests: list[object] = []

    class GuardedOpener:
        """Capture calls through the configured HTTPS-only opener."""

        def open(self, request: object, *, timeout: float) -> ValidResponse:
            """Return a valid response after recording the guarded call."""
            assert timeout == pytest.approx(30.0), (
                "guarded opener received an unbounded timeout"
            )
            requests.append(request)
            return ValidResponse()

    monkeypatch.setattr(rollout, "_HTTPS_OPENER", GuardedOpener())

    result = rollout.refresh_base(
        "https://example.test/base.toml",
        tmp_path / "cache.toml",
        rollout.RefreshOptions(
            metadata=tmp_path / "cache.json",
        ),
    )

    assert result.status == "refreshed", "guarded opener did not refresh"
    assert len(requests) == 1, "production bypassed the guarded opener"


def test_invalid_download_does_not_replace_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Malformed remote bytes are rejected before cache installation."""
    _, _, rollout = rollout_modules
    cache = tmp_path / "cache.toml"

    class InvalidResponse(ValidResponse):
        """Return malformed TOML from a successful response."""

        def read(self) -> bytes:
            """Return malformed dictionary bytes."""
            return b"not = [valid"

    with pytest.raises(tomllib.TOMLDecodeError):
        rollout.refresh_base(
            "https://example.test/base",
            cache,
            rollout.RefreshOptions(
                metadata=tmp_path / "cache.json",
                opener=lambda *_args, **_kwargs: InvalidResponse(),
            ),
        )
    assert not cache.exists(), "invalid remote bytes replaced the cache"


def test_metadata_and_http_freshness_edge_cases(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Malformed metadata is ignored and 304 remains the sole status fallback."""
    _, refresh, rollout = rollout_modules
    metadata = tmp_path / "cache.json"
    metadata.write_text("not-json", encoding="utf-8")
    assert refresh._read_metadata(metadata) == {}, (
        "invalid JSON metadata was treated as validators"
    )
    metadata.write_text("[]", encoding="utf-8")
    assert refresh._read_metadata(metadata) == {}, (
        "non-object JSON metadata was treated as validators"
    )

    cache = tmp_path / "cache.toml"
    cache.write_text(_dictionary_text(), encoding="utf-8")
    headers = email.message.Message()
    not_modified = urllib.error.HTTPError(
        "https://example.test/base",
        304,
        "not modified",
        headers,
        None,
    )
    current = rollout.refresh_base(
        "https://example.test/base",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            opener=lambda *_args, **_kwargs: (_ for _ in ()).throw(not_modified),
        ),
    )
    assert current.status == "current", "HTTP 304 did not retain a valid current cache"

    assert refresh._remote_is_not_newer(
        {"last_modified": "Fri, 10 Jul 2026 08:00:00 GMT"},
        {"Last-Modified": "Fri, 10 Jul 2026 07:00:00 GMT"},
    ), "older remote date was not recognized as current"
    assert refresh._remote_is_not_newer(
        {"last_modified": "invalid"},
        {"Last-Modified": "invalid"},
    ), "matching malformed dates did not use conservative equality"
    assert not refresh._remote_is_not_newer(
        {},
        {"Last-Modified": "invalid"},
    ), "missing saved date incorrectly proved freshness"
