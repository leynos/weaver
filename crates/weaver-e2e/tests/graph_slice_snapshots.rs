//! End-to-end snapshots for `observe graph-slice`.

#[path = "support/fixture_io.rs"]
mod fixture_io;
#[path = "test_support/graph_slice.rs"]
mod graph_slice_support;
mod test_support;
#[path = "support/weaver_binary.rs"]
mod weaver_binary;

use graph_slice_support::{GraphSliceRequest, run_graph_slice};
use rstest::{fixture, rstest};
use tempfile::TempDir;
use test_support::{TestDaemon, Transcript, assert_named_snapshot, fixture_uri};
use url::Url;
use weaver_e2e::graph_slice_fixtures::{GraphSliceFixtureCase, PYTHON_CASES, RUST_CASES};

use crate::fixture_io::write_fixture_path;

/// Owns the temporary directory and its corresponding `file://` URI for one snapshot test run.
struct WorkspaceUri {
    _temp_dir: TempDir,
    uri: String,
}

/// Shared configuration for graph-slice snapshot tests.
#[derive(Clone, Copy)]
struct SnapshotHarness {
    default_expected_requests: usize,
}

impl SnapshotHarness {
    #[expect(
        clippy::expect_used,
        reason = "test helper failures should panic with explicit setup messages"
    )]
    fn workspace_for_case(case: GraphSliceFixtureCase) -> WorkspaceUri {
        let temp_dir = TempDir::new().expect("creating temp dir");
        let uri = fixture_uri(&temp_dir, case);
        WorkspaceUri {
            _temp_dir: temp_dir,
            uri,
        }
    }

    #[expect(
        clippy::expect_used,
        reason = "test helper failures should panic with explicit setup messages"
    )]
    fn unsupported_workspace() -> WorkspaceUri {
        let temp_dir = TempDir::new().expect("creating temp dir");
        let path = write_fixture_path(&temp_dir, "notes.txt", "plain text\n")
            .expect("write unsupported fixture");
        let uri = Url::from_file_path(&path).expect("unsupported path to URI");
        WorkspaceUri {
            _temp_dir: temp_dir,
            uri: uri.to_string(),
        }
    }

    fn daemon(self, expected_requests: Option<usize>) -> TestDaemon {
        TestDaemon::start(expected_requests.unwrap_or(self.default_expected_requests))
    }

    const fn request(
        uri: &str,
        line: u32,
        column: u32,
        max_cards: Option<u32>,
    ) -> GraphSliceRequest<'_> {
        GraphSliceRequest {
            uri,
            line,
            column,
            entry_detail: "semantic",
            node_detail: "semantic",
            max_cards,
        }
    }
}

#[fixture]
const fn snapshot_harness() -> SnapshotHarness {
    SnapshotHarness {
        default_expected_requests: 1,
    }
}

/// Serialises a transcript to pretty-printed JSON for snapshot comparison.
#[expect(
    clippy::expect_used,
    reason = "snapshot helper failures should panic with explicit context"
)]
fn render_snapshot<T: serde::Serialize>(transcript: &T) -> String {
    serde_json::to_string_pretty(transcript).expect("serialize transcript")
}

#[expect(
    clippy::expect_used,
    reason = "snapshot assertions should panic with explicit context"
)]
fn assert_and_parse_stdout(stdout: &str) -> serde_json::Value {
    assert!(!stdout.is_empty(), "transcript stdout should not be empty");
    serde_json::from_str(stdout).expect("transcript stdout should be valid JSON")
}

fn assert_schema_version(value: &serde_json::Value, context: &str) {
    assert_eq!(
        value.pointer("/schema_version"),
        Some(&serde_json::json!("graph_slice.v1")),
        "{context} schema_version should be graph_slice.v1"
    );
}

fn expected_exit_status(value: &serde_json::Value) -> i32 {
    match value.get("status").and_then(serde_json::Value::as_str) {
        Some("success") => 0,
        Some("refusal") => match value
            .pointer("/refusal/reason")
            .and_then(serde_json::Value::as_str)
        {
            Some("unsupported_language") => 10,
            Some("no_symbol_at_position") => 11,
            Some("position_out_of_range") => 12,
            Some("not_yet_implemented") => 13,
            Some("backend_unavailable") => 14,
            Some(_) | None => 15,
        },
        Some(_) | None => 15,
    }
}

fn assert_exit_status(actual: i32, value: &serde_json::Value, context: &str) {
    assert_eq!(
        actual,
        expected_exit_status(value),
        "{context} exit status should match payload"
    );
}

