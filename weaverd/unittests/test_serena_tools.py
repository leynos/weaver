from __future__ import annotations

import pytest

from weaverd.serena_tools import _resolve_string_tool_name, _resolve_tool_name


def test_resolve_string_tool_name_enum_case_insensitive() -> None:
    assert _resolve_string_tool_name("onboarding") == "OnboardingTool"


def test_resolve_string_tool_name_direct_value() -> None:
    assert _resolve_string_tool_name("OnboardingTool") == "OnboardingTool"


def test_resolve_string_tool_name_unknown() -> None:
    with pytest.raises(RuntimeError, match="Unknown Serena tool 'Nope'"):
        _resolve_string_tool_name("Nope")


def test_resolve_tool_name_type_error() -> None:
    with pytest.raises(TypeError):
        _resolve_tool_name(123)  # type: ignore[arg-type]
