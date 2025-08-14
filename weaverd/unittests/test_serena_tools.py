from __future__ import annotations

import pytest

from weaverd.serena_tools import (
    SerenaTool,
    _resolve_string_tool_name,
    _resolve_tool_name,
)


@pytest.mark.parametrize(
    ("input_name", "expected"),
    [
        ("onboarding", "OnboardingTool"),
        ("OnboardingTool", "OnboardingTool"),
    ],
)
def test_resolve_string_tool_name_success(input_name: str, expected: str) -> None:
    assert _resolve_string_tool_name(input_name) == expected


def test_resolve_string_tool_name_unknown() -> None:
    with pytest.raises(RuntimeError, match="Unknown Serena tool 'Nope'"):
        _resolve_string_tool_name("Nope")


def test_resolve_tool_name_type_error() -> None:
    with pytest.raises(TypeError):
        _resolve_tool_name(123)  # type: ignore[arg-type]


def test_resolve_tool_name_from_enum() -> None:
    assert _resolve_tool_name(SerenaTool.ONBOARDING) == "OnboardingTool"
