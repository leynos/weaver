from __future__ import annotations

import typing as t

import msgspec

from .primitives import Range  # noqa: TC001


class CodeEdit(msgspec.Struct):
    """A text replacement within a file."""

    file: str
    range: Range
    new_text: str
    type: t.Literal["edit"] = "edit"


__all__ = ["CodeEdit"]
