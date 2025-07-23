from __future__ import annotations

import typing as typ

from msgspec import Struct

from .primitives import Location  # noqa: TC001


class Diagnostic(Struct):
    """A compiler or linter message."""

    location: Location
    severity: typ.Literal["Error", "Warning", "Info", "Hint"]
    code: str | None
    message: str
    type: typ.Literal["diagnostic"] = "diagnostic"


__all__ = ["Diagnostic"]
