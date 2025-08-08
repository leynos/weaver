"""Utilities for creating Serena tools used by the daemon."""

from __future__ import annotations

import enum
import typing as typ
from functools import cache
from importlib import import_module
from types import SimpleNamespace

# pyright: reportMissingImports=false  # Serena optional dependency


class SerenaTool(str, enum.Enum):
    """Supported Serena tools."""

    ONBOARDING = "OnboardingTool"
    LIST_DIAGNOSTICS = "ListDiagnosticsTool"


@cache
def _serena_modules() -> tuple[typ.Any, typ.Any]:
    """Return workflow tools module and prompt factory."""

    try:
        wf_tools = import_module("serena.tools.workflow_tools")
        prompt_mod = import_module("serena.prompt_factory")
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dep
        msg = "serena-agent is required; install it via 'uv add serena-agent'."
        raise RuntimeError(msg) from exc
    return wf_tools, prompt_mod.SerenaPromptFactory


def create_serena_tool(tool_attr: SerenaTool | str) -> typ.Any:
    """Instantiate a Serena tool.

    Accepts:
      - ``SerenaTool`` enum member, e.g. ``SerenaTool.ONBOARDING``
      - ``str``: either the enum member name (``"ONBOARDING"``) or the
        ``serena.tools.workflow_tools`` attribute name (``"OnboardingTool"``).

    Raises:
      ``RuntimeError`` if the tool class is not found or not callable.
      ``TypeError`` if ``tool_attr`` is neither ``SerenaTool`` nor ``str``.
    """

    wf_tools, prompt_factory_cls = _serena_modules()
    if isinstance(tool_attr, SerenaTool):
        name = tool_attr.value
    elif isinstance(tool_attr, str):
        try:
            # Allow enum member names such as "ONBOARDING".
            name = SerenaTool[tool_attr].value
        except KeyError:
            name = tool_attr
    else:
        raise TypeError("tool_attr must be SerenaTool or str")

    tool_cls = getattr(wf_tools, name, None)
    if tool_cls is None:  # pragma: no cover - optional dep
        raise RuntimeError(f"serena.tools.workflow_tools.{name} not found")
    if not callable(tool_cls):  # pragma: no cover - defensive
        raise RuntimeError(f"serena.tools.workflow_tools.{name} is not callable")

    agent = SimpleNamespace(prompt_factory=prompt_factory_cls())
    return tool_cls(agent)
