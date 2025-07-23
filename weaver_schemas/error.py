from __future__ import annotations

import typing as typ

from msgspec import Struct


class SchemaError(Struct):
    """A structured error message."""

    message: str
    type: typ.Literal["error"] = "error"


__all__ = ["SchemaError"]