/// Parses the CLI payload and asserts the envelope invariants every
/// graph-slice response shares, returning the payload for further checks.
fn assert_graph_slice_envelope(transcript: &Transcript, context: &str) -> serde_json::Value {
    let value = assert_and_parse_stdout(&transcript.stdout);
    assert_schema_version(&value, context);
    assert_exit_status(transcript.status, &value, context);
    value
}

/// Reports whether a graph-slice payload describes a successful slice.
fn is_success(value: &serde_json::Value) -> bool {
    value.get("status") == Some(&serde_json::json!("success"))
}

/// Asserts a payload is a refusal carrying `expected_reason`.
fn assert_refusal(value: &serde_json::Value, expected_reason: &str, context: &str) {
    assert_eq!(
        value.get("status"),
        Some(&serde_json::json!("refusal")),
        "{context} should return success or refusal"
    );
    assert_eq!(
        value.pointer("/refusal/reason"),
        Some(&serde_json::json!(expected_reason)),
        "{context} refusal reason should match the snapshot contract"
    );
}

/// Asserts an untruncated successful slice carries a budget, cards, and an
/// empty edge set.
fn assert_populated_slice(value: &serde_json::Value, context: &str) {
    assert!(
        value
            .pointer("/constraints/budget/max_cards")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            > 0,
        "{context} budget.max_cards should be positive"
    );
    assert_eq!(
        value.get("edges"),
        Some(&serde_json::json!([])),
        "{context} edges should be empty"
    );
    assert!(
        value
            .get("spillover")
            .is_some_and(serde_json::Value::is_object),
        "{context} spillover should be present"
    );
    assert!(
        value
            .get("cards")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|cards| !cards.is_empty()),
        "{context} cards should be non-empty"
    );
}

/// Asserts a successful slice honoured `--max-cards 1` and flagged spillover.
fn assert_truncated_to_single_card(value: &serde_json::Value, context: &str) {
    assert_eq!(
        value
            .get("cards")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len),
        1,
        "{context} should contain exactly 1 card (max_cards=1)"
    );
    assert_eq!(
        value
            .pointer("/spillover/truncated")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "{context} spillover should be truncated"
    );
}

#[rstest]
#[case::python_01(PYTHON_CASES[0])]
#[case::python_02(PYTHON_CASES[1])]
#[case::python_03(PYTHON_CASES[2])]
#[case::python_04(PYTHON_CASES[3])]
#[case::python_05(PYTHON_CASES[4])]
#[case::python_06(PYTHON_CASES[5])]
#[case::python_07(PYTHON_CASES[6])]
#[case::python_08(PYTHON_CASES[7])]
#[case::python_09(PYTHON_CASES[8])]
#[case::python_10(PYTHON_CASES[9])]
#[case::python_11(PYTHON_CASES[10])]
#[case::python_12(PYTHON_CASES[11])]
#[case::python_13(PYTHON_CASES[12])]
#[case::python_14(PYTHON_CASES[13])]
#[case::python_15(PYTHON_CASES[14])]
#[case::python_16(PYTHON_CASES[15])]
#[case::python_17(PYTHON_CASES[16])]
#[case::python_18(PYTHON_CASES[17])]
#[case::python_19(PYTHON_CASES[18])]
#[case::python_20(PYTHON_CASES[19])]
#[case::rust_01(RUST_CASES[0])]
#[case::rust_02(RUST_CASES[1])]
#[case::rust_03(RUST_CASES[2])]
#[case::rust_04(RUST_CASES[3])]
#[case::rust_05(RUST_CASES[4])]
#[case::rust_06(RUST_CASES[5])]
#[case::rust_07(RUST_CASES[6])]
#[case::rust_08(RUST_CASES[7])]
#[case::rust_09(RUST_CASES[8])]
#[case::rust_10(RUST_CASES[9])]
#[case::rust_11(RUST_CASES[10])]
#[case::rust_12(RUST_CASES[11])]
#[case::rust_13(RUST_CASES[12])]
#[case::rust_14(RUST_CASES[13])]
#[case::rust_15(RUST_CASES[14])]
#[case::rust_16(RUST_CASES[15])]
#[case::rust_17(RUST_CASES[16])]
#[case::rust_18(RUST_CASES[17])]
#[case::rust_19(RUST_CASES[18])]
#[case::rust_20(RUST_CASES[19])]
fn graph_slice_semantic_snapshots_cover_python_and_rust_fixture_battery(
    #[case] case: GraphSliceFixtureCase,
    snapshot_harness: SnapshotHarness,
) {
    let workspace = SnapshotHarness::workspace_for_case(case);
    let daemon = snapshot_harness.daemon(None);
    let transcript = run_graph_slice(
        &daemon,
        SnapshotHarness::request(&workspace.uri, case.line, case.column, None),
    );
    // Parse and assert structural shape so regressions surface even if snapshots are not reviewed.
    let snapshot_name = format!("graph_slice_{}", case.name);
    let value = assert_graph_slice_envelope(&transcript, &snapshot_name);
    if is_success(&value) {
        assert_populated_slice(&value, &snapshot_name);
    } else {
        assert_refusal(&value, "no_symbol_at_position", &snapshot_name);
    }

    daemon.join();
    assert_named_snapshot(&snapshot_name, &render_snapshot(&transcript));
}

