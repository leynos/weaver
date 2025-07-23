from __future__ import annotations

import typing as typ

import msgspec

from .diagnostics import Diagnostic  # noqa: TC001


class ImpactReport(msgspec.Struct, frozen=True):
    """Result of analysing a proposed change."""

    diagnostics: tuple[Diagnostic, ...]
    type: typ.Literal["impact"] = "impact"


class TestResult(msgspec.Struct):
    """Outcome of a project test run."""

    name: str
    status: typ.Literal["passed", "failed", "error", "skipped"]
    output: str | None = None
    type: typ.Literal["test-result"] = "test-result"


class OnboardingReport(msgspec.Struct):
    """Information gathered during project onboarding."""

    details: str
    type: typ.Literal["onboarding"] = "onboarding"


__all__ = ["ImpactReport", "OnboardingReport", "TestResult"]
