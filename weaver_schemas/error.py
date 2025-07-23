from __future__ import annotations

import typing as typ

import msgspec as ms


class SchemaError(ms.Struct, frozen=True):
    """A structured error message."""

    message: str
    type: typ.Literal["error"] = "error"


__all__ = ["SchemaError"]