#[rstest]
#[case::rust_multi_symbol(RUST_CASES[7])]
#[case::python_multi_symbol(PYTHON_CASES[5])]
fn graph_slice_truncation_snapshots(
    #[case] case: GraphSliceFixtureCase,
    snapshot_harness: SnapshotHarness,
) {
    let workspace = SnapshotHarness::workspace_for_case(case);
    let daemon = snapshot_harness.daemon(None);
    let transcript = run_graph_slice(
        &daemon,
        SnapshotHarness::request(&workspace.uri, case.line, case.column, Some(1)),
    );
    let snapshot_name = format!("graph_slice_truncated_{}", case.name);
    let value = assert_graph_slice_envelope(&transcript, &snapshot_name);
    if is_success(&value) {
        assert_truncated_to_single_card(&value, &snapshot_name);
    } else {
        assert_refusal(&value, "no_symbol_at_position", &snapshot_name);
    }

    daemon.join();
    assert_named_snapshot(&snapshot_name, &render_snapshot(&transcript));
}

/// Selects the fixture workspace a refusal case runs against.
///
/// The workspace cannot be built inside a `#[case]` attribute because it owns a
/// `TempDir` that must live only for the duration of the test.
#[derive(Clone, Copy)]
enum RefusalWorkspace {
    /// A plain-text file, which the language router cannot classify.
    UnsupportedLanguage,
    /// A Rust fixture drawn from the shared case battery.
    Fixture(GraphSliceFixtureCase),
}

impl RefusalWorkspace {
    fn build(self) -> WorkspaceUri {
        match self {
            Self::UnsupportedLanguage => SnapshotHarness::unsupported_workspace(),
            Self::Fixture(case) => SnapshotHarness::workspace_for_case(case),
        }
    }
}

#[derive(Clone, Copy)]
struct RefusalSnapshotCase {
    workspace: RefusalWorkspace,
    line: u32,
    column: u32,
    expected_reason: &'static str,
    snapshot_name: &'static str,
}

fn run_refusal_snapshot(harness: SnapshotHarness, case: RefusalSnapshotCase) {
    let workspace = case.workspace.build();
    let daemon = harness.daemon(None);
    let transcript = run_graph_slice(
        &daemon,
        SnapshotHarness::request(&workspace.uri, case.line, case.column, None),
    );

    let value = assert_graph_slice_envelope(&transcript, case.snapshot_name);
    assert_refusal(&value, case.expected_reason, case.snapshot_name);

    daemon.join();
    assert_named_snapshot(case.snapshot_name, &render_snapshot(&transcript));
}

#[rstest]
#[case::unsupported_language(RefusalSnapshotCase {
    workspace: RefusalWorkspace::UnsupportedLanguage,
    line: 1,
    column: 1,
    expected_reason: "unsupported_language",
    snapshot_name: "graph_slice_refusal_unsupported_language",
})]
#[case::no_symbol_at_position(RefusalSnapshotCase {
    workspace: RefusalWorkspace::Fixture(RUST_CASES[19]),
    line: 2,
    column: 1,
    expected_reason: "no_symbol_at_position",
    snapshot_name: "graph_slice_refusal_no_symbol_at_position",
})]
#[case::position_out_of_range(RefusalSnapshotCase {
    workspace: RefusalWorkspace::Fixture(RUST_CASES[0]),
    line: 10_000,
    column: 1,
    expected_reason: "position_out_of_range",
    snapshot_name: "graph_slice_refusal_position_out_of_range",
})]
fn graph_slice_refusal_snapshots(
    #[case] case: RefusalSnapshotCase,
    snapshot_harness: SnapshotHarness,
) {
    run_refusal_snapshot(snapshot_harness, case);
}
