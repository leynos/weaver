from __future__ import annotations

import msgspec


class Position(msgspec.Struct):
    """A point in a text file."""

    line: int
    character: int


class Range(msgspec.Struct):
    """A span of text between two positions."""

    start: Position
    end: Position


class Location(msgspec.Struct):
    """A range of text within a file."""

    file: str
    range: Range


__all__ = ["Location", "Position", "Range"]
