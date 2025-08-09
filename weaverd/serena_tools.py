"""Utilities for creating Serena tools used by the daemon."""

from __future__ import annotations

import enum
import sys
import typing as typ
from importlib import import_module

# pyright: reportMissingImports=false  # Serena optional dependency

if typ.TYPE_CHECKING:  # pragma: no cover - import-time only
    from serena.prompt_factory import SerenaPromptFactory
else:  # pragma: no cover - import-time only

    class SerenaPromptFactory(typ.Protocol):  # type: ignore[reportMissingTypeStubs]
        """Typed placeholder used when Serena is not installed."""


class SerenaTool(enum.StrEnum):
    """Supported Serena tools."""

    ONBOARDING = "OnboardingTool"
    LIST_DIAGNOSTICS = "ListDiagnosticsTool"


def clear_serena_imports() -> None:
    """Remove cached Serena modules.

    Tests use this helper to force ``import_module`` to attempt a fresh import.
    The standard ``sys.modules`` cache is cleared for the Serena modules used by
    :func:`create_serena_tool`.
    """

    for name in ("serena.tools.workflow_tools", "serena.prompt_factory"):
        sys.modules.pop(name, None)


def create_serena_tool(tool_attr: SerenaTool | str) -> typ.Any:
    """Instantiate a Serena tool from its enum member or raw attribute name.

    Accepts:
      - ``SerenaTool`` enum member, e.g. ``SerenaTool.ONBOARDING``
      - ``str``: either the enum member name (case-insensitive, ``"ONBOARDING"``)
        or the ``serena.tools.workflow_tools`` attribute name
        (``"OnboardingTool"``).

    Raises:
      ``RuntimeError`` if the tool class is not found or not callable.
      ``RuntimeError`` if ``tool_attr`` is an unknown string.
      ``TypeError`` if ``tool_attr`` is neither ``SerenaTool`` nor ``str``.
    """

    try:
        wf_tools = import_module("serena.tools.workflow_tools")
        prompt_mod = import_module("serena.prompt_factory")
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dep
        msg = "serena-agent is required; install it via 'uv add serena-agent'."
        raise RuntimeError(msg) from exc

    if isinstance(tool_attr, SerenaTool):
        name = tool_attr.value
    elif isinstance(tool_attr, str):
        upper = tool_attr.upper()
        if upper in SerenaTool.__members__:
            name = SerenaTool[upper].value
        elif tool_attr in {t.value for t in SerenaTool}:
            name = tool_attr
        else:
            raise RuntimeError(f"Unknown Serena tool '{tool_attr}'")
    else:
        raise TypeError("tool_attr must be SerenaTool or str")

    tool_cls = getattr(wf_tools, name, None)
    if tool_cls is None:
        raise RuntimeError(f"serena.tools.workflow_tools.{name} not found")
    if not callable(tool_cls):
        raise RuntimeError(f"serena.tools.workflow_tools.{name} is not callable")

    return tool_cls(_BareAgent(prompt_mod.SerenaPromptFactory()))


class _BareAgent:
    """Minimal agent providing only the prompt factory."""

    def __init__(self, prompt_factory: typ.Any) -> None:
        self.prompt_factory = prompt_factory
