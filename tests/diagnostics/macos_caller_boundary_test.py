"""Safety contracts for the disposable controller; these never invoke Birdcage.

The standard-library runner keeps diagnostic preparation dependency-free.
Existing pytest tooling can discover the same named unittest cases.
"""

from __future__ import annotations

import argparse
import errno
import importlib.util
import json
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parents[2] / "tools" / "diagnostics"
sys.path.insert(0, str(TOOLS))
try:
    import macos_caller_boundary_dependencies as dependencies
    import macos_caller_boundary_protocol as protocol
    import macos_caller_boundary_support as support
    SPEC = importlib.util.spec_from_file_location(
        "weaver_macos_boundary_controller", TOOLS / "macos-sandbox-caller-boundary.py"
    )
    if SPEC is None or SPEC.loader is None:
        raise RuntimeError("diagnostic controller import has no loader")
    controller = importlib.util.module_from_spec(SPEC)
    SPEC.loader.exec_module(controller)
finally:
    sys.path.pop(0)


def transcript(mode: str, during: bool, after: bool) -> bytes:
    """Render the declared two-case wire contract with selected caller reads."""
    def read(label: str, success: bool, value: bytes) -> str:
        if success:
            return f"OBS\t{label}\tok\t{value.hex()}"
        return f"OBS\t{label}\terr\topen_file\tPermissionDenied\t13"

    def write(label: str) -> str:
        if mode == "control" or label == "before_write":
            return f"OBS\t{label}\tok\t{protocol.MARKER.hex()}"
        return f"OBS\t{label}\terr\topen_parent\tPermissionDenied\t1"

    return ("\n".join([
        read("before_read", True, protocol.SECRET), write("before_write"),
        "EVENT\tspawn_start", read("target_control", True, protocol.CONTROL),
        read("target_secret", mode == "control", protocol.SECRET), "READY",
        read("during_read", during, protocol.SECRET), write("during_write"),
        "EVENT\ttarget_wait\t0", read("after_read", after, protocol.SECRET),
        write("after_write"), "EVENT\tcomplete",
    ]) + "\n").encode("ascii")


class ClassificationContracts(unittest.TestCase):
    """Pin decisions to the hypothesis table, including its asymmetric falsifier."""

    def test_declared_table_and_receipts_are_json_safe(self) -> None:
        cases = [
            ("activated", False, False, "not_falsified"),
            ("activated", True, True, "falsified"),
            ("activated", False, True, "falsified"),
            ("activated", True, False, "inconclusive_mixed_timing"),
            ("control", True, True, "control_pass"),
        ]
        for mode, during, after, expected in cases:
            with self.subTest(mode=mode, during=during, after=after):
                observations, events = protocol.validate_records(
                    protocol.parse_records(transcript(mode, during, after)), mode
                )
                self.assertEqual(protocol.classify_case(observations, mode), expected)
                encoded = json.dumps({"observations": observations, "events": events})
                self.assertEqual(json.loads(encoded)["observations"], observations)

    def test_target_or_no_activation_control_failure_is_not_a_measurement(self) -> None:
        variants = [
            ("activated", transcript("activated", False, False).replace(
                b"open_file\tPermissionDenied\t13", b"open_file\tNotFound\t2", 1)),
            ("activated", transcript("activated", False, False).replace(
                protocol.CONTROL.hex().encode(), b"00", 1)),
            ("control", transcript("control", False, True)),
        ]
        for mode, data in variants:
            with self.subTest(mode=mode, data=data):
                with self.assertRaises(support.ProbeError):
                    protocol.validate_records(protocol.parse_records(data), mode)

    def test_missing_target_wait_receipt_is_not_a_complete_case(self) -> None:
        data = transcript("activated", False, False).replace(b"EVENT\ttarget_wait\t0\n", b"")
        with self.assertRaises(support.ProbeError):
            protocol.validate_records(protocol.parse_records(data), "activated")


