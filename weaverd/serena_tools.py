"""Utilities for creating Serena tools used by the daemon."""

from __future__ import annotations

import dataclasses
import enum
import typing as typ
from functools import cache
from importlib import import_module

# pyright: reportMissingImports=false  # Serena optional dependency

if typ.TYPE_CHECKING:  # pragma: no cover - import-time only
    from types import ModuleType

    from serena.prompt_factory import SerenaPromptFactory
else:  # pragma: no cover - import-time only

    class SerenaPromptFactory(typ.Protocol):  # type: ignore[reportMissingTypeStubs]
        """Typed placeholder used when Serena is not installed."""


class SerenaTool(enum.StrEnum):
    """Supported Serena tools."""

    ONBOARDING = "OnboardingTool"
    LIST_DIAGNOSTICS = "ListDiagnosticsTool"


@cache
def _serena_modules() -> tuple[ModuleType, type[SerenaPromptFactory]]:
    """Return workflow tools module and prompt factory."""

    try:
        wf_tools = import_module("serena.tools.workflow_tools")
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dep
        msg = "serena-agent is required; install it via 'uv add serena-agent'."
        raise RuntimeError(msg) from exc
    try:
        prompt_mod = import_module("serena.prompt_factory")
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dep
        msg = "serena-agent is required; install it via 'uv add serena-agent'."
        raise RuntimeError(msg) from exc
    return wf_tools, prompt_mod.SerenaPromptFactory


class SerenaToolProtocol(typ.Protocol):
    """Expected interface for Serena tools."""

    def run(self, *args: object, **kwargs: object) -> object: ...
    def __getattr__(self, name: str) -> typ.Any: ...


def _resolve_tool_name(tool_attr: SerenaTool | str) -> str:
    """Normalise tool identifiers to workflow attribute names."""

    if isinstance(tool_attr, SerenaTool):
        return tool_attr.value
    if isinstance(tool_attr, str):
        try:
            # Allow enum member names such as "ONBOARDING".
            return SerenaTool[tool_attr].value
        except KeyError:
            return tool_attr
    raise TypeError("tool_attr must be SerenaTool or str")


@dataclasses.dataclass(frozen=True, slots=True)
class _Agent:
    """Minimal agent providing only the prompt factory."""

    prompt_factory: SerenaPromptFactory


def create_serena_tool(tool_attr: SerenaTool | str) -> SerenaToolProtocol:
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
    name = _resolve_tool_name(tool_attr)

    tool_cls = getattr(wf_tools, name, None)
    if tool_cls is None:  # pragma: no cover - optional dep
        raise RuntimeError(f"serena.tools.workflow_tools.{name} not found")
    if not callable(tool_cls):  # pragma: no cover - defensive
        raise RuntimeError(f"serena.tools.workflow_tools.{name} is not callable")

    agent = _Agent(prompt_factory_cls())
    return typ.cast("SerenaToolProtocol", tool_cls(agent))
