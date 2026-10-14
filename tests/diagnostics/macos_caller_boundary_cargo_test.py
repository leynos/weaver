"""Six preparation contracts for Cargo evidence; no compiler or probe executes.

The byte-exact fixture is saved build.stderr lines 258-259 from GitHub run
38495275672 attempt 1. Source SHA256:
3bbcaedef983bf878d94636f38f67e0a9e0e938788646911de4db9931ca6d4dd.
Fixture SHA256: c0b3fba5d941e454f5757ae94c15d1fcddbb552a3e06f163b5e775dde3b02919.
"""

from __future__ import annotations

import hashlib
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

TOOLS = Path(__file__).resolve().parents[2] / "tools" / "diagnostics"
sys.path.insert(0, str(TOOLS))
try:
    import macos_caller_boundary_cargo as cargo
    import macos_caller_boundary_support as support
finally:
    sys.path.pop(0)

FIXTURE = Path(__file__).with_name("fixtures") / "macos_cargo_multiline_running.txt"
PROBE = "sandbox_macos_caller_probe"


def command(name: str = PROBE, flags: str = "", metadata: str = "") -> str:
    """Render evidence, for example command('dep') wraps an ordinary rustc record."""
    return (f"     Running `CARGO_PKG_DESCRIPTION='{metadata}' /observed/sccache "
            f"/toolchain/bin/rustc --crate-name {name} {flags}`\n")