class DependencyMetadataContracts(unittest.TestCase):
    """Install-name scope does not imply that a loose dylib was observed."""

    def test_system_library_metadata_separates_files_cache_names_and_rejections(self) -> None:
        cases = [
            {"label": "existing-file", "name": "/usr/lib/libSystem.B.dylib",
             "physical": "/usr/lib/libSystem.B.dylib", "eligible": True,
             "scope": "approved-runtime", "availability": "file-observed"},
            {"label": "cache-only-logical-name", "name": "/usr/lib/libSystem.B.dylib",
             "physical": "/usr/lib/libSystem.B.dylib", "error": FileNotFoundError,
             "eligible": True, "scope": "approved-runtime-install-name",
             "availability": "unverified"},
            {"label": "out-of-root", "name": "/System/Library/libExample.dylib",
             "physical": "/System/Library/libExample.dylib", "eligible": False},
            {"label": "unresolved-rpath", "name": "@rpath/libExample.dylib",
             "physical": "/usr/lib/libExample.dylib", "eligible": False},
            {"label": "traversal", "name": "/usr/lib/../outside.dylib",
             "physical": "/usr/lib/../outside.dylib", "error": FileNotFoundError,
             "eligible": False},
            {"label": "non-normalized-dot", "name": "/usr/lib/./libExample.dylib",
             "physical": "/usr/lib/libExample.dylib", "error": FileNotFoundError,
             "eligible": False},
            {"label": "non-normalized-slash", "name": "/usr//lib/libExample.dylib",
             "physical": "/usr/lib/libExample.dylib", "error": FileNotFoundError,
             "eligible": False},
            {"label": "missing-beyond-escaping-ancestor", "name": "/usr/lib/link/libExample.dylib",
             "physical": "/outside/libExample.dylib", "error": FileNotFoundError,
             "eligible": False},
            {"label": "existing-escaping-symlink", "name": "/usr/lib/link/libExample.dylib",
             "physical": "/outside/libExample.dylib", "eligible": False},
            {"label": "dangling-symlink", "name": "/usr/lib/dangling.dylib",
             "physical": "/usr/lib/dangling.dylib", "error": FileNotFoundError,
             "lstat_exists": True, "eligible": False},
            {"label": "lstat-permission-error", "name": "/usr/lib/denied.dylib",
             "physical": "/usr/lib/denied.dylib", "error": FileNotFoundError,
             "lstat_error": PermissionError, "eligible": False},
            {"label": "other-io-error", "name": "/usr/lib/libExample.dylib",
             "physical": "/usr/lib/libExample.dylib", "error": PermissionError,
             "eligible": False},
        ]
        for case in cases:
            with self.subTest(label=case["label"]):
                output = f"/controlled/probe:\n\t{case['name']} (compatibility version 1.0.0)\n".encode()
                completed = subprocess.CompletedProcess([], 0, stdout=output, stderr=b"")

                def resolve(path, strict=False):
                    if str(path) == "/usr/lib":
                        return Path("/usr/lib")
                    if strict and str(path) == case["name"] and "error" in case:
                        raise case["error"]("declared metadata condition")
                    return Path(case["physical"])

                with mock.patch.object(dependencies.subprocess, "run", return_value=completed), \
                        mock.patch.object(dependencies.Path, "resolve", new=resolve), \
                        mock.patch.object(dependencies.Path, "is_file", return_value=True), \
                        mock.patch.object(
                            dependencies.Path, "lstat",
                            side_effect=None if case.get("lstat_exists") else case.get("lstat_error", FileNotFoundError),
                            return_value=mock.Mock(st_mode=stat.S_IFLNK),
                        ):
                    result = dependencies.dependency_receipt(Path("/controlled/probe"))
                self.assertEqual(result["eligible"], case["eligible"])
                if case["eligible"]:
                    self.assertEqual(result["dependencies"][0]["scope"], case["scope"])
                    self.assertEqual(result["dependencies"][0]["availability"], case["availability"])
                json.dumps(result)

    def test_real_resolver_does_not_hide_ancestor_permission_errors(self) -> None:
        """A second-pass EACCES must fail even when the final file is absent."""
        name = "/usr/lib/probe-private/libCache.dylib"
        completed = subprocess.CompletedProcess(
            [], 0, stdout=f"/controlled/probe:\n\t{name} (compatibility version 1.0.0)\n".encode(),
            stderr=b"",
        )
        ancestor_reads = 0

        def lstat(path, *args, **kwargs):
            nonlocal ancestor_reads
            text = str(path)
            if text == "/usr/lib/probe-private":
                ancestor_reads += 1
                if ancestor_reads > 1:
                    raise PermissionError(errno.EACCES, "declared denied ancestor", text)
            if text == name:
                raise FileNotFoundError(errno.ENOENT, "declared absent loose file", text)
            return mock.Mock(st_mode=stat.S_IFDIR)

        # Keep Path.resolve real: mocking it would miss non-strict realpath's
        # suppression of OSError at the underlying lstat boundary.
        with mock.patch.object(dependencies.subprocess, "run", return_value=completed), \
                mock.patch("os.lstat", side_effect=lstat):
            result = dependencies.dependency_receipt(Path("/controlled/probe"))
        self.assertGreater(ancestor_reads, 1)
        self.assertFalse(result["eligible"])
        self.assertEqual(result["dependencies"][0]["scope"], "inspection-error")
        self.assertEqual(result["dependencies"][0]["error_kind"], "PermissionError")


