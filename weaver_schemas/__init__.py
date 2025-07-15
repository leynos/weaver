from __future__ import annotations

import typing as t

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


class Diagnostic(Struct):
    """A compiler or linter message."""

    location: Location
    severity: t.Literal["Error", "Warning", "Info", "Hint"]
    code: str | None
    message: str
    type: t.Literal["diagnostic"] = "diagnostic"


class Symbol(Struct):
    """A named code symbol."""

    name: str
    kind: str
    location: Location
    type: t.Literal["symbol"] = "symbol"


class Reference(Struct):
    """A reference to a symbol."""

    location: Location
    type: t.Literal["reference"] = "reference"


class CodeEdit(Struct):
    """A text replacement within a file."""

    file: str
    range: Range
    new_text: str
    type: t.Literal["edit"] = "edit"


class ImpactReport(Struct):
    """Result of analysing a proposed change."""

    diagnostics: list[Diagnostic]
    type: t.Literal["impact"] = "impact"


class TestResult(Struct):
    """Outcome of a project test run."""

    name: str
    status: t.Literal["passed", "failed", "error", "skipped"]
    output: str | None = None
    type: t.Literal["test-result"] = "test-result"


class OnboardingReport(Struct):
    """Information gathered during project onboarding."""

    details: str
    type: t.Literal["onboarding"] = "onboarding"


class Error(Struct):
    """A structured error message."""

    message: str
    type: t.Literal["error"] = "error"


__all__ = [
    "CodeEdit",
    "Diagnostic",
    "Error",
    "ImpactReport",
    "Location",
    "OnboardingReport",
    "Position",
    "Range",
    "Reference",
    "Symbol",
    "TestResult",
]
