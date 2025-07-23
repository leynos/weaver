from __future__ import annotations

import typing as t

import msgspec

from .primitives import Location  # noqa: TC001


class Diagnostic(msgspec.Struct):
    """A compiler or linter message."""

    location: Location
    severity: t.Literal["Error", "Warning", "Info", "Hint"]
    code: str | None
    message: str
    type: t.Literal["diagnostic"] = "diagnostic"


__all__ = ["Diagnostic"]
