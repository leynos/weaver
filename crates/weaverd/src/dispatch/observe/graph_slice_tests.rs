//! Unit tests for the `observe graph-slice` dispatch handler.
use rstest::rstest;
use tempfile::TempDir;
use weaver_cards::DetailLevel;

use self::support::{
    SourceRequest,
    backends_fixture,
    detail_value,
    dispatch_payload,
    dispatch_source,
    prepare_request,
};
use super::{MAX_SAME_FILE_DISCOVERY_POSITIONS, first_non_whitespace_column};
use crate::{backends::FusionBackends, semantic_provider::SemanticBackendProvider};

#[path = "graph_slice_test_support.rs"]
mod support;

fn assert_success_response(status: i32, payload: &serde_json::Value) {
    assert_eq!(status, 0, "expected success exit status");
    assert_eq!(
        payload["status"], "success",
        "expected success payload status"
    );
    assert_eq!(payload["schema_version"], "graph_slice.v1");
}

fn assert_default_graph_slice_shape(payload: &serde_json::Value) {
    assert_eq!(payload["constraints"]["direction"], "both");
    assert_eq!(
        payload["constraints"]["budget"]["max_cards"],
        serde_json::json!(30)
    );
    assert_eq!(payload["edges"], serde_json::json!([]));
}

fn assert_spillover_truncated_with_frontier(payload: &serde_json::Value) {
    assert_eq!(
        payload["spillover"]["truncated"], true,
        "expected truncated"
    );
    let frontier = match payload["spillover"]["frontier"].as_array() {
        Some(frontier) => frontier,
        None => panic!("frontier array"),
    };
    assert!(!frontier.is_empty(), "expected non-empty frontier");
    for (index, entry) in frontier.iter().enumerate() {
        assert!(
            entry["symbol_id"].as_str().is_some_and(|s| !s.is_empty()),
            "frontier entry {index} should have a non-empty symbol_id"
        );
        assert_eq!(
            entry["depth"],
            serde_json::json!(1),
            "frontier entry {index} depth should be 1"
        );
    }
}

/// Collects an identifier from every entry of a JSON array, reporting the
/// supplied message when the value is not an array or an entry lacks a
/// string identifier.
///
/// Callers supply the diagnostics so each collection names the payload shape
/// it expected.
fn collect_ids_from_array<'a>(
    values: &'a serde_json::Value,
    array_error: &'static str,
    id_error: &'static str,
    extract_id: impl Fn(&'a serde_json::Value) -> Option<&'a str>,
) -> Result<Vec<&'a str>, String> {
    values
        .as_array()
        .ok_or_else(|| array_error.to_owned())?
        .iter()
        .map(|value| extract_id(value).ok_or_else(|| id_error.to_owned()))
        .collect()
}

/// Collects `symbol.symbol_id` from every card in a `cards` array.
fn symbol_ids(cards: &serde_json::Value) -> Result<Vec<&str>, String> {
    collect_ids_from_array(
        cards,
        "cards should be an array",
        "symbol_id should be a string",
        |card| card["symbol"]["symbol_id"].as_str(),
    )
}

/// Collects `symbol_id` from every entry in a spillover frontier array.
fn frontier_ids(frontier: &serde_json::Value) -> Result<Vec<&str>, String> {
    collect_ids_from_array(
        frontier,
        "frontier should be an array",
        "frontier symbol_id should be a string",
        |entry| entry["symbol_id"].as_str(),
    )
}

/// Maps a structured refusal reason onto the exit status the handler reports.
fn refusal_status(reason: &str) -> Option<i32> {
    match reason {
        "unsupported_language" => Some(10),
        "no_symbol_at_position" => Some(11),
        "position_out_of_range" => Some(12),
        "not_yet_implemented" => Some(13),
        "backend_unavailable" => Some(14),
        _ => None,
    }
}

/// Asserts the refusal envelope shape, keeping failures on the calling line.
macro_rules! assert_refusal {
    ($status:expr, $payload:expr, $reason:expr) => {{
        let reason: &str = $reason;
        let payload = &$payload;
        let expected_status =
            refusal_status(reason).unwrap_or_else(|| panic!("unknown refusal reason {reason}"));
        assert_eq!($status, expected_status);
        assert_eq!(payload["status"], "refusal");
        assert_eq!(payload["schema_version"], "graph_slice.v1");
        assert_eq!(payload["refusal"]["reason"], reason);
    }};
}

