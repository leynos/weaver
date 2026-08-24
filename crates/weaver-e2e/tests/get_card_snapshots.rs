//! End-to-end snapshots for `observe get-card`.

#[path = "support/fixture_io.rs"]
mod fixture_io;
#[path = "test_support/get_card.rs"]
mod get_card_support;
mod test_support;
#[path = "support/weaver_binary.rs"]
mod weaver_binary;

use fixture_io::write_fixture_path;
use get_card_support::{CacheTranscript, GetCardRequest, run_get_card};
use rstest::{fixture, rstest};
use tempfile::TempDir;
use test_support::{TestDaemon, assert_named_snapshot, fixture_uri, path_uri};
use weaver_e2e::card_fixtures::{PYTHON_CASES, RUST_CASES};

struct WorkspaceUri {
    _temp_dir: TempDir,
    uri: String,
}

#[derive(Clone, Copy)]
struct RequestSpec {
    line: u32,
    column: u32,
    detail: &'static str,
}

#[derive(Clone, Copy)]
struct RefusalCase {
    uses_unsupported_workspace: bool,
    request: RequestSpec,
    snapshot_name: &'static str,
}

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
    fn workspace_for_case(
        case: weaver_e2e::card_fixtures::CardFixtureCase,
    ) -> Result<WorkspaceUri, String> {
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
        let uri = unsupported_fixture_uri(&temp_dir)?;
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

    const fn request(uri: &str, spec: RequestSpec) -> GetCardRequest<'_> {
        GetCardRequest {
            uri,
            line: spec.line,
            column: spec.column,
            detail: spec.detail,
        }
    }
}

#[fixture]
const fn snapshot_harness() -> SnapshotHarness {
    SnapshotHarness {
        default_expected_requests: 1,
    }
}

/// Writes a plain-text fixture and returns its `file://` URI.
///
/// # Errors
/// Returns a description if the file cannot be written or its path has no
/// `file://` URI representation.
fn unsupported_fixture_uri(temp_dir: &TempDir) -> Result<String, String> {
    let path = write_fixture_path(temp_dir, "notes.txt", "plain text\n")
        .map_err(|error| format!("write fixture path: {error}"))?;
    path_uri(&path)
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
fn get_card_structure_snapshots_cover_python_and_rust_fixture_battery(
    #[case] case: weaver_e2e::card_fixtures::CardFixtureCase,
    snapshot_harness: SnapshotHarness,
) -> Result<(), String> {
    let workspace = SnapshotHarness::workspace_for_case(case)?;
    let daemon = snapshot_harness.daemon(None)?;
    let transcript = run_get_card(
        &daemon,
        SnapshotHarness::request(
            &workspace.uri,
            RequestSpec {
                line: case.line,
                column: case.column,
                detail: "structure",
            },
        ),
    )?;
    daemon.join()?;
    assert_named_snapshot(case.name, &render_snapshot(&transcript)?);
    Ok(())
}

#[rstest]
#[case("minimal")]
#[case("signature")]
#[case("structure")]
fn get_card_detail_levels_snapshot(
    #[case] detail: &'static str,
    snapshot_harness: SnapshotHarness,
) -> Result<(), String> {
    let case = RUST_CASES[0];
    let workspace = SnapshotHarness::workspace_for_case(case)?;
    let daemon = snapshot_harness.daemon(None)?;
    let transcript = run_get_card(
        &daemon,
        SnapshotHarness::request(
            &workspace.uri,
            RequestSpec {
                line: case.line,
                column: case.column,
                detail,
            },
        ),
    )?;
    daemon.join()?;
    assert_named_snapshot(
        &format!("rust_detail_{detail}"),
        &render_snapshot(&transcript)?,
    );
    Ok(())
}

#[rstest]
#[case::unsupported_language(RefusalCase {
    uses_unsupported_workspace: true,
    request: RequestSpec {
        line: 1,
        column: 1,
        detail: "structure",
    },
    snapshot_name: "refusal_unsupported_language",
})]
#[case::position_out_of_range(RefusalCase {
    uses_unsupported_workspace: false,
    request: RequestSpec {
        line: 99,
        column: 99,
        detail: "structure",
    },
    snapshot_name: "refusal_position_out_of_range",
})]
fn get_card_refusal_snapshots(
    #[case] refusal_case: RefusalCase,
    snapshot_harness: SnapshotHarness,
) -> Result<(), String> {
    let workspace = if refusal_case.uses_unsupported_workspace {
        SnapshotHarness::unsupported_workspace()?
    } else {
        SnapshotHarness::workspace_for_case(RUST_CASES[0])?
    };
    let daemon = snapshot_harness.daemon(None)?;
    let transcript = run_get_card(
        &daemon,
        SnapshotHarness::request(&workspace.uri, refusal_case.request),
    )?;
    daemon.join()?;
    assert_named_snapshot(refusal_case.snapshot_name, &render_snapshot(&transcript)?);
    Ok(())
}

#[rstest]
fn get_card_repeated_request_uses_cache_snapshot(
    snapshot_harness: SnapshotHarness,
) -> Result<(), String> {
    let case = RUST_CASES[0];
    let workspace = SnapshotHarness::workspace_for_case(case)?;
    let daemon = snapshot_harness.daemon(Some(2))?;
    let request = SnapshotHarness::request(
        &workspace.uri,
        RequestSpec {
            line: case.line,
            column: case.column,
            detail: "structure",
        },
    );
    let first = run_get_card(&daemon, request)?;
    let second = run_get_card(&daemon, request)?;
    let stats = daemon.cache_stats()?;
    daemon.join()?;

    let transcript = CacheTranscript {
        first,
        second,
        cache_hits: stats.hits,
        cache_misses: stats.misses,
    };
    assert_named_snapshot("cache_repeated_request", &render_snapshot(&transcript)?);
    Ok(())
}
