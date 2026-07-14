"""Validate shared spelling authorities and narrow local exceptions.

Local regexes must not match empty or generic prose, and local file globs must
not exclude every Markdown path.
"""

from __future__ import annotations

import re
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

SCHEMA_VERSION = 1
REQUIRED_AUTHORITY_FIELDS = (
    ("oxford", "stems"),
    ("words", "accepted"),
    ("words", "corrections"),
    ("phrases", "corrections"),
    ("patterns", "ignore"),
    ("files", "exclude"),
)
GENERIC_PROSE = ("ordinary prose", "unrelated_identifier")
UNIVERSAL_FILE_GLOBS = frozenset({"*", "**", "**/*", "*.md", "**/*.md"})


def validate_document(
    document: cabc.Mapping[str, object],
    *,
    sparse: bool,
) -> None:
    """Validate schema identity and required shared-authority fields."""
    schema = document.get("schema")
    is_valid_schema = (
        isinstance(schema, int)
        and not isinstance(schema, bool)
        and schema == SCHEMA_VERSION
    )
    if not is_valid_schema:
        message = f"unsupported dictionary schema {schema!r}"
        raise ValueError(message)
    if sparse:
        return
    for table_name, field_name in REQUIRED_AUTHORITY_FIELDS:
        if table_name not in document:
            message = f"missing required table {table_name!r}"
            raise ValueError(message)
        table = document[table_name]
        if isinstance(table, dict) and field_name not in table:
            message = f"missing required field {table_name}.{field_name}"
            raise ValueError(message)


def validate_local_exceptions(
    ignore_patterns: tuple[str, ...],
    excluded_files: tuple[str, ...],
) -> None:
    """Reject local exceptions that can match generic prose or all Markdown."""
    for pattern in ignore_patterns:
        compiled = re.compile(pattern)
        if compiled.search("") is not None or any(
            compiled.fullmatch(probe) for probe in GENERIC_PROSE
        ):
            message = f"local ignore pattern is too broad: {pattern!r}"
            raise ValueError(message)
    for pattern in excluded_files:
        if pattern in UNIVERSAL_FILE_GLOBS:
            message = f"local file exclusion is too broad: {pattern!r}"
            raise ValueError(message)
