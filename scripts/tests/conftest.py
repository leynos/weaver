"""Shared fixtures for Weaver's spelling refresh tests."""

from __future__ import annotations

import importlib
import types
from pathlib import Path

import pytest

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]


@pytest.fixture(name="rollout_modules")
def rollout_modules_fixture(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[types.ModuleType, types.ModuleType, types.ModuleType]:
    """Import the three refresh modules through their runtime paths.

    Returns
    -------
    tuple[types.ModuleType, types.ModuleType, types.ModuleType]
        Cache, HTTP refresh and rollout modules in dependency order.
    """
    monkeypatch.syspath_prepend(str(SCRIPT_DIRECTORY))
    names = ("typos_rollout_cache", "typos_rollout_http", "typos_rollout")
    importlib.invalidate_caches()
    cache, refresh, rollout = (importlib.import_module(name) for name in names)
    return cache, refresh, rollout
