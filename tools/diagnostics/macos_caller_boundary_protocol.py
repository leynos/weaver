"""Private fixed-protocol parser for the macOS caller-boundary diagnostic.

Only the owning controller and its named private contract test import these
records and checks. This module does not define a reusable protocol or permit
additional labels or event variants.
"""

from __future__ import annotations

import re
from typing import Any

from macos_caller_boundary_support import ProbeError

SECRET = b"weaver-caller-boundary-secret-v1\n"
CONTROL = b"weaver-caller-boundary-control-v1\n"
MARKER = b"harmless-host-marker"
DENIAL_CODES = {1, 13}


def parse_records(stdout: bytes) -> list[str]:
    try:
        text = stdout.decode("ascii")
    except UnicodeDecodeError as error:
        raise ProbeError("caller stdout is not ASCII protocol data") from error
    lines = text.splitlines(keepends=True)
    if any(not line.endswith("\n") or len(line) > 4096 for line in lines):
        raise ProbeError("caller emitted a partial or oversized protocol record")
    return [line[:-1] for line in lines]


def parse_observation(line: str) -> dict[str, Any]:
    fields = line.split("\t")
    if len(fields) == 4 and fields[:1] == ["OBS"] and fields[2] == "ok":
        if not re.fullmatch(r"(?:[0-9a-f]{2})*", fields[3]):
            raise ProbeError("observation contains malformed hexadecimal bytes")
        return {"label": fields[1], "ok": True, "bytes_hex": fields[3]}
    if len(fields) == 6 and fields[:1] == ["OBS"] and fields[2] == "err":
        if not re.fullmatch(r"-1|(?:0|[1-9][0-9]*)", fields[5]):
            raise ProbeError("observation has a malformed raw error code")
        try:
            raw = int(fields[5])
        except ValueError as error:
            raise ProbeError("observation has a malformed raw error code") from error
        if raw < -1:
            raise ProbeError("observation has an invalid raw error code")
        return {"label": fields[1], "ok": False, "stage": fields[3],
                "kind": fields[4], "raw_code": raw}
    raise ProbeError("caller emitted an unknown observation record")


def expected_denial(item: dict[str, Any], operation: str) -> bool:
    stages = {"read": {"open_parent", "open_file", "read_file"},
              "write": {"open_parent", "write_file"}}
    return (not item.get("ok") and item.get("kind") == "PermissionDenied"
            and item.get("raw_code") in DENIAL_CODES and item.get("stage") in stages[operation])


def classify_case(observations: dict[str, Any], mode: str) -> str:
    """Classify records only after validation and successful controller controls."""
    if mode == "control":
        return "control_pass"
    if mode != "activated":
        raise ProbeError("cannot classify an unknown caller mode")
    if observations["after_read"].get("ok"):
        return "falsified"
    if observations["during_read"].get("ok"):
        return "inconclusive_mixed_timing"
    return "not_falsified"


def validate_records(lines: list[str], mode: str) -> tuple[dict[str, Any], list[str]]:
    labels = ("before_read", "before_write", "target_control", "target_secret",
              "during_read", "during_write", "after_read", "after_write")
    events = ["OBS\tbefore_read", "OBS\tbefore_write", "EVENT\tspawn_start",
              "OBS\ttarget_control", "OBS\ttarget_secret", "READY",
              "OBS\tduring_read", "OBS\tduring_write", "EVENT\ttarget_wait\t0",
              "OBS\tafter_read", "OBS\tafter_write", "EVENT\tcomplete"]
    projection = ["OBS\t" + line.split("\t", 2)[1] if line.startswith("OBS\t") else line
                  for line in lines]
    if projection != events:
        raise ProbeError("caller event order or target handshake is incomplete")
    observations = [parse_observation(line) for line in lines if line.startswith("OBS\t")]
    if [item["label"] for item in observations] != list(labels):
        raise ProbeError("caller observation labels are missing, repeated or reordered")
    by_label = {item["label"]: item for item in observations}
    for label, expected in (("before_read", SECRET), ("target_control", CONTROL)):
        if by_label[label].get("bytes_hex") != expected.hex():
            raise ProbeError(f"{label} control did not read the exact expected bytes")
    if (not by_label["before_write"].get("ok")
            or by_label["before_write"].get("bytes_hex") != MARKER.hex()):
        raise ProbeError("caller host-write control failed before activation")
    target_secret = by_label["target_secret"]
    if mode == "control":
        if not target_secret.get("ok") or target_secret.get("bytes_hex") != SECRET.hex():
            raise ProbeError("no-activation target could not read the secret control")
    elif not expected_denial(target_secret, "read"):
        raise ProbeError("activated target did not receive the expected permission denial")
    for label in ("during_read", "during_write", "after_read", "after_write"):
        item = by_label[label]
        if item.get("ok"):
            expected = SECRET if label.endswith("read") else MARKER
            if item.get("bytes_hex") != expected.hex():
                raise ProbeError(f"{label} success returned unexpected bytes")
        elif not expected_denial(item, "read" if label.endswith("read") else "write"):
            raise ProbeError(f"{label} failed for a reason outside the accepted policy")
    if mode == "control" and not all(by_label[name].get("ok") for name in labels[4:]):
        raise ProbeError("ordinary-spawn control was unexpectedly restricted")
    return by_label, [line for line in lines if line.startswith("EVENT\t")]
