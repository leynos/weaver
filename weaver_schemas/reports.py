from __future__ import annotations

import typing as t

from msgspec import Struct

from .diagnostics import Diagnostic  # noqa: TC001


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


__all__ = ["ImpactReport", "OnboardingReport", "TestResult"]
