from __future__ import annotations

import typing as typ

from msgspec import Struct

from .primitives import Location  # noqa: TC001


class Symbol(Struct):
    """A named code symbol."""

    name: str
    kind: str
    location: Location
    type: typ.Literal["symbol"] = "symbol"


class Reference(Struct):
    """A reference to a symbol."""

    location: Location
    type: typ.Literal["reference"] = "reference"


__all__ = ["Reference", "Symbol"]
