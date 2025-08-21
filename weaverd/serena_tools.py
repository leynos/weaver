"""Utilities for creating Serena tools used by the daemon."""

from __future__ import annotations

import enum
import sys
import typing as typ
from contextlib import suppress
from importlib import import_module, invalidate_caches

# pyright: reportMissingImports=false  # Serena optional dependency

if typ.TYPE_CHECKING:  # pragma: no cover - import-time only
    import collections.abc as cabc
    from types import ModuleType

    from serena.prompt_factory import SerenaPromptFactory
else:  # pragma: no cover - import-time only

    class SerenaPromptFactory(typ.Protocol):  # type: ignore[reportMissingTypeStubs]
        """Typed placeholder used when Serena is not installed."""


# Opaque tool instance returned by Serena; interface comes from serena-agent.
# We expose it as ``object`` to avoid ``Any`` in the public API while keeping it
# flexible.
SerenaToolInstance: typ.TypeAlias = object


class SerenaAgentNotFoundError(RuntimeError):
    """Raised when the optional Serena dependency is missing."""

    def __init__(self) -> None:
        super().__init__("serena-agent not found")


class ToolClassNotFoundError(RuntimeError):
    """Raised when a workflow tool class cannot be located."""

    def __init__(self, name: str) -> None:
        super().__init__(f"serena.tools.workflow_tools.{name} not found")


class ToolClassNotCallableError(TypeError):
    """Raised when a workflow tool attribute is not callable."""

    def __init__(self, name: str) -> None:
        super().__init__(f"serena.tools.workflow_tools.{name} is not callable")


class PromptFactoryError(TypeError):
    """Raised when ``SerenaPromptFactory`` is missing or invalid."""

    def __init__(self) -> None:
        super().__init__(
            "serena.prompt_factory.SerenaPromptFactory not found or not a type",
        )


class ToolInstantiationError(RuntimeError):
    """Raised when a tool fails to instantiate with the agent."""

    def __init__(self, name: str, exc: Exception) -> None:
        super().__init__(
            f"Failed to instantiate serena.tools.workflow_tools.{name}: {exc}",
        )


class ToolAttrTypeError(TypeError):
    """Raised when ``tool_attr`` is neither ``SerenaTool`` nor ``str``."""

    def __init__(self) -> None:
        super().__init__("tool_attr must be SerenaTool or str")


class UnknownSerenaToolError(RuntimeError):
    """Raised when a requested Serena tool does not exist."""

    def __init__(self, tool_name: str, valid: cabc.Collection[str]) -> None:
        super().__init__(
            f"Unknown Serena tool '{tool_name}'. Expected one of: {', '.join(valid)}",
        )


class SerenaTool(enum.StrEnum):
    """Supported Serena tools."""

    ONBOARDING = "OnboardingTool"
    LIST_DIAGNOSTICS = "ListDiagnosticsTool"
    GET_DEFINITION = "GetDefinitionTool"
    LIST_REFERENCES = "ListReferencesTool"


_VALID_TOOL_MEMBER_NAMES = frozenset(SerenaTool.__members__.keys())
_VALID_TOOL_VALUES = frozenset(t.value for t in SerenaTool)


def clear_serena_imports() -> None:
    """Remove cached Serena modules.

    Tests use this helper to force ``import_module`` to attempt a fresh import.
    The standard ``sys.modules`` cache is cleared for the Serena modules used by
    :func:`create_serena_tool`.

    This helper mutates ``sys.modules`` without any locking and is therefore
    not thread-safe. It is intended for single-threaded test contexts only.
    """

    # Optional: ensure import finders don't use stale filesystem caches.
    with suppress(Exception):  # pragma: no cover - best effort
        invalidate_caches()
    for name in (
        "serena.tools.workflow_tools",
        "serena.prompt_factory",
        "serena.tools",
        "serena",
    ):
        sys.modules.pop(name, None)


