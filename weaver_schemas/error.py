from __future__ import annotations

import typing as t

from msgspec import Struct


class SchemaError(Struct):
    """A structured error message."""

    message: str
    type: t.Literal["error"] = "error"


__all__ = ["SchemaError"]
