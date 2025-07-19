"""Shared msgspec models for the weaver API."""

from __future__ import annotations

from .diagnostics import Diagnostic
from .edits import CodeEdit
from .error import SchemaError
from .primitives import Location, Position, Range
from .references import Reference, Symbol
from .reports import ImpactReport, OnboardingReport, TestResult
from .status import ProjectStatus

__all__ = [
    "CodeEdit",
    "Diagnostic",
    "ImpactReport",
    "Location",
    "OnboardingReport",
    "Position",
    "ProjectStatus",
    "Range",
    "Reference",
    "SchemaError",
    "Symbol",
    "TestResult",
]
