from __future__ import annotations

import types as _types


def make_fake_module(name: str, **attrs: object) -> _types.ModuleType:
    """Return a simple module with ``attrs`` assigned."""
    mod = _types.ModuleType(name)
    for key, value in attrs.items():
        setattr(mod, key, value)
    return mod