def _validate_and_get_tool_class(wf_tools: ModuleType, name: str) -> typ.Any:
    """Return the workflow tool class, ensuring it exists and is callable."""

    tool_cls = getattr(wf_tools, name, None)
    if tool_cls is None:
        raise ToolClassNotFoundError(name)
    if not callable(tool_cls):
        raise ToolClassNotCallableError(name)
    return tool_cls


def _create_agent_with_prompt_factory(prompt_mod: ModuleType) -> _BareAgent:
    """Create an agent using ``SerenaPromptFactory`` from ``prompt_mod``."""

    # Assumption: SerenaPromptFactory can be instantiated without arguments.
    prompt_factory_attr = getattr(prompt_mod, "SerenaPromptFactory", None)
    if not isinstance(prompt_factory_attr, type):
        raise PromptFactoryError()
    prompt_factory_cls = typ.cast("type[SerenaPromptFactory]", prompt_factory_attr)
    return _BareAgent(prompt_factory_cls())


def _instantiate_tool(
    tool_cls: typ.Any, agent: _BareAgent, name: str
) -> SerenaToolInstance:
    """Instantiate ``tool_cls`` with ``agent`` and wrap errors."""

    try:
        return tool_cls(agent)
    except Exception as exc:  # pragma: no cover - defensive
        raise ToolInstantiationError(name, exc) from exc


def create_serena_tool(tool_attr: SerenaTool | str) -> SerenaToolInstance:
    """Instantiate a Serena tool.

    Parameters
    ----------
    tool_attr:
        ``SerenaTool`` enum member or string. Enum member names are
        case-insensitive; raw class names are also accepted.

    Returns
    -------
    SerenaToolInstance
        Instantiated Serena tool.

    Raises
    ------
    RuntimeError
        If the tool class is not found, not callable or ``tool_attr`` is an
        unknown string.
    TypeError
        If ``tool_attr`` is neither ``SerenaTool`` nor ``str``.
    """
    wf_tools, prompt_mod = _load_serena_modules()
    name = _resolve_tool_name(tool_attr)
    tool_cls = _validate_and_get_tool_class(wf_tools, name)
    return _instantiate_tool(
        tool_cls,
        _create_agent_with_prompt_factory(prompt_mod),
        name,
    )


def _resolve_tool_name(tool_attr: SerenaTool | str) -> str:
    """Resolve ``tool_attr`` to the actual workflow tool class name."""

    if isinstance(tool_attr, SerenaTool):
        return tool_attr.value

    if not isinstance(tool_attr, str):
        raise ToolAttrTypeError()

    return _resolve_string_tool_name(tool_attr)


def _resolve_string_tool_name(tool_name: str) -> str:
    """Resolve ``tool_name`` to the actual workflow tool class name."""

    upper = tool_name.upper()
    if upper in _VALID_TOOL_MEMBER_NAMES:
        return SerenaTool[upper].value
    if tool_name in _VALID_TOOL_VALUES:
        return tool_name
    valid = sorted({*_VALID_TOOL_MEMBER_NAMES, *_VALID_TOOL_VALUES})
    raise UnknownSerenaToolError(tool_name, valid)


def _import_serena_module(module_name: str) -> ModuleType:
    """Import a Serena module with consistent error handling."""
    try:
        return import_module(module_name)
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dep
        if getattr(exc, "name", "") and str(exc.name).startswith("serena"):
            raise SerenaAgentNotFoundError() from exc
        raise


def _load_serena_modules() -> tuple[ModuleType, ModuleType]:
    """Load the Serena workflow tools and prompt factory modules."""

    wf_tools = _import_serena_module("serena.tools.workflow_tools")
    prompt_mod = _import_serena_module("serena.prompt_factory")
    return wf_tools, prompt_mod


class _BareAgent:
    """Minimal agent providing only the prompt factory."""

    __slots__ = ("prompt_factory",)

    def __init__(self, prompt_factory: SerenaPromptFactory) -> None:
        self.prompt_factory = prompt_factory

    def __repr__(self) -> str:  # pragma: no cover - debug helper
        return f"_BareAgent(prompt_factory={type(self.prompt_factory).__name__})"
