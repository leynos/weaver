"""Private evidence and process-safety helpers for the caller-boundary probe.

This module is owned by the diagnostic controller and has no other call sites.
It keeps provenance, Cargo routing, and process-group cleanup together so the
executable controller can stay reviewable.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shlex
import signal
import subprocess
import time
import tomllib
from pathlib import Path
from typing import Any

import macos_caller_boundary_cargo as cargo_records

BRANCH_BASE = "aab776ff050f12e1448f404eab01027c500a2d8e"
BIRDCAGE_CHECKSUM = "848df95320021558dd6bb4c26de3fe66724cdcbdbbf3fa720150b52b086ae568"
RUNTIME_GRANTS = (Path("/usr/lib"),)
INPUT_LIMIT = 8 * 1024 * 1024
DEV_FAST_RE = re.compile(r"(?i)(?:tools/)?dev-fast/config\.toml")
SAFE_CODEGEN = {"codegen-backend", "linker", "link-arg", "opt-level", "debuginfo", "codegen-units", "target-cpu"}


class ProbeError(Exception):
    """A bounded measurement or provenance check failed."""


class CleanupFailure(ProbeError):
    """The caller group could not be proven cleaned up safely."""


def sha256(data: bytes) -> str: return hashlib.sha256(data).hexdigest()


def write_json(path: Path, value: Any) -> None: path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def bounded_read(path: Path, limit: int) -> bytes:
    try:
        with path.open("rb") as stream:
            data = stream.read(limit + 1)
    except OSError as error:
        raise ProbeError(f"cannot read {path.name}: {error}") from error
    if len(data) > limit:
        raise ProbeError(f"{path.name} exceeds the {limit}-byte evidence limit")
    return data


def clean_environment() -> dict[str, str]:
    env = os.environ.copy()
    for key in list(env):
        if key.startswith("GIT_CONFIG_") or key in {
            "GH_TOKEN", "GITHUB_TOKEN", "BASH_ENV", "GIT_SSH", "GIT_SSH_COMMAND"
        }:
            env.pop(key, None)
    lody = str(Path.home() / ".lody")
    env["PATH"] = os.pathsep.join(
        part for part in env.get("PATH", "").split(os.pathsep)
        if part != lody and not part.startswith(lody + os.sep)
    )
    return env


def repo_root(start: Path) -> Path:
    for candidate in (start.resolve(), *start.resolve().parents):
        if (candidate / "Cargo.toml").is_file() and (candidate / ".git").exists():
            return candidate
    raise ProbeError("cannot locate the checked-out Cargo repository")


def read_git(root: Path, *args: str, limit: int = 65536) -> bytes:
    env = clean_environment()
    env.update({"GIT_PAGER": "cat", "PAGER": "cat", "LC_ALL": "C"})
    try:
        result = subprocess.run(
            ["/usr/bin/git", *args], cwd=root, env=env, capture_output=True,
            timeout=5, check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProbeError(f"git {args[0]} could not complete") from error
    if result.returncode != 0 or len(result.stdout) > limit or len(result.stderr) > 4096:
        raise ProbeError(f"git {args[0]} failed or exceeded its output bound")
    return result.stdout


def config_files(root: Path, env: dict[str, str]) -> list[tuple[str, Path]]:
    files: list[tuple[str, Path]] = []
    for depth, parent in enumerate((root, *root.parents)):
        for name in ("config.toml", "config"):
            path = parent / ".cargo" / name
            if path.is_file():
                files.append((f"ancestor-{depth}/{name}", path))
    cargo_home = Path(env.get("CARGO_HOME", Path.home() / ".cargo"))
    for name in ("config.toml", "config"):
        path = cargo_home / name
        if path.is_file():
            files.append((f"cargo-home/{name}", path))
    return files


def flag_tokens(value: str) -> list[str]:
    try:
        return shlex.split(value.replace("\x1f", " "))
    except ValueError as error:
        raise ProbeError("Rust flag environment cannot be parsed safely") from error


def inspect_flags(tokens: list[str], source: str, observed: list[str]) -> None:
    if DEV_FAST_RE.search(" ".join(tokens)):
        raise ProbeError(f"development backend selection found in {source}")
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if token in ("-C", "-Z") and index + 1 < len(tokens):
            kind, value = token, tokens[index + 1]
            index += 2
        elif token.startswith(("-C", "-Z")) and "=" in token[2:]:
            kind, value = token[:2], token[2:]
            index += 1
        else:
            index += 1
            continue
        key, _, setting = value.partition("=")
        if key == "codegen-backend" and setting.lower() != "llvm":
            raise ProbeError(f"non-LLVM codegen backend selected in {source}")
        if key in SAFE_CODEGEN:
            if key == "linker":
                setting = Path(setting).name
            elif key == "link-arg" and not setting.startswith("-fuse-ld="):
                continue
            observed.append(f"{kind} {key}={setting}")


def config_flags(value: Any, source: str, observed: list[str], wrappers: dict[str, str]) -> None:
    if isinstance(value, dict):
        for key, nested in value.items():
            if key == "codegen-backend":
                if not isinstance(nested, str) or nested.lower() != "llvm":
                    raise ProbeError(f"non-LLVM codegen backend selected in {source}")
            if key == "linker" and isinstance(nested, str):
                observed.append(f"config linker={Path(nested).name}")
            if key in {"rustc", "rustdoc", "rustc-wrapper", "rustc-workspace-wrapper"}:
                if not isinstance(nested, str):
                    raise ProbeError(f"Cargo {key} in {source} is not a string")
                if key == "rustc-workspace-wrapper" and nested:
                    raise ProbeError(f"unreviewed Cargo {key} configured in {source}")
                wrappers[f"{source}:{key}"] = Path(nested).name
            if key == "rustflags":
                values = flag_tokens(nested) if isinstance(nested, str) else nested
                if isinstance(values, list) and all(isinstance(item, str) for item in values):
                    inspect_flags(values, source, observed)
            config_flags(nested, source, observed, wrappers)
    elif isinstance(value, list):
        for nested in value:
            config_flags(nested, source, observed, wrappers)


def routing_receipt(root: Path, build_log: Path | None) -> dict[str, Any]:
    env = clean_environment()
    observed: list[str] = []
    files: list[dict[str, str]] = []
    target_dirs: list[tuple[str, Path, Path]] = []
    wrappers: dict[str, str] = {}
    if "CARGO_TARGET_DIR" in env:
        target_dirs.append(("environment:CARGO_TARGET_DIR", root, Path(env["CARGO_TARGET_DIR"])))
    for label, path in config_files(root, env):
        content = bounded_read(path, 1024 * 1024)
        files.append({"name": label, "sha256": sha256(content)})
        try:
            parsed = tomllib.loads(content.decode("utf-8"))
        except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
            raise ProbeError(f"active Cargo config {label} is not valid TOML") from error
        config_flags(parsed, label, observed, wrappers)
        build = parsed.get("build", {})
        if isinstance(build, dict) and "target-dir" in build:
            base = path.parent if label.startswith("cargo-home/") else path.parent.parent
            target = build["target-dir"]
            if not isinstance(target, str):
                raise ProbeError(f"Cargo target directory in {label} is not a string")
            target_dirs.append((f"config:{label}", base, Path(target)))
    target_root = (root / "target").resolve(strict=False)
    if root not in target_root.parents:
        raise ProbeError("worktree target directory resolves outside the checkout")
    target_selections: list[str] = []
    for source, base, target_dir in target_dirs:
        resolved = (target_dir if target_dir.is_absolute() else base / target_dir).resolve(strict=False)
        if resolved != target_root and target_root not in resolved.parents:
            raise ProbeError("effective Cargo target directory is outside worktree target")
        target_selections.append(f"{source}={resolved}")
    for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS"):
        if value := env.get(name):
            inspect_flags(flag_tokens(value), name, observed)
    for name, value in env.items():
        if name.startswith("CARGO_TARGET_") and name.endswith("_RUSTFLAGS"):
            inspect_flags(flag_tokens(value), name, observed)
        if name.startswith("CARGO_PROFILE_") and name.endswith("_CODEGEN_BACKEND"):
            if value.lower() != "llvm":
                raise ProbeError(f"non-LLVM codegen backend selected in {name}")
            observed.append("cargo-profile-codegen-backend=llvm")
    if env.get("RUSTC_WORKSPACE_WRAPPER"):
        raise ProbeError("unreviewed RUSTC_WORKSPACE_WRAPPER configured")
    for name in ("RUSTC", "RUSTC_WRAPPER", "RUSTDOC"):
        if value := env.get(name):
            tokens = flag_tokens(value)
            inspect_flags(tokens, name, observed)
            wrappers[name.lower()] = Path(tokens[0]).name if tokens else "configured"
    routing_inputs = sorted(set(observed))
    compiler_records: list[dict[str, Any]] = []
    build_log_hash = None
    if build_log is not None:
        raw_data = bounded_read(build_log, INPUT_LIMIT)
        data = raw_data.decode("utf-8", errors="replace")
        if DEV_FAST_RE.search(data):
            raise ProbeError("build log selects the dev-fast fragment")
        build_log_hash = sha256(raw_data)
        wrapper_names = {value for key, value in wrappers.items()
                         if key == "rustc_wrapper" or key.endswith(":rustc-wrapper")}
        try:
            for record in cargo_records.command_records(data):
                arguments = cargo_records.compiler_arguments(record, wrapper_names)
                if arguments is None:
                    continue
                name = cargo_records.crate_name(arguments)
                inspect_flags(list(arguments), "build.stderr", observed)
                compiler_records.append({"crate_name": name, "start_line": record.start_line,
                                         "end_line": record.end_line})
        except cargo_records.CargoRecordError as error:
            raise ProbeError(f"Cargo build records cannot be parsed safely: {error}") from error
        if sum(record["crate_name"] == "sandbox_macos_caller_probe"
               for record in compiler_records) != 1:
            raise ProbeError("build log does not contain exactly one probe rustc invocation")
    observed = sorted(set(observed))
    linkers = sorted({item.split("=", 1)[1] for item in observed if item.startswith("-C linker=")})
    backend = ("unobserved before verbose rustc invocation" if build_log is None else
               "LLVM explicit; verbose inputs checked" if any(
                   "codegen-backend=llvm" in item for item in observed) else
               "default; verbose inputs contain no non-LLVM override")
    return {
        "cargo_config_sha256": files,
        "build_stderr_sha256": build_log_hash,
        "rustc_records": compiler_records,
        "target_dir": str(target_root),
        "target_dir_selectors": sorted(target_selections),
        "backend": backend,
        "linker": linkers or (["unobserved before verbose rustc invocation"]
                               if build_log is None else ["toolchain default"]),
        "routing_inputs": routing_inputs,
        "effective_flags": observed,
        "wrappers": wrappers,
    }


def provenance(root: Path) -> dict[str, Any]:
    head = read_git(root, "rev-parse", "HEAD").decode().strip()
    base = read_git(root, "rev-parse", "HEAD^").decode().strip()
    if base != BRANCH_BASE:
        raise ProbeError("candidate parent is not the frozen aab776f source")
    if read_git(root, "status", "--porcelain", "--untracked-files=all", limit=65536).strip():
        raise ProbeError("checkout is dirty; immutable candidate cannot be attested")
    runtime: dict[str, dict[str, str]] = {}
    for path in ("crates/weaver-sandbox/src", "crates/weaver-plugins/src"):
        head_tree = read_git(root, "rev-parse", f"HEAD:{path}").decode().strip()
        base_tree = read_git(root, "rev-parse", f"HEAD^:{path}").decode().strip()
        if head_tree != base_tree:
            raise ProbeError(f"production runtime tree changed: {path}")
        runtime[path] = {"candidate": head_tree, "base": base_tree}
    lock = read_git(root, "show", "HEAD:Cargo.lock", limit=2 * 1024 * 1024)
    base_lock = read_git(root, "show", "HEAD^:Cargo.lock", limit=2 * 1024 * 1024)
    if sha256(lock) != sha256(base_lock):
        raise ProbeError("Cargo.lock differs from the frozen runtime base")
    packages = tomllib.loads(lock.decode("utf-8"))["package"]
    birdcage = next((item for item in packages if item["name"] == "birdcage"), None)
    if not birdcage or birdcage.get("version") != "0.8.1" or birdcage.get("checksum") != BIRDCAGE_CHECKSUM:
        raise ProbeError("Cargo.lock does not contain the frozen Birdcage package")
    return {
        "git_sha": head,
        "base_sha": base,
        "runtime_tree_hashes": runtime,
        "cargo_lock_sha256": sha256(lock),
        "birdcage": {"version": birdcage["version"], "checksum": birdcage["checksum"]},
    }


def artifact_executable(root: Path, artifact_log: Path) -> Path:
    artifacts: list[dict[str, Any]] = []
    for line in bounded_read(artifact_log, INPUT_LIMIT).splitlines():
        if not line:
            continue
        try:
            item = json.loads(line)
        except json.JSONDecodeError as error:
            raise ProbeError("Cargo artifact log contains a malformed JSON line") from error
        target = item.get("target", {})
        if (item.get("reason") == "compiler-artifact"
                and target.get("name") == "sandbox_macos_caller_probe"
                and "test" in target.get("kind", []) and item.get("executable")):
            artifacts.append(item)
    if len(artifacts) != 1:
        raise ProbeError(f"expected one probe executable artifact, found {len(artifacts)}")
    executable = Path(artifacts[0]["executable"]).resolve(strict=True)
    if not executable.is_file() or root / "target" not in executable.parents:
        raise ProbeError("probe executable is outside the worktree target directory")
    return executable


def ps_snapshot() -> list[tuple[int, int, str]]:
    try:
        result = subprocess.run(
            ["/bin/ps", "-axo", "pid=,pgid=,stat="], env=clean_environment(),
            capture_output=True, timeout=2, check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProbeError("cannot inspect owned process groups") from error
    if result.returncode or len(result.stdout) > 1024 * 1024:
        raise ProbeError("process table failed or exceeded its output bound")
    entries = []
    for line in result.stdout.decode("ascii", errors="replace").splitlines():
        parts = line.split()
        if len(parts) != 3 or not all(part.isascii() and part.isdecimal() for part in parts[:2]):
            raise ProbeError("process table row has an unexpected shape")
        entries.append((int(parts[0]), int(parts[1]), parts[2]))
    return entries


def owned_entries(pid: int, snapshot: list[tuple[int, int, str]]) -> list[tuple[int, int, str]]: return [entry for entry in snapshot if entry[1] == pid]


def prove_owned_group(proc: subprocess.Popen[bytes]) -> None:
    pid = proc.pid
    if os.getpgid(pid) != pid or os.getsid(pid) != pid:
        raise ProbeError("new caller is not the leader of its owned session and group")
    if not any(p == pid and g == pid for p, g, _ in ps_snapshot()):
        raise ProbeError("process table does not confirm the new caller group")


def group_live(pid: int) -> tuple[bool, list[tuple[int, int, str]]]:
    entries = owned_entries(pid, ps_snapshot())
    return any(not state.startswith(("Z", "X")) for _, _, state in entries), entries


def cleanup_group(proc: subprocess.Popen[bytes], owned: bool) -> dict[str, Any]:
    pid = proc.pid
    if not owned:
        if proc.stdin is not None and not proc.stdin.closed:
            proc.stdin.close()
        try:
            returncode = proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.send_signal(signal.SIGTERM)
            try:
                returncode = proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.send_signal(signal.SIGKILL)
                returncode = proc.wait(timeout=2)
        return {"terminated_direct_child": True, "session_verified": False, "returncode": returncode}
    live, entries = group_live(pid)
    if live:
        if os.getpgid(pid) != pid or os.getsid(pid) != pid:
            raise ProbeError("caller session identity changed before cleanup")
        os.killpg(pid, signal.SIGTERM)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            live, entries = group_live(pid)
            if not live:
                break
            time.sleep(0.05)
        if live:
            if os.getpgid(pid) != pid or os.getsid(pid) != pid:
                raise ProbeError("caller session identity changed before SIGKILL")
            os.killpg(pid, signal.SIGKILL)
    returncode = proc.wait(timeout=2)
    deadline = time.monotonic() + 2
    while time.monotonic() < deadline:
        if not owned_entries(pid, ps_snapshot()):
            return {"terminated": True, "returncode": returncode}
        time.sleep(0.05)
    raise ProbeError("owned process group remains after cleanup and caller reap")
