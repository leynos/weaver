from __future__ import annotations

import typing as t

import msgspec


class SchemaError(msgspec.Struct):
    """A structured error message."""

    message: str
    type: t.Literal["error"] = "error"


__all__ = ["SchemaError"]
