//! End-to-end snapshots for `observe graph-slice`.

#[path = "support/fixture_io.rs"]
mod fixture_io;
#[path = "test_support/graph_slice.rs"]
mod graph_slice_support;
mod test_support;
#[path = "support/weaver_binary.rs"]
mod weaver_binary;

use graph_slice_support::{
    GraphSliceRequest,
    assert_graph_slice_envelope,
    assert_populated_slice,
    assert_refusal,
    assert_truncated_to_single_card,
    is_success,
    run_graph_slice,
};
use rstest::{fixture, rstest};
use tempfile::TempDir;
use test_support::{TestDaemon, assert_named_snapshot, fixture_uri, path_uri};
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
    /// Writes `case` into a fresh temporary workspace and returns its URI.
    ///
    /// # Errors
    /// Returns a description if the temporary directory or the fixture file
    /// cannot be created.
    fn workspace_for_case(case: GraphSliceFixtureCase) -> Result<WorkspaceUri, String> {
        let temp_dir = TempDir::new().map_err(|error| format!("creating temp dir: {error}"))?;
        let uri = fixture_uri(&temp_dir, case)?;
        Ok(WorkspaceUri {
            _temp_dir: temp_dir,
            uri,
        })
    }

    /// Builds a workspace holding a file the language router cannot classify.
    ///
    /// # Errors
    /// Returns a description if the temporary directory or the fixture file
    /// cannot be created.
    fn unsupported_workspace() -> Result<WorkspaceUri, String> {
        let temp_dir = TempDir::new().map_err(|error| format!("creating temp dir: {error}"))?;
        let path = write_fixture_path(&temp_dir, "notes.txt", "plain text\n")
            .map_err(|error| format!("write unsupported fixture: {error}"))?;
        let uri = path_uri(&path)?;
        Ok(WorkspaceUri {
            _temp_dir: temp_dir,
            uri,
        })
    }

    /// Starts a daemon expecting `expected_requests`, or the harness default.
    ///
    /// # Errors
    /// Returns a description if the daemon cannot bind or start serving.
    fn daemon(self, expected_requests: Option<usize>) -> Result<TestDaemon, String> {
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
///
/// # Errors
/// Returns a description if the transcript cannot be serialised.
fn render_snapshot<T: serde::Serialize>(transcript: &T) -> Result<String, String> {
    serde_json::to_string_pretty(transcript)
        .map_err(|error| format!("serialize transcript: {error}"))
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
) -> Result<(), String> {
    let workspace = SnapshotHarness::workspace_for_case(case)?;
    let daemon = snapshot_harness.daemon(None)?;
    let transcript = run_graph_slice(
        &daemon,
        SnapshotHarness::request(&workspace.uri, case.line, case.column, None),
    )?;
    // Parse and assert structural shape so regressions surface even if snapshots are not reviewed.
    let snapshot_name = format!("graph_slice_{}", case.name);
    let value = assert_graph_slice_envelope(&transcript, &snapshot_name)?;
    if is_success(&value) {
        assert_populated_slice(&value, &snapshot_name);
    } else {
        assert_refusal(&value, "no_symbol_at_position", &snapshot_name);
    }

    daemon.join()?;
    assert_named_snapshot(&snapshot_name, &render_snapshot(&transcript)?);
    Ok(())
}

#[rstest]
#[case::rust_multi_symbol(RUST_CASES[7])]
#[case::python_multi_symbol(PYTHON_CASES[5])]
fn graph_slice_truncation_snapshots(
    #[case] case: GraphSliceFixtureCase,
    snapshot_harness: SnapshotHarness,
) -> Result<(), String> {
    let workspace = SnapshotHarness::workspace_for_case(case)?;
    let daemon = snapshot_harness.daemon(None)?;
    let transcript = run_graph_slice(
        &daemon,
        SnapshotHarness::request(&workspace.uri, case.line, case.column, Some(1)),
    )?;
    let snapshot_name = format!("graph_slice_truncated_{}", case.name);
    let value = assert_graph_slice_envelope(&transcript, &snapshot_name)?;
    if is_success(&value) {
        assert_truncated_to_single_card(&value, &snapshot_name);
    } else {
        assert_refusal(&value, "no_symbol_at_position", &snapshot_name);
    }

    daemon.join()?;
    assert_named_snapshot(&snapshot_name, &render_snapshot(&transcript)?);
    Ok(())
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
    /// Materialises the workspace this case runs against.
    ///
    /// # Errors
    /// Returns a description if the temporary directory or the fixture file
    /// cannot be created.
    fn build(self) -> Result<WorkspaceUri, String> {
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

/// Runs one refusal case end to end and records its snapshot.
///
/// # Errors
/// Returns a description if the workspace, daemon, CLI invocation, or
/// transcript rendering fails.
fn run_refusal_snapshot(harness: SnapshotHarness, case: RefusalSnapshotCase) -> Result<(), String> {
    let workspace = case.workspace.build()?;
    let daemon = harness.daemon(None)?;
    let transcript = run_graph_slice(
        &daemon,
        SnapshotHarness::request(&workspace.uri, case.line, case.column, None),
    )?;

    let value = assert_graph_slice_envelope(&transcript, case.snapshot_name)?;
    assert_refusal(&value, case.expected_reason, case.snapshot_name);

    daemon.join()?;
    assert_named_snapshot(case.snapshot_name, &render_snapshot(&transcript)?);
    Ok(())
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
) -> Result<(), String> {
    run_refusal_snapshot(snapshot_harness, case)
}
