from __future__ import annotations

import typing as t

from msgspec import Struct


class ProjectStatus(Struct, frozen=True):
    """Basic daemon health indicator."""

    message: str
    type: t.Literal["project-status"] = "project-status"


__all__ = ["ProjectStatus"]
