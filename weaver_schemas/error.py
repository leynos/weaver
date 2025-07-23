from __future__ import annotations

import typing as typ

import msgspec


class SchemaError(msgspec.Struct):
    """A structured error message."""

    message: str
    type: typ.Literal["error"] = "error"


__all__ = ["SchemaError"]
