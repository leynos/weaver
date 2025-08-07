from __future__ import annotations

"""Utilities for creating Serena tools used by the daemon."""

from importlib import import_module  # noqa: E402 - import below module docstring

# pyright: reportMissingImports=false  # Serena optional dependency


class _BareAgent:
    """Minimal agent providing only the prompt factory."""

    def __init__(self, prompt_factory) -> None:
        self.prompt_factory = prompt_factory


def _load_serena_tool(tool_attr: str):
    """Return the requested Serena tool class and prompt factory.

    Raises:
        RuntimeError: if the ``serena-agent`` package is missing or the tool
        attribute cannot be imported.
    """
    try:
        wf_tools = import_module("serena.tools.workflow_tools")
        prompt_mod = import_module("serena.prompt_factory")
    except ModuleNotFoundError as exc:  # pragma: no cover - optional dep
        msg = "serena-agent is required; install it via 'uv add serena-agent'."
        raise RuntimeError(msg) from exc

    tool_cls = getattr(wf_tools, tool_attr, None)
    if tool_cls is None:  # pragma: no cover - optional dep
        raise RuntimeError(f"{tool_attr} not found in serena")

    return tool_cls, prompt_mod.SerenaPromptFactory


def create_serena_tool(tool_attr: str):
    """Instantiate a Serena tool given its attribute name."""

    tool_cls, prompt_factory = _load_serena_tool(tool_attr)
    return tool_cls(_BareAgent(prompt_factory()))
