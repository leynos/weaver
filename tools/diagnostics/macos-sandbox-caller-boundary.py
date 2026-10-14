#!/usr/bin/env python3
"""Run the bounded macOS caller-boundary observation."""

from __future__ import annotations

import argparse
import json
import os
import selectors
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from contextlib import ExitStack
from pathlib import Path
from typing import Any

from macos_caller_boundary_support import (
    RUNTIME_GRANTS,
    CleanupFailure,
    ProbeError,
    artifact_executable,
    bounded_read,
    clean_environment,
    cleanup_group,
    group_live,
    owned_entries,
    provenance,
    prove_owned_group,
    ps_snapshot,
    repo_root,
    routing_receipt,
    sha256,
    write_json,
)
from macos_caller_boundary_dependencies import dependency_receipt
from macos_caller_boundary_protocol import (
    CONTROL,
    MARKER,
    SECRET,
    classify_case,
    parse_records,
    validate_records,
)

CALLER_SECONDS = 30
READY_SECONDS = 10
TERM_SECONDS = 2
OUTPUT_LIMIT = 32 * 1024
VERDICT_EXIT_CODES = {"not_falsified": 10, "falsified": 0, "inconclusive_mixed_timing": 20}


def path_contains(parent: Path, child: Path) -> bool:
    return parent == child or parent in child.parents


def make_fixture() -> dict[str, Any]:
    root = Path(tempfile.mkdtemp(prefix="weaver-caller-boundary-")).resolve(strict=True)
    try:
        os.chmod(root, 0o700)
        paths = {name: root / name for name in ("allowed", "denied", "host-only")}
        for path in paths.values():
            path.mkdir(mode=0o700)
            os.chmod(path, 0o700)
        control, secret = paths["allowed"] / "control.txt", paths["denied"] / "secret.txt"
        for path, value in ((control, CONTROL), (secret, SECRET)):
            with path.open("xb") as stream:
                stream.write(value)
            os.chmod(path, 0o600)
        return {"root": root, "allowed": paths["allowed"], "denied": paths["denied"],
                "host": paths["host-only"], "control": control.resolve(strict=True),
                "secret": secret.resolve(strict=True)}
    except OSError as error:
        try:
            shutil.rmtree(root)
        except OSError as cleanup_error:
            raise ProbeError(f"partial fixture cleanup failed at {root}: {cleanup_error}") from error
        raise ProbeError(f"partial fixture at {root} was removed after setup failure: {error}") from error


def fixture_metadata(paths: dict[str, Any]) -> dict[str, Any]:
    files = {}
    for label, path in (("control", paths["control"]), ("secret", paths["secret"])):
        data = path.read_bytes()
        mode = stat.S_IMODE(path.stat().st_mode)
        files[label] = {"path": str(path), "mode": oct(mode), "sha256": sha256(data)}
    directories = {key: {"path": str(path), "mode": oct(stat.S_IMODE(path.stat().st_mode))}
                   for key, path in paths.items() if key in {"root", "allowed", "denied", "host"}}
    return {"files": files, "directories": directories}


def validate_topology(paths: dict[str, Any], executable: Path, cwd: Path) -> list[Path]:
    root = paths["root"].resolve(strict=True)
    expected_names = {"allowed": "allowed", "denied": "denied", "host": "host-only"}
    for name, directory in expected_names.items():
        raw_path = paths[name]
        path = raw_path.resolve(strict=True)
        if raw_path.is_symlink() or path != root / directory or path.parent != root:
            raise ProbeError("fixture subtrees are not disjoint canonical siblings")
    control, secret = paths["control"], paths["secret"]
    if (control.is_symlink() or secret.is_symlink() or control != control.resolve(strict=True)
            or secret != secret.resolve(strict=True) or control.parent != paths["allowed"]
            or secret.parent != paths["denied"]):
        raise ProbeError("fixture files are outside their designated subtrees")
    grants = [executable.resolve(strict=True), paths["control"].resolve(strict=True)]
    grants.extend(path.resolve(strict=True) for path in RUNTIME_GRANTS)
    if (len(set(grants)) != len(grants) or any(not path.is_dir() for path in grants[2:])
            or any(path.resolve(strict=True) != path for path in RUNTIME_GRANTS)):
        raise ProbeError("runtime grants are missing, noncanonical or not unique")
    secret, host = paths["secret"], paths["host"]
    for grant in grants:
        if grant == paths["control"]:
            continue
        if path_contains(grant, root) or path_contains(root, grant):
            raise ProbeError("a grant overlaps the fixture root")
        if path_contains(grant, secret) or path_contains(grant, host):
            raise ProbeError("a grant covers denied or host-only data")
    if path_contains(root, cwd) or any(path_contains(grant, cwd) for grant in grants):
        raise ProbeError("caller working directory is granted or inside the fixture")
    for path in (root, paths["allowed"], paths["denied"], paths["host"]):
        if stat.S_IMODE(path.stat().st_mode) != 0o700:
            raise ProbeError("fixture directory mode changed")
    for path in (paths["control"], paths["secret"]):
        if stat.S_IMODE(path.stat().st_mode) != 0o600:
            raise ProbeError("fixture file mode changed")
    return grants


