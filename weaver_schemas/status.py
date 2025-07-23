from __future__ import annotations

import typing as typ

from msgspec import Struct


class ProjectStatus(Struct, frozen=True):
    """Basic daemon health indicator."""

    message: str
    type: typ.Literal["project-status"] = "project-status"


__all__ = ["ProjectStatus"]
