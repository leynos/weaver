from __future__ import annotations

import resource
import sys
import types

from weaverd import server


def test_get_rss_mb_linux(monkeypatch) -> None:
    usage = types.SimpleNamespace(ru_maxrss=2048)
    monkeypatch.setattr(resource, "getrusage", lambda _: usage)
    monkeypatch.setattr(sys, "platform", "linux", raising=False)
    assert server._get_rss_mb() == 2.0


def test_get_rss_mb_darwin(monkeypatch) -> None:
    usage = types.SimpleNamespace(ru_maxrss=2 * 1024 * 1024)
    monkeypatch.setattr(resource, "getrusage", lambda _: usage)
    monkeypatch.setattr(sys, "platform", "darwin", raising=False)
    assert server._get_rss_mb() == 2.0


def test_get_rss_mb_error(monkeypatch) -> None:
    def raise_error(arg):
        raise OSError()

    monkeypatch.setattr(resource, "getrusage", raise_error)
    assert server._get_rss_mb() == 0.0
