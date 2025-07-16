from __future__ import annotations

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
        range=Range(Position(1, 0), Position(1, 1)),
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
