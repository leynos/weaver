"""Frame Cargo's bounded verbose commands for this diagnostic's routing check.

Only macos_caller_boundary_support and its preparation contracts consume this
private module. Commands are evidence, never shell input to execute. Record
byte bounds include raw presentation bytes, delimiters and physical newlines.
"""

from __future__ import annotations

import re
import shlex
from dataclasses import dataclass, field
from pathlib import Path

RECORD_BYTE_LIMIT = 64 * 1024
RECORD_LINE_LIMIT = 256
ANSI_PRESENTATION = re.compile(r"\x1b\[[0-9;]*m")
RUNNING = re.compile(r"^[ \t]*Running[ \t]+(.*)$", re.DOTALL)
ASSIGNMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=")


class CargoRecordError(ValueError):
    """Cargo evidence has malformed, ambiguous or oversized command framing."""


@dataclass
class _Frame:
    """Keep shell quote state separate from Cargo's outer display delimiter."""

    start: int
    quote: str = ""
    escaped: bool = False
    size: int = 0
    lines: int = 0
    parts: list[str] = field(default_factory=list)

    def append(self, text: str, raw: str) -> str | None:
        """Return the complete command only at an unquoted outer backtick.

        For example, ``_Frame(1).append("rustc`", "Running `rustc`")``
        returns ``"rustc"``; a backtick inside shell quotes remains data.
        """
        self.size += len(raw.encode("utf-8"))
        self.lines += 1
        if self.size > RECORD_BYTE_LIMIT or self.lines > RECORD_LINE_LIMIT:
            raise CargoRecordError(
                f"Cargo Running record at line {self.start} exceeds framing limits "
                f"({self.size} bytes, {self.lines} physical lines)"
            )
        for offset, char in enumerate(text):
            if self.escaped:
                self.escaped = False
            elif self.quote == "'":
                if char == "'":
                    self.quote = ""
            elif char == "\\":
                self.escaped = True
            elif self.quote == '"':
                if char == '"':
                    self.quote = ""
            elif char in ("'", '"'):
                self.quote = char
            elif char == "`":
                if text[offset + 1:].strip():
                    raise CargoRecordError("ambiguous Cargo outer display delimiter")
                return "".join(self.parts) + text[:offset]
        self.parts.append(text)
        return None


@dataclass(frozen=True)
class CargoCommand:
    """A tokenized logical command and its inclusive physical source range."""

    tokens: tuple[str, ...]
    start_line: int
    end_line: int


def command_records(text: str) -> list[CargoCommand]:
    """Frame and tokenize every anchored Running record, refusing partial data.

    For example, ``command_records("Running `rustc`\n")[0].tokens`` is
    ``("rustc",)`` and its inclusive source range is line 1 through line 1.
    """
    records = []
    pending: _Frame | None = None
    for number, raw in enumerate(text.splitlines(keepends=True), 1):
        line = ANSI_PRESENTATION.sub("", raw)
        if "\x1b" in line:
            raise CargoRecordError("unrecognized ANSI presentation in Cargo evidence")
        marker = RUNNING.match(line)
        if marker:
            if pending is not None:
                raise CargoRecordError("incomplete Cargo record before next Running marker")
            body = marker.group(1)
            if not body.startswith("`"):
                raise CargoRecordError("Cargo Running record lacks outer display delimiter")
            pending = _Frame(number)
            line = body[1:]
        if pending is None:
            continue
        command = pending.append(line, raw)
        if command is not None:
            try:
                tokens = tuple(shlex.split(command.replace("\x1f", " ")))
            except ValueError as error:
                raise CargoRecordError("Cargo command has malformed shell quoting") from error
            if not tokens:
                raise CargoRecordError("Cargo Running record has no executable")
            records.append(CargoCommand(tokens, pending.start, number))
            pending = None
    if pending is not None:
        raise CargoRecordError("incomplete Cargo record at EOF")
    return records


def compiler_arguments(record: CargoCommand, wrappers: set[str]) -> tuple[str, ...] | None:
    """Locate rustc after environment assignments and an observed wrapper only.

    For example, ``CargoCommand(("sccache", "rustc", "--version"), 1, 1)``
    yields ``("--version",)`` when ``wrappers`` contains ``"sccache"``.
    """
    tokens = record.tokens
    index = 0
    while index < len(tokens) and ASSIGNMENT.match(tokens[index]):
        index += 1
    if index == len(tokens):
        raise CargoRecordError("Cargo command has environment assignments but no executable")
    executable = Path(tokens[index]).name
    if executable in wrappers:
        index += 1
        if index == len(tokens) or Path(tokens[index]).name != "rustc":
            raise CargoRecordError("observed compiler wrapper does not precede rustc")
        executable = "rustc"
    if executable != "rustc":
        if any(Path(token).name == "rustc" for token in tokens[index + 1:]):
            raise CargoRecordError("rustc appears behind an unobserved executable prefix")
        return None
    return tokens[index + 1:]


def crate_name(arguments: tuple[str, ...]) -> str | None:
    """Select the compiler's exact crate-name option, refusing ambiguous values.

    For example, ``crate_name(("--crate-name", "dependency"))`` returns
    ``"dependency"``; a metadata substring cannot select the probe.
    """
    names = []
    for index, token in enumerate(arguments):
        if token == "--crate-name":
            if index + 1 == len(arguments) or arguments[index + 1].startswith("-"):
                raise CargoRecordError("rustc crate-name option has no value")
            names.append(arguments[index + 1])
        elif token.startswith("--crate-name="):
            names.append(token.partition("=")[2])
    if len(names) > 1 or names == [""]:
        raise CargoRecordError("rustc crate-name option is ambiguous")
    return names[0] if names else None