#[rstest]
fn valid_request_returns_success_and_echoed_constraints(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let source = SourceRequest {
        temp_dir: &temp_dir,
        filename: "slice.rs",
        content: concat!(
            "struct Counter(u32);\n\n",
            "impl Counter {\n",
            "    fn increment(&mut self) {\n",
            "        self.0 += 1;\n",
            "    }\n",
            "}\n"
        ),
        arguments: &[
            "--position",
            "4:8",
            "--entry-detail",
            detail_value(DetailLevel::Structure),
            "--node-detail",
            detail_value(DetailLevel::Semantic),
        ],
    };

    let request = prepare_request(&source)?;
    let (status, payload) = dispatch_payload(&request, &mut backends)?;
    let (second_status, second_payload) = dispatch_payload(&request, &mut backends)?;

    assert_success_response(status, &payload);
    assert_success_response(second_status, &second_payload);
    assert_default_graph_slice_shape(&payload);
    assert_eq!(payload["constraints"]["entry_detail"], "structure");
    assert_eq!(payload["constraints"]["node_detail"], "semantic");
    assert_eq!(payload["spillover"]["truncated"], false);
    assert_eq!(payload["cards"][0]["symbol"]["ref"]["name"], "increment");
    let first_symbol_ids = symbol_ids(&payload["cards"])?;
    let second_symbol_ids = symbol_ids(&second_payload["cards"])?;
    assert!(first_symbol_ids.len() >= 2);
    assert_eq!(first_symbol_ids, second_symbol_ids);
    Ok(())
}

#[rstest]
fn max_cards_budget_truncates_same_file_symbol_inventory(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let source = SourceRequest {
        temp_dir: &temp_dir,
        filename: "slice.py",
        content: concat!(
            "class Factory:\n",
            "    @classmethod\n",
            "    def build(cls) -> \"Factory\":\n",
            "        return cls()\n\n",
            "    @staticmethod\n",
            "    def version() -> str:\n",
            "        return \"1.0\"\n"
        ),
        arguments: &[
            "--position",
            "3:9",
            "--max-cards",
            "1",
            "--entry-detail",
            detail_value(DetailLevel::Semantic),
            "--node-detail",
            detail_value(DetailLevel::Semantic),
        ],
    };

    let request = prepare_request(&source)?;
    let (status, payload) = dispatch_payload(&request, &mut backends)?;
    let (second_status, second_payload) = dispatch_payload(&request, &mut backends)?;

    assert_success_response(status, &payload);
    assert_eq!(payload["cards"].as_array().expect("cards array").len(), 1);
    assert_eq!(
        payload["constraints"]["budget"]["max_cards"],
        serde_json::json!(1)
    );
    assert_success_response(second_status, &second_payload);
    assert_eq!(payload["cards"][0]["symbol"]["ref"]["name"], "build");
    let kept_symbol_id = payload["cards"][0]["symbol"]["symbol_id"]
        .as_str()
        .expect("kept symbol_id should be a string");
    let first_frontier = frontier_ids(&payload["spillover"]["frontier"])?;
    let second_frontier = frontier_ids(&second_payload["spillover"]["frontier"])?;
    assert!(!first_frontier.contains(&kept_symbol_id));
    assert_eq!(first_frontier, second_frontier);
    assert_spillover_truncated_with_frontier(&payload);
    Ok(())
}

#[path = "argument_tests.rs"]
mod argument_tests;
#[path = "coverage_tests.rs"]
mod coverage_tests;

#[rstest]
#[case(("notes.txt", "plain text\n", "1:1", "unsupported_language", None))]
#[case(
    (
        "main.rs",
        "fn main() {}\n \n",
        "2:1",
        "no_symbol_at_position",
        Some("observe graph-slice: no symbol found at 2:1"),
    )
)]
#[case(
    (
        "main.rs",
        "fn main() {}\n",
        "10:1",
        "position_out_of_range",
        Some("observe graph-slice: position 10:1 is outside the bounds of the file"),
    )
)]
fn structured_refusal_cases(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
    #[case] case: (&str, &str, &str, &str, Option<&str>),
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let (filename, content, position, expected_reason, expected_message) = case;

    let (status, payload) = dispatch_source(
        &mut backends,
        &SourceRequest {
            temp_dir: &temp_dir,
            filename,
            content,
            arguments: &["--position", position],
        },
    )?;

    assert_refusal!(status, payload, expected_reason);
    if let Some(message) = expected_message {
        assert_eq!(payload["refusal"]["message"], message);
    }
    Ok(())
}

#[test]
fn first_non_whitespace_column_uses_character_offsets() {
    assert_eq!(
        first_non_whitespace_column("\u{2003}\u{2003}fn main() {}"),
        Some(3)
    );
}
