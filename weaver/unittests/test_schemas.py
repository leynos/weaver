from __future__ import annotations

from msgspec import json

from weaver_schemas import Diagnostic, Location, Position, Range


def make_example() -> Diagnostic:
    loc = Location(file="foo.py", range=Range(start=Position(1, 0), end=Position(1, 3)))
    return Diagnostic(location=loc, severity="Error", code="E123", message="oh no")


def test_json_roundtrip() -> None:
    diagnostic = make_example()
    data = json.encode(diagnostic)
    decoded = json.decode(data, type=Diagnostic)
    assert decoded == diagnostic


def test_make_example() -> None:
    diag = make_example()
    assert diag.message == "oh no"
