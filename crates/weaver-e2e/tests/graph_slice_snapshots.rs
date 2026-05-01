//! End-to-end snapshots for `observe graph-slice`.

#[path = "support/fixture_io.rs"]
mod fixture_io;
mod test_support;
#[path = "support/weaver_binary.rs"]
mod weaver_binary;

use rstest::{fixture, rstest};
use tempfile::TempDir;
use test_support::{
    GraphSliceRequest,
    TestDaemon,
    assert_named_snapshot,
    fixture_uri,
    run_graph_slice,
};
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
    {
        let raw = &transcript.stdout;
        if !raw.is_empty() {
            let value: serde_json::Value =
                serde_json::from_str(raw).expect("transcript stdout should be valid JSON");
            if value.get("status") == Some(&serde_json::json!("success")) {
                assert!(
                    value
                        .pointer("/constraints/budget/max_cards")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
                        > 0,
                    "graph_slice_{} budget.max_cards should be positive",
                    case.name
                );
                assert_eq!(
                    value.get("edges"),
                    Some(&serde_json::json!([])),
                    "graph_slice_{} edges should be empty",
                    case.name
                );
                assert!(
                    value
                        .get("spillover")
                        .is_some_and(serde_json::Value::is_object),
                    "graph_slice_{} spillover should be present",
                    case.name
                );
                assert!(
                    value
                        .get("cards")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|cards| !cards.is_empty()),
                    "graph_slice_{} cards should be non-empty",
                    case.name
                );
            } else {
                assert_eq!(
                    value.get("status"),
                    Some(&serde_json::json!("refusal")),
                    "graph_slice_{} should return success or refusal",
                    case.name
                );
                assert_eq!(
                    value.pointer("/refusal/reason"),
                    Some(&serde_json::json!("no_symbol_at_position")),
                    "graph_slice_{} refusal reason should match the snapshot contract",
                    case.name
                );
            }
        }
    }
    daemon.join();
    assert_named_snapshot(
        &format!("graph_slice_{}", case.name),
        &render_snapshot(&transcript),
    );
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
    {
        let raw = &transcript.stdout;
        if !raw.is_empty() {
            let value: serde_json::Value =
                serde_json::from_str(raw).expect("transcript stdout should be valid JSON");
            if value.get("status") == Some(&serde_json::json!("success")) {
                assert_eq!(
                    value
                        .get("cards")
                        .and_then(serde_json::Value::as_array)
                        .map_or(0, Vec::len),
                    1,
                    "graph_slice_truncated_{} should contain exactly 1 card (max_cards=1)",
                    case.name
                );
                assert_eq!(
                    value
                        .pointer("/spillover/truncated")
                        .and_then(serde_json::Value::as_bool),
                    Some(true),
                    "graph_slice_truncated_{} spillover should be truncated",
                    case.name
                );
            } else {
                assert_eq!(
                    value.get("status"),
                    Some(&serde_json::json!("refusal")),
                    "graph_slice_truncated_{} should return success or refusal",
                    case.name
                );
                assert_eq!(
                    value.pointer("/refusal/reason"),
                    Some(&serde_json::json!("no_symbol_at_position")),
                    "graph_slice_truncated_{} refusal reason should match the snapshot contract",
                    case.name
                );
            }
        }
    }
    daemon.join();
    assert_named_snapshot(
        &format!("graph_slice_truncated_{}", case.name),
        &render_snapshot(&transcript),
    );
}

#[rstest]
fn graph_slice_refusal_snapshots(snapshot_harness: SnapshotHarness) {
    let workspace = SnapshotHarness::unsupported_workspace();
    let daemon = snapshot_harness.daemon(None);
    let transcript = run_graph_slice(
        &daemon,
        SnapshotHarness::request(&workspace.uri, 1, 1, None),
    );
    {
        let raw = &transcript.stdout;
        if !raw.is_empty() {
            let value: serde_json::Value =
                serde_json::from_str(raw).expect("transcript stdout should be valid JSON");
            assert_eq!(
                value.get("status"),
                Some(&serde_json::json!("refusal")),
                "unsupported language should produce a refusal"
            );
            assert_eq!(
                value.pointer("/refusal/reason"),
                Some(&serde_json::json!("unsupported_language")),
                "refusal reason should be unsupported_language"
            );
        }
    }
    daemon.join();
    assert_named_snapshot(
        "graph_slice_refusal_unsupported_language",
        &render_snapshot(&transcript),
    );
}
