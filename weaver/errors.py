from __future__ import annotations

import enum
import logging
import re
import typing as typ

__all__ = ["DependencyErrorCode", "is_dependency_error"]


class DependencyErrorCode(enum.StrEnum):
    """Enumerate dependency-related error codes."""

    MISSING_DEPENDENCY = "MISSING_DEPENDENCY"
    SERENA_AGENT_NOT_FOUND = "SERENA_AGENT_NOT_FOUND"
    DEPENDENCY_UNAVAILABLE = "DEPENDENCY_UNAVAILABLE"
    DEPENDENCY_VERSION_MISMATCH = "DEPENDENCY_VERSION_MISMATCH"


_missing_dep_pattern = re.compile(r"\bmissing dependency\b[:\-]?\s*\w+", re.IGNORECASE)
_serena_agent_pattern = re.compile(
    r"\bserena[- ]agent\b.*(not found|unavailable|missing)", re.IGNORECASE
)
logger = logging.getLogger(__name__)


def is_dependency_error(record: dict[str, typ.Any]) -> bool:
    """Return ``True`` if ``record`` signals a dependency problem."""

    if record.get("type") != "error":
        return False

    code = record.get("error_code") or record.get("code")

    match code:
        case (
            DependencyErrorCode.MISSING_DEPENDENCY
            | DependencyErrorCode.SERENA_AGENT_NOT_FOUND
            | DependencyErrorCode.DEPENDENCY_UNAVAILABLE
            | DependencyErrorCode.DEPENDENCY_VERSION_MISMATCH
        ):
            return True
        case _:
            msg = str(record.get("message", ""))
            if _missing_dep_pattern.search(msg) or _serena_agent_pattern.search(msg):
                return True

    return False
