from __future__ import annotations

import typing as typ

import pytest
from msgspec import json

from weaver_schemas import (
    CodeEdit,
    Diagnostic,
    ImpactReport,
    Location,
    Position,
    Range,
)
from weaver_schemas import (
    TestResult as SchemaTestResult,
)


def make_diagnostic() -> Diagnostic:
    loc = Location(file="foo.py", range=Range(start=Position(1, 0), end=Position(1, 3)))
    return Diagnostic(location=loc, severity="Error", code="E123", message="oh no")


def test_diagnostic_roundtrip() -> None:
    diag = make_diagnostic()
    data = json.encode(diag)
    assert json.decode(data, type=Diagnostic) == diag


def test_codeedit_roundtrip() -> None:
    edit = CodeEdit(
        file="foo.py",
        range=Range(start=Position(1, 0), end=Position(1, 1)),
        new_text="bar",
    )
    data = json.encode(edit)
    assert json.decode(data, type=CodeEdit) == edit


def test_impact_report_roundtrip() -> None:
    report = ImpactReport(diagnostics=[make_diagnostic()])
    data = json.encode(report)
    assert json.decode(data, type=ImpactReport) == report


def test_test_result_roundtrip() -> None:
    result = SchemaTestResult(name="pytest", status="passed", output="ok")
    data = json.encode(result)
    assert json.decode(data, type=SchemaTestResult) == result


def test_impact_report_empty_list() -> None:
    report = ImpactReport(diagnostics=[])
    data = json.encode(report)
    assert json.decode(data, type=ImpactReport) == report


def test_test_result_none_output() -> None:
    result = SchemaTestResult(name="pytest", status="failed", output=None)
    data = json.encode(result)
    assert json.decode(data, type=SchemaTestResult) == result


@pytest.mark.parametrize("severity", ["Error", "Warning", "Info", "Hint"])
def test_diagnostic_severity_roundtrip(
    severity: typ.Literal["Error", "Warning", "Info", "Hint"],
) -> None:
    diag = Diagnostic(
        location=Location(
            file="foo.py",
            range=Range(start=Position(2, 0), end=Position(2, 1)),
        ),
        severity=severity,
        code=None,
        message="msg",
    )
    data = json.encode(diag)
    assert json.decode(data, type=Diagnostic) == diag


@pytest.mark.parametrize("status", ["passed", "failed", "error", "skipped"])
def test_test_result_status_roundtrip(
    status: typ.Literal["passed", "failed", "error", "skipped"],
) -> None:
    result = SchemaTestResult(name="pytest", status=status)
    data = json.encode(result)
    assert json.decode(data, type=SchemaTestResult) == result