def drain_after_cleanup(selector: selectors.BaseSelector, captured: dict[str, bytearray],
                        total: int) -> bool:
    overflow = False
    for key in list(selector.get_map().values()):
        while True:
            chunk = os.read(key.fileobj.fileno(), 4096)
            if not chunk:
                selector.unregister(key.fileobj)
                break
            remaining = OUTPUT_LIMIT - total
            captured[key.data].extend(chunk[:remaining])
            total += min(len(chunk), remaining)
            if len(chunk) > remaining:
                overflow = True
                break
    return overflow


def supervise(executable: Path, mode: str, paths: dict[str, Any], grants: list[Path],
              cwd: Path, case_dir: Path) -> tuple[int, bytes, bytes, dict[str, Any]]:
    argv = [str(executable), mode, str(paths["secret"]), str(paths["control"]), str(paths["host"]),
            *(str(path) for path in grants[2:])]
    try:
        proc = subprocess.Popen(argv, cwd=cwd, env=clean_environment(), stdin=subprocess.PIPE,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, close_fds=True,
                                start_new_session=True)
    except OSError as error:
        raise ProbeError(f"cannot start {mode} caller: {error}") from error
    owned = False
    reaped = False
    selector = selectors.DefaultSelector()
    captured = {"stdout": bytearray(), "stderr": bytearray()}
    pending = bytearray()
    records: list[str] = []
    total = 0
    spawn_at: float | None = None
    cleanup_confirmed = False
    try:
        if proc.stdin is None or proc.stdout is None or proc.stderr is None:
            raise ProbeError("caller admission or output pipe is unavailable")
        for name, stream in (("stdout", proc.stdout), ("stderr", proc.stderr)):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, name)
        prove_owned_group(proc)
        owned = True
        if proc.stdin.write(b"A") != 1:
            raise ProbeError("caller did not accept the admission byte")
        proc.stdin.close()
        deadline = time.monotonic() + CALLER_SECONDS
        while selector.get_map():
            now = time.monotonic()
            ready_deadline = spawn_at + READY_SECONDS if spawn_at is not None else deadline
            remaining = min(deadline, ready_deadline) - now
            if remaining <= 0:
                raise ProbeError("caller or target-ready deadline expired")
            for key, _ in selector.select(remaining):
                chunk = os.read(key.fileobj.fileno(), 4096)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                remaining_output = OUTPUT_LIMIT - total
                captured[key.data].extend(chunk[:remaining_output])
                total += min(len(chunk), remaining_output)
                if len(chunk) > remaining_output:
                    raise ProbeError("caller stdout and stderr exceeded 32 KiB")
                if key.data == "stdout":
                    pending.extend(chunk)
                    while b"\n" in pending:
                        raw, _, tail = pending.partition(b"\n")
                        pending = bytearray(tail)
                        if len(raw) > 4096:
                            raise ProbeError("caller emitted an oversized protocol line")
                        try:
                            line = raw.decode("ascii")
                        except UnicodeDecodeError as error:
                            raise ProbeError("caller stdout is not ASCII") from error
                        records.append(line)
                        if line == "EVENT\tspawn_start":
                            spawn_at = time.monotonic()
                        elif line == "READY":
                            spawn_at = None
        if pending:
            raise ProbeError("caller stdout ended in a partial protocol record")
        validate_records(records, mode)
        before_live, before_entries = group_live(proc.pid)
        exit_deadline = time.monotonic() + TERM_SECONDS
        while before_live and time.monotonic() < exit_deadline:
            time.sleep(0.05)
            before_live, before_entries = group_live(proc.pid)
        if before_live or any(pid != proc.pid for pid, _, _ in before_entries):
            raise ProbeError("owned descendants remain after the complete receipt")
        returncode = proc.wait()
        reaped = True
        if owned_entries(proc.pid, ps_snapshot()):
            raise CleanupFailure("process group remains after normal caller reap")
        cleanup_confirmed = True
        if returncode != 0:
            raise ProbeError(f"caller returned unexpected status {returncode}")
        receipt = {"before_reap_group": before_entries, "after_reap_group": [],
                   "target_wait": "0", "complete": True, "returncode": returncode}
        return returncode, bytes(captured["stdout"]), bytes(captured["stderr"]), receipt
    except (ProbeError, OSError, subprocess.SubprocessError) as error:
        if not cleanup_confirmed:
            if reaped:
                raise CleanupFailure(f"process cleanup could not be confirmed: {error}") from error
            try:
                cleanup = cleanup_group(proc, owned)
            except (ProbeError, OSError, subprocess.SubprocessError) as cleanup_error:
                raise CleanupFailure(f"process cleanup failed after {error}: {cleanup_error}") from cleanup_error
            cleanup_confirmed = True
        else:
            cleanup = {"terminated": True, "already_reaped": True}
        cleanup["output_overflow"] = drain_after_cleanup(selector, captured, total)
        raise ProbeError(f"{error}; cleanup={cleanup}") from error
    finally:
        try:
            try:
                (case_dir / "stdout.bin").write_bytes(bytes(captured["stdout"]))
                (case_dir / "stderr.bin").write_bytes(bytes(captured["stderr"]))
            finally:
                with ExitStack() as closing:
                    for stream in (proc.stdin, proc.stdout, proc.stderr):
                        if stream is not None and not stream.closed:
                            closing.callback(stream.close)
                    closing.callback(selector.close)
        except OSError as error:
            if not cleanup_confirmed:
                raise CleanupFailure("cannot save evidence with unverified process cleanup") from error
            raise


