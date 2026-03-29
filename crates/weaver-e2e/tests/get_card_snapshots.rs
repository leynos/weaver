//! End-to-end snapshots for `observe get-card`.

mod test_support;

use tempfile::TempDir;
use url::Url;

use test_support::{
    CacheTranscript, GetCardRequest, TestDaemon, assert_named_snapshot, fixture_uri, run_get_card,
};
use weaver_e2e::card_fixtures::{PYTHON_CASES, RUST_CASES};

#[expect(
    clippy::expect_used,
    reason = "test helper failures should panic with explicit setup messages"
)]
fn unsupported_fixture_uri(temp_dir: &TempDir) -> String {
    let path = temp_dir.path().join("notes.txt");
    std::fs::write(&path, "plain text\n").expect("write unsupported fixture");
    Url::from_file_path(&path)
        .map(|uri| uri.to_string())
        .expect("unsupported path to URI")
}

#[test]
fn get_card_structure_snapshots_cover_python_and_rust_fixture_battery() {
    for case in PYTHON_CASES.into_iter().chain(RUST_CASES) {
        let temp_dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
        let uri = fixture_uri(&temp_dir, case);
        let daemon = TestDaemon::start(1);
        let transcript = run_get_card(
            &daemon,
            GetCardRequest {
                uri: &uri,
                line: case.line,
                column: case.column,
                detail: "structure",
            },
        );
        daemon.join();
        let rendered = serde_json::to_string_pretty(&transcript)
            .unwrap_or_else(|error| panic!("serialize transcript: {error}"));
        assert_named_snapshot(case.name, &rendered);
    }
}

#[test]
fn get_card_detail_levels_snapshot() {
    let case = RUST_CASES[0];
    for detail in ["minimal", "signature", "structure"] {
        let temp_dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
        let uri = fixture_uri(&temp_dir, case);
        let daemon = TestDaemon::start(1);
        let transcript = run_get_card(
            &daemon,
            GetCardRequest {
                uri: &uri,
                line: case.line,
                column: case.column,
                detail,
            },
        );
        daemon.join();
        let rendered = serde_json::to_string_pretty(&transcript)
            .unwrap_or_else(|error| panic!("serialize transcript: {error}"));
        assert_named_snapshot(&format!("rust_detail_{detail}"), &rendered);
    }
}

#[test]
fn get_card_refusal_snapshots() {
    let unsupported_dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let unsupported_uri = unsupported_fixture_uri(&unsupported_dir);
    let unsupported_daemon = TestDaemon::start(1);
    let unsupported = run_get_card(
        &unsupported_daemon,
        GetCardRequest {
            uri: &unsupported_uri,
            line: 1,
            column: 1,
            detail: "structure",
        },
    );
    unsupported_daemon.join();
    let unsupported_rendered = serde_json::to_string_pretty(&unsupported)
        .unwrap_or_else(|error| panic!("serialize transcript: {error}"));
    assert_named_snapshot("refusal_unsupported_language", &unsupported_rendered);

    let fixture = RUST_CASES[0];
    let invalid_dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let invalid_uri = fixture_uri(&invalid_dir, fixture);
    let invalid_daemon = TestDaemon::start(1);
    let invalid_position = run_get_card(
        &invalid_daemon,
        GetCardRequest {
            uri: &invalid_uri,
            line: 99,
            column: 99,
            detail: "structure",
        },
    );
    invalid_daemon.join();
    let invalid_rendered = serde_json::to_string_pretty(&invalid_position)
        .unwrap_or_else(|error| panic!("serialize transcript: {error}"));
    assert_named_snapshot("refusal_position_out_of_range", &invalid_rendered);
}

#[test]
fn get_card_repeated_request_uses_cache_snapshot() {
    let case = RUST_CASES[0];
    let temp_dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
    let uri = fixture_uri(&temp_dir, case);
    let daemon = TestDaemon::start(2);
    let request = GetCardRequest {
        uri: &uri,
        line: case.line,
        column: case.column,
        detail: "structure",
    };
    let first = run_get_card(&daemon, request);
    let second = run_get_card(&daemon, request);
    let stats = daemon.cache_stats();
    daemon.join();

    let transcript = CacheTranscript {
        first,
        second,
        cache_hits: stats.hits,
        cache_misses: stats.misses,
    };
    let rendered = serde_json::to_string_pretty(&transcript)
        .unwrap_or_else(|error| panic!("serialize transcript: {error}"));
    assert_named_snapshot("cache_repeated_request", &rendered);
}
