//! Additional coverage for `observe graph-slice` response invariants.

use rstest::rstest;
use serde_json::Value;
use tempfile::TempDir;
use weaver_cards::DetailLevel;
use weaver_lsp_host::{Language, ServerCapabilitySet};

use super::{
    FusionBackends,
    SemanticBackendProvider,
    assert_success_response,
    support::{
        SourceRequest,
        backends_fixture,
        detail_value,
        dispatch_payload,
        dispatch_source,
        prepare_request,
    },
};
use crate::{
    dispatch::observe::test_support::{
        StubLanguageServer,
        markdown_hover,
        semantic_backends_with_server,
    },
    test_helpers::{ensure_same, ensure_same_ref, ensure_true},
};

/// Returns the `key` member of a JSON object, reporting a missing field.
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("missing field `{key}`"))
}

/// Returns the `key` member of a JSON object as an array.
fn array_field<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("field `{key}` is not an array"))
}

/// Returns the `key` member of a JSON object as a string.
fn string_field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("field `{key}` is not a string"))
}

/// Collects the `symbol.symbol_id` of every card, in card order.
fn symbol_ids(cards: &[Value]) -> Result<Vec<&str>, String> {
    cards
        .iter()
        .map(|card| field(card, "symbol").and_then(|symbol| string_field(symbol, "symbol_id")))
        .collect()
}

/// Renders `count` distinct single-symbol functions as Rust source.
fn render_functions(count: usize) -> String {
    (0..count).fold(String::new(), |mut source, index| {
        source.push_str("fn item_");
        source.push_str(&index.to_string());
        source.push_str("() {}\n");
        source
    })
}

#[rstest]
fn enrichment_applies_lsp_provenance_when_detail_is_semantic() -> Result<(), String> {
    let (server, _hover_params) = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        markdown_hover("```rust\nfn increment(&mut self)\n```"),
    );
    let (mut backends, temp_dir) = semantic_backends_with_server(Language::Rust, server)?;
    let source = SourceRequest {
        temp_dir: &temp_dir,
        filename: "enrich.rs",
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
            detail_value(DetailLevel::Semantic),
            "--node-detail",
            detail_value(DetailLevel::Semantic),
        ],
    };

    let (status, payload) = dispatch_source(&mut backends, &source)?;

    assert_success_response(status, &payload);

    // The entry card provenance must include lsp_hover after semantic enrichment.
    let cards = array_field(&payload, "cards")?;
    let entry_card = cards.first().ok_or("cards should include an entry card")?;
    let provenance = field(entry_card, "provenance")?;
    let provenance_sources = array_field(provenance, "sources")?;
    let source_names = provenance_sources
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or("provenance.sources must contain only strings")
        })
        .collect::<Result<Vec<_>, _>>()?;

    ensure_true(
        source_names.contains(&"lsp_hover"),
        concat!(
            "entry card provenance to include lsp_hover after semantic ",
            "enrichment",
        ),
    )?;
    ensure_true(
        !source_names.contains(&"tree_sitter_degraded_semantic"),
        "entry card provenance to drop tree_sitter_degraded_semantic",
    )
}

#[rstest]
fn stable_card_order_produces_deterministic_results(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let request = prepare_request(&SourceRequest {
        temp_dir: &temp_dir,
        filename: "order.rs",
        content: concat!(
            "struct Alpha(u32);\n\n",
            "impl Alpha {\n",
            "    fn first(&self) -> u32 { self.0 }\n",
            "    fn second(&self) -> u32 { self.0 + 1 }\n",
            "    fn third(&self) -> u32 { self.0 + 2 }\n",
            "}\n"
        ),
        arguments: &["--position", "4:8"],
    })?;

    let (status_a, payload_a) = dispatch_payload(&request, &mut backends)?;
    let (mut fresh_backends, _fresh_backend_dir) = super::support::backends_fixture()?;
    let (status_b, payload_b) = dispatch_payload(&request, &mut fresh_backends)?;

    assert_success_response(status_a, &payload_a);
    assert_success_response(status_b, &payload_b);

    let ids_a = symbol_ids(array_field(&payload_a, "cards")?)?;
    let ids_b = symbol_ids(array_field(&payload_b, "cards")?)?;

    ensure_same_ref(&ids_a, &ids_b, "deterministic card order across requests")
}

#[rstest]
fn single_symbol_file_returns_one_card_with_empty_frontier(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let source = SourceRequest {
        temp_dir: &temp_dir,
        filename: "solo.rs",
        content: "fn solo() {}\n",
        arguments: &["--position", "1:4"],
    };

    let (status, payload) = dispatch_source(&mut backends, &source)?;

    assert_success_response(status, &payload);

    let cards = array_field(&payload, "cards")?;
    ensure_same(cards.len(), 1, "single-symbol card count")?;

    let spillover = field(&payload, "spillover")?;
    ensure_same(
        spillover.get("truncated").and_then(Value::as_bool),
        Some(false),
        "spillover.truncated",
    )?;

    let frontier = array_field(spillover, "frontier")?;
    ensure_true(frontier.is_empty(), "empty spillover frontier")
}

#[rstest]
fn discovery_cap_marks_spillover_truncated_when_card_budget_remains(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    use super::MAX_SAME_FILE_DISCOVERY_POSITIONS;

    let (mut backends, temp_dir) = backends_fixture?;
    let source = render_functions(MAX_SAME_FILE_DISCOVERY_POSITIONS + 1);
    let (status, payload) = dispatch_source(
        &mut backends,
        &SourceRequest {
            temp_dir: &temp_dir,
            filename: "large.rs",
            content: &source,
            arguments: &[
                "--position",
                "1:4",
                "--max-cards",
                "300",
                "--entry-detail",
                detail_value(DetailLevel::Structure),
                "--node-detail",
                detail_value(DetailLevel::Structure),
            ],
        },
    )?;

    assert_success_response(status, &payload);

    let spillover = field(&payload, "spillover")?;
    ensure_same(
        spillover.get("truncated").and_then(Value::as_bool),
        Some(true),
        "spillover.truncated",
    )?;
    let frontier_len = spillover
        .get("frontier")
        .and_then(Value::as_array)
        .map(Vec::len);
    ensure_same(frontier_len, Some(0), "spillover.frontier length")
}
