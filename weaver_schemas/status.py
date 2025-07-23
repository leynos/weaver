from __future__ import annotations

import typing as typ

import msgspec


class ProjectStatus(msgspec.Struct, frozen=True):
    """Basic daemon health indicator."""

    message: str
    type: typ.Literal["project-status"] = "project-status"


__all__ = ["ProjectStatus"]
