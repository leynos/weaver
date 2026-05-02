//! Additional coverage for `observe graph-slice` response invariants.

use rstest::rstest;
use tempfile::TempDir;
use url::Url;
use weaver_cards::DetailLevel;
use weaver_lsp_host::{Language, ServerCapabilitySet};

use super::{
    FusionBackends,
    SemanticBackendProvider,
    assert_success_response,
    backends_fixture,
    detail_value,
    dispatch_payload,
    make_request,
    write_source,
};
use crate::dispatch::observe::test_support::{
    StubLanguageServer,
    markdown_hover,
    semantic_backends_with_server,
};

#[rstest]
fn enrichment_applies_lsp_provenance_when_detail_is_semantic() -> Result<(), String> {
    let (server, _hover_params) = StubLanguageServer::with_hover(
        ServerCapabilitySet::new(false, false, false).with_hover(true),
        markdown_hover("```rust\nfn increment(&mut self)\n```"),
    );
    let (mut backends, temp_dir) = semantic_backends_with_server(Language::Rust, server)?;
    let path = write_source(
        &temp_dir,
        "enrich.rs",
        concat!(
            "struct Counter(u32);\n\n",
            "impl Counter {\n",
            "    fn increment(&mut self) {\n",
            "        self.0 += 1;\n",
            "    }\n",
            "}\n"
        ),
    )
    .map_err(|error| error.to_string())?;
    let uri = Url::from_file_path(&path)
        .map_err(|()| "file uri".to_string())?
        .to_string();
    let request = make_request(&[
        "--uri",
        &uri,
        "--position",
        "4:8",
        "--entry-detail",
        detail_value(DetailLevel::Semantic),
        "--node-detail",
        detail_value(DetailLevel::Semantic),
    ]);

    let (status, payload) = dispatch_payload(&request, &mut backends)?;

    assert_success_response(status, &payload);

    // The entry card provenance must include lsp_hover after semantic enrichment.
    let entry_card = payload
        .get("cards")
        .and_then(|value| value.as_array())
        .and_then(|cards| cards.first())
        .expect("payload should contain at least one card");
    let provenance_sources = entry_card["provenance"]["sources"]
        .as_array()
        .expect("provenance.sources should be an array");
    let source_names: Vec<&str> = provenance_sources
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("provenance_sources entries should be strings")
        })
        .collect();
    assert!(
        source_names.contains(&"lsp_hover"),
        "entry card provenance should include lsp_hover after semantic enrichment, got: \
         {source_names:?}"
    );
    assert!(
        !source_names.contains(&"tree_sitter_degraded_semantic"),
        "entry card provenance should not contain tree_sitter_degraded_semantic after enrichment, \
         got: {source_names:?}"
    );

    Ok(())
}

#[rstest]
fn stable_card_order_produces_deterministic_results(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let path = write_source(
        &temp_dir,
        "order.rs",
        concat!(
            "struct Alpha(u32);\n\n",
            "impl Alpha {\n",
            "    fn first(&self) -> u32 { self.0 }\n",
            "    fn second(&self) -> u32 { self.0 + 1 }\n",
            "    fn third(&self) -> u32 { self.0 + 2 }\n",
            "}\n"
        ),
    )
    .map_err(|error| error.to_string())?;
    let uri = Url::from_file_path(&path)
        .map_err(|()| "file uri".to_string())?
        .to_string();
    let request = make_request(&["--uri", &uri, "--position", "4:8"]);

    let (status_a, payload_a) = dispatch_payload(&request, &mut backends)?;
    let (mut fresh_backends, _fresh_backend_dir) = super::backends_fixture()?;
    let (status_b, payload_b) = dispatch_payload(&request, &mut fresh_backends)?;

    assert_success_response(status_a, &payload_a);
    assert_success_response(status_b, &payload_b);

    let cards_a = payload_a["cards"].as_array().expect("cards array");
    let cards_b = payload_b["cards"].as_array().expect("cards array");

    let ids_a: Vec<_> = cards_a
        .iter()
        .map(|card| {
            card["symbol"]["symbol_id"]
                .as_str()
                .expect("card should include symbol.symbol_id")
        })
        .collect();
    let ids_b: Vec<_> = cards_b
        .iter()
        .map(|card| {
            card["symbol"]["symbol_id"]
                .as_str()
                .expect("card should include symbol.symbol_id")
        })
        .collect();

    assert_eq!(
        ids_a, ids_b,
        "card order must be deterministic across repeated requests"
    );

    Ok(())
}

#[rstest]
fn single_symbol_file_returns_one_card_with_empty_frontier(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    let (mut backends, temp_dir) = backends_fixture?;
    let path =
        write_source(&temp_dir, "solo.rs", "fn solo() {}\n").map_err(|error| error.to_string())?;
    let uri = Url::from_file_path(&path)
        .map_err(|()| "file uri".to_string())?
        .to_string();
    let request = make_request(&["--uri", &uri, "--position", "1:4"]);

    let (status, payload) = dispatch_payload(&request, &mut backends)?;

    assert_success_response(status, &payload);

    let cards = payload["cards"].as_array().expect("cards array");
    assert_eq!(
        cards.len(),
        1,
        "single-symbol file should produce exactly one card"
    );

    assert_eq!(
        payload["spillover"]["truncated"], false,
        "single-symbol file should not be truncated"
    );

    let frontier = payload["spillover"]["frontier"]
        .as_array()
        .expect("frontier array");
    assert!(
        frontier.is_empty(),
        "single-symbol file should have an empty spillover frontier"
    );

    Ok(())
}

#[rstest]
fn discovery_cap_marks_spillover_truncated_when_card_budget_remains(
    backends_fixture: Result<(FusionBackends<SemanticBackendProvider>, TempDir), String>,
) -> Result<(), String> {
    use super::MAX_SAME_FILE_DISCOVERY_POSITIONS;

    let (mut backends, temp_dir) = backends_fixture?;
    let source = (0..=MAX_SAME_FILE_DISCOVERY_POSITIONS)
        .map(|index| format!("fn item_{index}() {{}}\n"))
        .collect::<String>();
    let path = write_source(&temp_dir, "large.rs", &source).map_err(|error| error.to_string())?;
    let uri = Url::from_file_path(&path)
        .map_err(|()| "file uri".to_string())?
        .to_string();
    let request = make_request(&[
        "--uri",
        &uri,
        "--position",
        "1:4",
        "--max-cards",
        "300",
        "--entry-detail",
        detail_value(DetailLevel::Structure),
        "--node-detail",
        detail_value(DetailLevel::Structure),
    ]);

    let (status, payload) = dispatch_payload(&request, &mut backends)?;

    assert_success_response(status, &payload);
    assert_eq!(payload["spillover"]["truncated"], true);
    assert_eq!(
        payload["spillover"]["frontier"]
            .as_array()
            .expect("frontier array")
            .len(),
        0
    );
    Ok(())
}