class LifecycleContracts(unittest.TestCase):
    """Check controller ownership boundaries without activating a sandbox."""

    def preserve_cleanup(self, receipt: dict) -> None:
        """Remove only the fixture which this test created without a real child."""
        fixture = Path(receipt["fixture"]["directories"]["root"]["path"])
        self.addCleanup(shutil.rmtree, fixture)

    def test_unverified_cleanup_never_reads_fixture_or_reports_measurement(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            with mock.patch.object(controller, "supervise", side_effect=support.CleanupFailure("refused")), \
                    mock.patch.object(controller, "fixture_metadata", wraps=controller.fixture_metadata) as metadata:
                receipt = controller.run_case(Path(raw), Path(sys.executable).resolve(), "activated")
            self.preserve_cleanup(receipt)
            self.assertEqual(metadata.call_count, 1)
            self.assertEqual(receipt["status"], "inconclusive")
            self.assertIn("cleanup was unverified", receipt["fixture_final"])
            self.assertTrue(receipt["fixture_preserved_for_inconclusive_result"])
            self.assertEqual(json.loads((Path(raw) / "case-activated/case.json").read_text())["status"], "inconclusive")

    def test_changed_controller_control_never_reports_measurement(self) -> None:
        def changed_control(executable, mode, paths, grants, cwd, case_dir):
            paths["secret"].write_bytes(b"changed-controller-control")
            return 0, transcript(mode, False, False), b"", {"target_wait": "0", "complete": True}

        with tempfile.TemporaryDirectory() as raw:
            with mock.patch.object(controller, "supervise", side_effect=changed_control):
                receipt = controller.run_case(Path(raw), Path(sys.executable).resolve(), "activated")
            self.preserve_cleanup(receipt)
            self.assertEqual(receipt["status"], "inconclusive")
            self.assertIn("changed", receipt["error"])
            self.assertTrue(receipt["fixture_preserved_for_inconclusive_result"])

    def invoke_main(self, root: Path, outcomes: list[dict]):
        """Supply only these contracts' known metadata and case outcomes to main."""
        (root / "routing-preflight.json").write_text(json.dumps({
            "status": "ready", "routing": {}, "provenance": {"git_sha": "candidate"},
        }))
        args = argparse.Namespace(output_dir=root, cargo_artifact_log=root / "build.jsonl",
                                  check_routing_only=False)
        with mock.patch.object(controller.argparse.ArgumentParser, "parse_args", return_value=args), \
                mock.patch.object(controller.sys, "platform", "darwin"), \
                mock.patch.object(controller, "repo_root", return_value=root), \
                mock.patch.object(controller, "routing_receipt", return_value={}), \
                mock.patch.object(controller, "provenance", return_value={"git_sha": "candidate"}), \
                mock.patch.object(controller, "artifact_executable", return_value=Path(sys.executable)), \
                mock.patch.object(controller, "dependency_receipt", return_value={"eligible": True}), \
                mock.patch.object(controller, "run_case", side_effect=outcomes) as cases:
            status = controller.main()
        return status, cases, json.loads((root / "verdict.json").read_text())

    def test_failed_control_stops_before_activated_case(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            status, cases, verdict = self.invoke_main(Path(raw), [
                {"status": "inconclusive", "error": "cleanup unverified"},
            ])
        self.assertEqual(status, 20)
        self.assertEqual(cases.call_count, 1)
        self.assertEqual(cases.call_args.args[-1], "control")
        self.assertEqual(verdict["status"], "inconclusive")

    def test_top_level_verdict_preserves_finding_falsifier_and_mixed_timing(self) -> None:
        for name, expected_code in [("not_falsified", 10), ("falsified", 0),
                                    ("inconclusive_mixed_timing", 20)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as raw:
                status, cases, verdict = self.invoke_main(Path(raw), [
                    {"status": "control_pass"}, {"status": name},
                ])
                self.assertEqual(status, expected_code)
                self.assertEqual(verdict["exit_code"], expected_code)
                self.assertEqual(verdict["status"], name)
                self.assertEqual([call.args[-1] for call in cases.call_args_list],
                                 ["control", "activated"])

    def test_ownership_refusal_never_admits_and_reaps_preactivation_child(self) -> None:
        created = []
        real_popen = subprocess.Popen
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            marker = root / "activation-must-not-happen"
            script = (
                "import pathlib,sys; admitted=sys.stdin.buffer.read(1); "
                "pathlib.Path(sys.argv[1]).write_text('activated') if admitted == b'A' else None"
            )

            def child_factory(*args, **kwargs):
                child = real_popen([sys.executable, "-c", script, str(marker)], **kwargs)
                created.append(child)
                return child

            try:
                with mock.patch.object(controller.subprocess, "Popen", side_effect=child_factory), \
                        mock.patch.object(controller, "prove_owned_group", side_effect=support.ProbeError("ownership refused")), \
                        mock.patch.object(support.os, "killpg", side_effect=AssertionError("unverified group signal")):
                    with self.assertRaisesRegex(support.ProbeError, "ownership refused"):
                        controller.supervise(
                            Path(sys.executable), "activated",
                            {"secret": root / "secret", "control": root / "control", "host": root / "host"},
                            [], Path("/"), root,
                        )
                self.assertEqual(len(created), 1)
                self.assertEqual(created[0].returncode, 0)
                self.assertTrue(created[0].stdin.closed)
                self.assertFalse(marker.exists())
            finally:
                for child in created:
                    if child.returncode is None:
                        child.kill()
                        child.wait(timeout=2)


if __name__ == "__main__":
    unittest.main()