def run_case(root: Path, executable: Path, mode: str) -> dict[str, Any]:
    case_dir = root / f"case-{mode}"
    case_dir.mkdir()
    paths: dict[str, Any] | None = None
    receipt: dict[str, Any] = {"mode": mode, "profile": "FullEnvironment", "status": "inconclusive"}
    cleanup_ok = False
    cleanup_verified = True
    try:
        paths = make_fixture()
        cwd = Path("/")
        grants = validate_topology(paths, executable, cwd)
        receipt.update({"fixture": fixture_metadata(paths), "working_directory": str(cwd),
                        "grants": [str(path) for path in grants]})
        independent = paths["host"] / "controller-marker.txt"
        with independent.open("xb") as stream:
            stream.write(b"controller-host-control\n")
        os.chmod(independent, 0o600)
        receipt["controller_marker"] = {
            "path": str(independent), "mode": oct(stat.S_IMODE(independent.stat().st_mode)),
            "sha256": sha256(independent.read_bytes()),
        }
        if paths["secret"].read_bytes() != SECRET:
            raise ProbeError("unrestricted controller could not read secret before caller")
        _, stdout, _, process = supervise(executable, mode, paths, grants, cwd, case_dir)
        receipt["process"] = process
        receipt["observations"], receipt["events"] = validate_records(parse_records(stdout), mode)
        after = fixture_metadata(paths)
        if after != receipt["fixture"]:
            raise ProbeError("controller fixture content or modes changed")
        if paths["secret"].read_bytes() != SECRET or independent.read_bytes() != b"controller-host-control\n":
            raise ProbeError("unrestricted controller controls failed after caller")
        if (stat.S_IMODE(independent.stat().st_mode) != 0o600
                or sha256(independent.read_bytes()) != receipt["controller_marker"]["sha256"]):
            raise ProbeError("controller host marker changed during caller execution")
        receipt["controller_controls"] = {"secret_before_after": True, "host_marker": True,
                                          "fixture_unchanged": True}
        receipt["status"] = classify_case(receipt["observations"], mode)
        cleanup_ok = True
    except (ProbeError, OSError, subprocess.SubprocessError) as error:
        receipt["error"] = str(error)
        cleanup_verified = not isinstance(error, CleanupFailure)
    finally:
        if paths is not None:
            receipt["fixture_final"] = (fixture_metadata(paths) if cleanup_verified else
                                         "not captured because process cleanup was unverified")
            if cleanup_ok:
                shutil.rmtree(paths["root"])
                receipt["fixture_removed_after_capture"] = True
            else:
                receipt["fixture_preserved_for_inconclusive_result"] = True
        write_json(case_dir / "case.json", receipt)
    return receipt