class CargoRecordContracts(unittest.TestCase):
    """Keep framing failures separate from backend-policy and selection failures."""

    def routing(self, text: str) -> dict:
        """Return controlled routing evidence; routing(command()) inspects one probe."""
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw).resolve()
            log = root / "build.stderr"
            log.write_text(text, encoding="utf-8")
            with mock.patch.object(support, "clean_environment", return_value={
                    "RUSTC_WRAPPER": "/observed/sccache"}), \
                    mock.patch.object(support, "config_files", return_value=[]):
                return support.routing_receipt(root, log)

    def test_saved_multiline_record_retains_compiler_arguments(self) -> None:
        fixture = FIXTURE.read_bytes()
        self.assertEqual(hashlib.sha256(fixture).hexdigest(),
                         "c0b3fba5d941e454f5757ae94c15d1fcddbb552a3e06f163b5e775dde3b02919")
        text = fixture.decode("utf-8") + command()
        records = cargo.command_records(text)
        self.assertEqual((records[0].start_line, records[0].end_line), (1, 2))
        self.assertIn("CARGO_PKG_DESCRIPTION=Write your own test harness that looks "
                      "and behaves like the built-in test harness used by `rustc --test`\n",
                      records[0].tokens)
        self.assertEqual(cargo.crate_name(cargo.compiler_arguments(records[0], {"sccache"})),
                         "libtest_mimic")
        self.assertIn("--cap-lints", records[0].tokens)
        receipt = self.routing(text)
        self.assertEqual([item["crate_name"] for item in receipt["rustc_records"]],
                         ["libtest_mimic", PROBE])
        self.assertEqual(receipt["build_stderr_sha256"], hashlib.sha256(text.encode()).hexdigest())

    def test_exact_probe_selection_requires_one_actual_invocation(self) -> None:
        cases = [
            ("multiline", command(metadata="first\nsecond"), True),
            ("zero", command("dependency"), False),
            ("duplicate", command() + command(metadata="first\nsecond"), False),
        ]
        for label, text, valid in cases:
            with self.subTest(label=label):
                if valid:
                    receipt = self.routing(text)
                    self.assertEqual(receipt["rustc_records"], [
                        {"crate_name": PROBE, "start_line": 1, "end_line": 2},
                    ])
                else:
                    with self.assertRaisesRegex(support.ProbeError, "exactly one probe"):
                        self.routing(text)

    def test_forbidden_flags_on_continuations_are_rejected_for_every_compiler(self) -> None:
        cases = [
            ("dependency-attached", command("dependency", "\n-Zcodegen-backend=cranelift") + command(), "non-LLVM"),
            ("dependency-separated", command("dependency", "\n-Z codegen-backend=cranelift") + command(), "non-LLVM"),
            ("probe-attached", command(flags="\n-Zcodegen-backend=cranelift"), "non-LLVM"),
            ("probe-separated", command(flags="\n-Z codegen-backend=cranelift"), "non-LLVM"),
            ("metadata-dev-fast", command("dependency", metadata="first\ntools/dev-fast/config.toml") + command(), "dev-fast"),
        ]
        for label, text, reason in cases:
            with self.subTest(label=label):
                with self.assertRaisesRegex(support.ProbeError, reason):
                    self.routing(text)

    def test_malformed_or_ambiguous_records_fail_closed(self) -> None:
        cases = [
            ("single-quote-eof", "Running `DESCRIPTION='unterminated\n", "incomplete.*EOF"),
            ("double-quote-eof", 'Running `DESCRIPTION="unterminated\n', "incomplete.*EOF"),
            ("escape-eof", "Running `rustc \\", "incomplete.*EOF"),
            ("absent-display-open", "Running rustc --crate-name dep\n", "outer display delimiter"),
            ("absent-display-close", "Running `rustc --crate-name dep\n", "incomplete.*EOF"),
            ("next-running", "Running `DESCRIPTION='unterminated\n" + command(), "before next Running"),
            ("ambiguous-delimiter", "Running `rustc` trailing\n", "ambiguous.*delimiter"),
            ("unrecognized-presentation", "\x1b[2K" + command(), "unrecognized ANSI"),
        ]
        for label, text, reason in cases:
            with self.subTest(label=label):
                with self.assertRaisesRegex(support.ProbeError, reason):
                    self.routing(text)

    def test_declared_record_and_input_bounds_are_inclusive(self) -> None:
        base = command()
        exact_bytes = command(metadata="x" * (cargo.RECORD_BYTE_LIMIT - len(base.encode())))
        exact_lines = command(metadata="\n" * (cargo.RECORD_LINE_LIMIT - 1))
        exact_input = "x" * (support.INPUT_LIMIT - len(base.encode()) - 1) + "\n" + base
        cases = [
            ("record-bytes-boundary", exact_bytes, True, ""),
            ("record-bytes-over", exact_bytes.replace("DESCRIPTION='", "DESCRIPTION='x", 1), False, "framing limits"),
            ("record-lines-boundary", exact_lines, True, ""),
            ("record-lines-over", exact_lines.replace("DESCRIPTION='", "DESCRIPTION='\n", 1), False, "framing limits"),
            ("input-boundary", exact_input, True, ""),
            ("input-over", "x" + exact_input, False, "evidence limit"),
        ]
        for label, text, valid, reason in cases:
            with self.subTest(label=label):
                if valid:
                    self.assertEqual(len(self.routing(text)["rustc_records"]), 1)
                else:
                    with self.assertRaisesRegex(support.ProbeError, reason):
                        self.routing(text)

    def test_non_commands_cannot_supply_probe_and_existing_routing_is_preserved(self) -> None:
        metadata = ("Running `CARGO_PKG_DESCRIPTION='rustc --crate-name "
                    f"{PROBE}' /observed/build-script`\n")
        cases = [
            ("progress", f"Compiling rustc --crate-name {PROBE}\n", "exactly one probe"),
            ("metadata", metadata, "exactly one probe"),
            ("crate-name-substring", command(PROBE + "_other"), "exactly one probe"),
            ("unobserved-wrapper", command().replace("/observed/sccache", "/unobserved/wrapper"), "unobserved executable"),
        ]
        for label, text, reason in cases:
            with self.subTest(label=label):
                with self.assertRaisesRegex(support.ProbeError, reason):
                    self.routing(text)
        explicit = command(flags="-Z codegen-backend=llvm -C linker=/usr/bin/clang")
        explicit = explicit.replace("DESCRIPTION=''", 'DESCRIPTION="a \\"quoted\\" `rustc --test`"')
        receipt = self.routing(metadata + explicit)
        self.assertEqual(len(receipt["rustc_records"]), 1)
        self.assertEqual(receipt["backend"], "LLVM explicit; verbose inputs checked")
        self.assertEqual(receipt["linker"], ["clang"])
        self.assertEqual(receipt["wrappers"], {"rustc_wrapper": "sccache"})
        self.assertIn("-Z codegen-backend=llvm", receipt["effective_flags"])
        self.assertIn("-C linker=clang", receipt["effective_flags"])
