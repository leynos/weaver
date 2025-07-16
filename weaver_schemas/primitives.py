from __future__ import annotations

from msgspec import Struct


class Position(Struct):
    """A point in a text file."""

    line: int
    character: int


class Range(Struct):
    """A span of text between two positions."""

    start: Position
    end: Position


class Location(Struct):
    """A range of text within a file."""

    file: str
    range: Range


__all__ = ["Location", "Position", "Range"]