def output_dir(path: Path, allow_preflight: bool) -> Path:
    if path.exists() and (path.is_symlink() or not path.is_dir()):
        raise ProbeError("output directory must be a real directory")
    path.mkdir(parents=True, exist_ok=True)
    existing = {item.name for item in path.iterdir()}
    permitted = {"routing-preflight.json"} if allow_preflight else set()
    if existing - permitted:
        raise ProbeError("output directory contains prior or unexpected artefacts")
    return path.resolve(strict=True)


def preflight(args: argparse.Namespace) -> int:
    out = output_dir(args.output_dir, allow_preflight=False)
    evidence: dict[str, Any] = {"runner": {"system": sys.platform}}
    if sys.platform != "darwin":
        evidence.update({"status": "unsupported", "exit_code": 77})
    else:
        try:
            root = repo_root(Path.cwd())
            evidence.update({"provenance": provenance(root),
                             "routing": routing_receipt(root, None),
                             "runner": {"system": sys.platform, "machine": os.uname().machine},
                             "status": "ready", "exit_code": 0})
        except (ProbeError, OSError, KeyError, TypeError, ValueError, AttributeError) as error:
            evidence.update({"status": "inconclusive", "exit_code": 20, "error": str(error)})
    write_json(out / "routing-preflight.json", evidence)
    return int(evidence["exit_code"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cargo-artifact-log", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--check-routing-only", action="store_true")
    args = parser.parse_args()
    if args.check_routing_only:
        return preflight(args)
    if args.cargo_artifact_log is None:
        parser.error("--cargo-artifact-log is required for the observation run")
    out = output_dir(args.output_dir, allow_preflight=True)
    result: dict[str, Any] = {"status": "inconclusive", "exit_code": 20}
    try:
        if sys.platform != "darwin":
            result.update({"status": "unsupported", "exit_code": 77})
        else:
            try:
                preflight_data = json.loads(bounded_read(out / "routing-preflight.json", 1024 * 1024))
            except (OSError, json.JSONDecodeError) as error:
                raise ProbeError("routing preflight receipt is missing or malformed") from error
            if preflight_data.get("status") != "ready":
                raise ProbeError("routing preflight did not authorize the observation build")
            root = repo_root(Path.cwd())
            build_stderr = args.cargo_artifact_log.with_name("build.stderr")
            routing = routing_receipt(root, build_stderr)
            source = provenance(root)
            if source["git_sha"] != preflight_data.get("provenance", {}).get("git_sha"):
                raise ProbeError("candidate changed after routing preflight")
            before = preflight_data["routing"]
            if any(before.get(key) != routing.get(key)
                   for key in ("cargo_config_sha256", "target_dir", "target_dir_selectors",
                               "routing_inputs", "wrappers")):
                raise ProbeError("Cargo routing changed after routing preflight")
            executable = artifact_executable(root, args.cargo_artifact_log)
            source["probe_executable_sha256"] = sha256(bounded_read(executable, 128 * 1024 * 1024))
            dependencies = dependency_receipt(executable)
            source["dependency_inspection"] = dependencies
            result.update({"provenance": source, "routing": routing,
                           "dependency_inspection": dependencies})
            if dependencies["eligible"]:
                controller = run_case(out, executable, "control")
                cases = result["cases"] = {"control": controller}
                if controller["status"] == "control_pass":
                    activated = cases["activated"] = run_case(out, executable, "activated")
                    if activated["status"] in VERDICT_EXIT_CODES:
                        result.update(status=activated["status"],
                                      exit_code=VERDICT_EXIT_CODES[activated["status"]])
    except (ProbeError, OSError, KeyError, ValueError, subprocess.SubprocessError,
            TypeError, AttributeError) as error:
        result["error"] = str(error)
    write_json(out / "verdict.json", result)
    return int(result["exit_code"])


if __name__ == "__main__":
    raise SystemExit(main())
