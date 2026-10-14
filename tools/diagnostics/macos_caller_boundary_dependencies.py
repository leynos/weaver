"""Private bounded Mach-O dependency evidence for the caller-boundary controller.

Only the controller and its named private contract test import this module. It
records dependency names and file availability without changing runtime grants.
"""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

from macos_caller_boundary_support import ProbeError, RUNTIME_GRANTS, clean_environment, sha256


def _inside(path: Path, root: Path) -> bool:
    return path == root or root in path.parents


def _normalized_absolute(name: str) -> Path | None:
    candidate = Path(name)
    if (not candidate.is_absolute() or str(candidate) != name
            or ".." in candidate.parts or "\x00" in name):
        return None
    return candidate


def _missing_name_stays_in_runtime(
    candidate: Path, logical_root: Path, canonical_root: Path
) -> str:
    """Resolve existing prefixes and reject dangling links or escaping ancestors."""
    for depth in range(len(logical_root.parts) + 1, len(candidate.parts) + 1):
        prefix = Path(*candidate.parts[:depth])
        try:
            canonical = prefix.resolve(strict=True)
        except FileNotFoundError:
            try:
                prefix.lstat()
            except FileNotFoundError:
                return "approved-runtime-install-name"
            return "unresolved"
        if not _inside(canonical, canonical_root):
            return "outside-reviewed-roots"
    return "approved-runtime-install-name"


def _entry(name: str, scope: str, availability: str, error: BaseException | None = None) -> dict[str, str]:
    entry = {"name": name, "scope": scope, "availability": availability}
    if error is not None:
        entry["error_kind"] = type(error).__name__
    return entry


def _dependency_entry(name: str, logical_root: Path, canonical_root: Path) -> dict[str, str]:
    candidate = _normalized_absolute(name)
    if candidate is None:
        return _entry(name, "unresolved", "unverified")

    try:
        canonical = candidate.resolve(strict=True)
    except FileNotFoundError:
        if candidate == logical_root or logical_root not in candidate.parents:
            return _entry(name, "outside-reviewed-roots", "unverified")
        try:
            scope = _missing_name_stays_in_runtime(candidate, logical_root, canonical_root)
            return _entry(name, scope, "unverified")
        except (OSError, RuntimeError, ValueError) as error:
            return _entry(name, "inspection-error", "unverified", error)
    except (OSError, RuntimeError, ValueError) as error:
        return _entry(name, "inspection-error", "unverified", error)

    if not _inside(canonical, canonical_root):
        return _entry(name, "outside-reviewed-roots", "unverified")
    try:
        if canonical.is_file():
            return _entry(name, "approved-runtime", "file-observed")
    except (OSError, RuntimeError, ValueError) as error:
        return _entry(name, "inspection-error", "unverified", error)
    return _entry(name, "not-a-file", "unverified")


def dependency_receipt(executable: Path) -> dict[str, Any]:
    """Return bounded `otool` evidence without inferring dyld-cache availability."""
    try:
        result = subprocess.run(
            ["/usr/bin/otool", "-L", str(executable)], env=clean_environment(),
            capture_output=True, timeout=5, check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProbeError("bounded Mach-O dependency inspection could not complete") from error
    if result.returncode or len(result.stdout) > 32 * 1024 or len(result.stderr) > 4096:
        raise ProbeError("otool -L failed or exceeded its evidence bound")
    lines = result.stdout.decode("utf-8", errors="replace").splitlines()
    if not lines or not lines[0].endswith(":"):
        raise ProbeError("otool -L returned an unexpected output shape")

    logical_root = RUNTIME_GRANTS[0]
    try:
        canonical_root = logical_root.resolve(strict=True)
    except (OSError, RuntimeError, ValueError) as error:
        raise ProbeError("approved runtime grant could not be canonicalized") from error
    names = [line.strip().split(" (", 1)[0] for line in lines[1:] if line.strip()]
    entries = [_dependency_entry(name, logical_root, canonical_root) for name in names]
    eligible_scopes = {"approved-runtime", "approved-runtime-install-name"}
    return {"command": "otool -L", "output_sha256": sha256(result.stdout),
            "runtime_grants": [str(path) for path in RUNTIME_GRANTS],
            "dependencies": entries,
            "eligible": all(item["scope"] in eligible_scopes for item in entries)}
