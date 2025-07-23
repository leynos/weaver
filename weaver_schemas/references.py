from __future__ import annotations

import typing as t

import msgspec

from .primitives import Location  # noqa: TC001


class Symbol(msgspec.Struct):
    """A named code symbol."""

    name: str
    kind: str
    location: Location
    type: t.Literal["symbol"] = "symbol"


class Reference(msgspec.Struct):
    """A reference to a symbol."""

    location: Location
    type: t.Literal["reference"] = "reference"


__all__ = ["Reference", "Symbol"]
