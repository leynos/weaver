from __future__ import annotations

import msgspec as ms


class Position(ms.Struct, frozen=True):
    """A point in a text file."""

    line: int
    character: int


class Range(ms.Struct, frozen=True):
    """A span of text between two positions."""

    start: Position
    end: Position


class Location(ms.Struct, frozen=True):
    """A range of text within a file."""

    file: str
    range: Range


__all__ = ["Location", "Position", "Range"]
