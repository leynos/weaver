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

/// Returns the value at a nested object path, or `None` when any level is absent.
fn json_at<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    path.iter().try_fold(value, |node, key| node.get(*key))
}

/// Returns the first element of the array-valued member at `path`, if present.
fn first_at<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    json_at(value, path)
        .and_then(serde_json::Value::as_array)
        .and_then(|items| items.first())
}

/// Returns `Err` carrying `message` unless `condition` holds.
fn ensure(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn assert_success_response(status: i32, payload: &serde_json::Value) {
    assert_eq!(status, 0, "expected success exit status");
    assert_eq!(
        json_at(payload, &["status"]),
        Some(&serde_json::json!("success")),
        "expected success payload status"
    );
    assert_eq!(
        json_at(payload, &["schema_version"]),
        Some(&serde_json::json!("graph_slice.v1"))
    );
}

fn assert_default_graph_slice_shape(payload: &serde_json::Value) {
    assert_eq!(
        json_at(payload, &["constraints", "direction"]),
        Some(&serde_json::json!("both"))
    );
    assert_eq!(
        json_at(payload, &["constraints", "budget", "max_cards"]),
        Some(&serde_json::json!(30))
    );
    assert_eq!(json_at(payload, &["edges"]), Some(&serde_json::json!([])));
}

fn assert_spillover_truncated_with_frontier(payload: &serde_json::Value) {
    assert_eq!(
        json_at(payload, &["spillover", "truncated"]),
        Some(&serde_json::json!(true)),
        "expected truncated"
    );
    let Some(frontier) =
        json_at(payload, &["spillover", "frontier"]).and_then(serde_json::Value::as_array)
    else {
        panic!("frontier array");
    };
    assert!(!frontier.is_empty(), "expected non-empty frontier");
    for (index, entry) in frontier.iter().enumerate() {
        assert!(
            entry
                .get("symbol_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|s| !s.is_empty()),
            "frontier entry {index} should have a non-empty symbol_id"
        );
        assert_eq!(
            entry.get("depth"),
            Some(&serde_json::json!(1)),
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
fn symbol_ids(cards: Option<&serde_json::Value>) -> Result<Vec<&str>, String> {
    let cards_value = cards.ok_or_else(|| "cards should be an array".to_owned())?;
    collect_ids_from_array(
        cards_value,
        "cards should be an array",
        "symbol_id should be a string",
        |card| json_at(card, &["symbol", "symbol_id"]).and_then(serde_json::Value::as_str),
    )
}

/// Collects `symbol_id` from every entry in a spillover frontier array.
fn frontier_ids(frontier: Option<&serde_json::Value>) -> Result<Vec<&str>, String> {
    let frontier_value = frontier.ok_or_else(|| "frontier should be an array".to_owned())?;
    collect_ids_from_array(
        frontier_value,
        "frontier should be an array",
        "frontier symbol_id should be a string",
        |entry| entry.get("symbol_id").and_then(serde_json::Value::as_str),
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

/// Checks the refusal envelope shape, returning an error on the calling line.
macro_rules! assert_refusal {
    ($status:expr, $payload:expr, $reason:expr) => {{
        let refusal_reason: &str = $reason;
        let refusal_payload = &$payload;
        let expected_status = refusal_status(refusal_reason)
            .ok_or_else(|| format!("unknown refusal reason {refusal_reason}"))?;
        let actual_status = $status;
        ensure(
            actual_status == expected_status,
            format!("expected status {expected_status}, got {actual_status}"),
        )?;
        ensure(
            json_at(refusal_payload, &["status"]).and_then(serde_json::Value::as_str)
                == Some("refusal"),
            "expected a refusal payload status",
        )?;
        ensure(
            json_at(refusal_payload, &["schema_version"]).and_then(serde_json::Value::as_str)
                == Some("graph_slice.v1"),
            "expected schema_version graph_slice.v1",
        )?;
        ensure(
            json_at(refusal_payload, &["refusal", "reason"]).and_then(serde_json::Value::as_str)
                == Some(refusal_reason),
            format!("expected refusal reason {refusal_reason}"),
        )?;
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
    ensure(
        json_at(&payload, &["constraints", "entry_detail"]).and_then(serde_json::Value::as_str)
            == Some("structure"),
        "expected structure entry detail",
    )?;
    ensure(
        json_at(&payload, &["constraints", "node_detail"]).and_then(serde_json::Value::as_str)
            == Some("semantic"),
        "expected semantic node detail",
    )?;
    ensure(
        json_at(&payload, &["spillover", "truncated"]) == Some(&serde_json::json!(false)),
        "expected spillover not truncated",
    )?;
    ensure(
        first_at(&payload, &["cards"])
            .and_then(|card| json_at(card, &["symbol", "ref", "name"]))
            .and_then(serde_json::Value::as_str)
            == Some("increment"),
        "expected the increment card name",
    )?;
    let first_symbol_ids = symbol_ids(json_at(&payload, &["cards"]))?;
    let second_symbol_ids = symbol_ids(json_at(&second_payload, &["cards"]))?;
    ensure(
        first_symbol_ids.len() >= 2,
        format!(
            "expected at least two symbols, got {}",
            first_symbol_ids.len()
        ),
    )?;
    ensure(
        first_symbol_ids == second_symbol_ids,
        "expected stable symbol ids across identical requests",
    )?;
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
    ensure(
        json_at(&payload, &["cards"])
            .and_then(serde_json::Value::as_array)
            .is_some_and(|cards| cards.len() == 1),
        "expected exactly one card",
    )?;
    ensure(
        json_at(&payload, &["constraints", "budget", "max_cards"]) == Some(&serde_json::json!(1)),
        "expected a max_cards budget of 1",
    )?;
    assert_success_response(second_status, &second_payload);
    ensure(
        first_at(&payload, &["cards"])
            .and_then(|card| json_at(card, &["symbol", "ref", "name"]))
            .and_then(serde_json::Value::as_str)
            == Some("build"),
        "expected the build card name",
    )?;
    let kept_symbol_id = first_at(&payload, &["cards"])
        .and_then(|card| json_at(card, &["symbol", "symbol_id"]))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "kept symbol_id should be a string".to_owned())?;
    let first_frontier = frontier_ids(json_at(&payload, &["spillover", "frontier"]))?;
    let second_frontier = frontier_ids(json_at(&second_payload, &["spillover", "frontier"]))?;
    ensure(
        !first_frontier.contains(&kept_symbol_id),
        "expected the kept symbol to be absent from the frontier",
    )?;
    ensure(
        first_frontier == second_frontier,
        "expected stable frontier ids across identical requests",
    )?;
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
        ensure(
            json_at(&payload, &["refusal", "message"]).and_then(serde_json::Value::as_str)
                == Some(message),
            format!("expected refusal message {message}"),
        )?;
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
