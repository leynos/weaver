from __future__ import annotations

import typing as t

from msgspec import Struct

from .primitives import Range  # noqa: TC001


class CodeEdit(Struct):
    """A text replacement within a file."""

    file: str
    range: Range
    new_text: str
    type: t.Literal["edit"] = "edit"


__all__ = ["CodeEdit"]
