from __future__ import annotations

import typing as typ

import msgspec as ms


class ProjectStatus(ms.Struct, frozen=True):
    """Health and readiness details for the daemon."""

    pid: int
    rss_mb: float
    ready: bool
    message: str
    type: typ.Literal["project-status"] = "project-status"


__all__ = ["ProjectStatus"]
