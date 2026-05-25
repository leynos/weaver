//! Property tests for `observe graph-slice` response invariants.

use std::collections::HashSet;

use proptest::{collection::vec, prelude::*};
use tempfile::TempDir;
use url::Url;
use weaver_cards::{
    CardLanguage,
    CardSymbolKind,
    DetailLevel,
    GraphSliceRequest,
    Provenance,
    SourcePosition,
    SourceRange,
    SymbolCard,
    SymbolIdentity,
    SymbolRef,
};

use super::{
    super::{SliceDocument, apply_card_budget, discover_same_file_cards, stable_card_order},
    backends_fixture,
    detail_value,
    dispatch_payload,
    make_request,
    write_source,
};

fn source_position_strategy() -> impl Strategy<Value = SourcePosition> {
    (0_u32..=1000, 0_u32..=200).prop_map(|(line, column)| SourcePosition { line, column })
}

fn source_range_strategy() -> impl Strategy<Value = SourceRange> {
    source_position_strategy().prop_flat_map(|start| {
        (0_u32..=20, 0_u32..=80).prop_map(move |(line_offset, column_offset)| {
            let end_line = start.line.saturating_add(line_offset);
            let end_column = if line_offset == 0 {
                start.column.saturating_add(column_offset)
            } else {
                column_offset
            };
            SourceRange {
                start,
                end: SourcePosition {
                    line: end_line,
                    column: end_column,
                },
            }
        })
    })
}

fn card_language_strategy() -> impl Strategy<Value = CardLanguage> {
    prop_oneof![
        Just(CardLanguage::Rust),
        Just(CardLanguage::Python),
        Just(CardLanguage::TypeScript),
    ]
}

fn card_symbol_kind_strategy() -> impl Strategy<Value = CardSymbolKind> {
    prop_oneof![
        Just(CardSymbolKind::Function),
        Just(CardSymbolKind::Method),
        Just(CardSymbolKind::Class),
        Just(CardSymbolKind::Interface),
        Just(CardSymbolKind::Type),
        Just(CardSymbolKind::Variable),
        Just(CardSymbolKind::Module),
        Just(CardSymbolKind::Field),
    ]
}

fn identifier_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z_][a-zA-Z0-9_]{0,24}".prop_map(String::from)
}

fn symbol_ref_strategy() -> impl Strategy<Value = SymbolRef> {
    (
        source_range_strategy(),
        card_language_strategy(),
        card_symbol_kind_strategy(),
        identifier_strategy(),
        prop::option::of(identifier_strategy()),
    )
        .prop_map(|(range, language, kind, name, container)| SymbolRef {
            uri: String::from("file:///test.rs"),
            range,
            language,
            kind,
            name,
            container,
        })
}

fn symbol_identity_strategy() -> impl Strategy<Value = SymbolIdentity> {
    ("[a-zA-Z0-9_-]{1,32}", symbol_ref_strategy()).prop_map(|(symbol_id, symbol_ref)| {
        SymbolIdentity {
            symbol_id,
            symbol_ref,
        }
    })
}

fn provenance_strategy() -> impl Strategy<Value = Provenance> {
    vec("[a-z_]{1,16}".prop_map(String::from), 1..=3).prop_map(|sources| Provenance {
        extracted_at: String::from("2026-01-01T00:00:00Z"),
        sources,
    })
}

prop_compose! {
    fn symbol_card_strategy()(
        symbol in symbol_identity_strategy(),
        provenance in provenance_strategy(),
    ) -> SymbolCard {
        SymbolCard {
            card_version: 1,
            symbol,
            signature: None,
            doc: None,
            attachments: None,
            structure: None,
            lsp: None,
            metrics: None,
            deps: None,
            interstitial: None,
            provenance,
            etag: None,
        }
    }
}

fn symbol_card_vec_strategy(min: usize, max: usize) -> impl Strategy<Value = Vec<SymbolCard>> {
    vec(symbol_card_strategy(), min..=max)
}

fn ordered_symbol_ids(cards: &[SymbolCard]) -> Vec<String> {
    cards
        .iter()
        .map(|card| card.symbol.symbol_id.clone())
        .collect()
}

fn has_total_stable_order(cards: &[SymbolCard]) -> bool {
    cards.iter().enumerate().all(|(index, left)| {
        cards
            .iter()
            .skip(index + 1)
            .all(|right| stable_card_order(left, right) != std::cmp::Ordering::Equal)
    })
}

fn rust_functions_source(function_count: usize) -> String {
    (0..function_count)
        .map(|index| format!("fn generated_{index}() -> usize {{ {index} }}\n"))
        .collect()
}

proptest! {
    #[test]
    fn apply_card_budget_never_exceeds_positive_max_cards(
        entry_card in symbol_card_strategy(),
        sibling_cards in symbol_card_vec_strategy(0, 80),
        max_cards in 1_u32..=50,
        discovery_capped in any::<bool>(),
    ) {
        let (cards, _spillover) =
            apply_card_budget(entry_card, sibling_cards, max_cards, discovery_capped);

        prop_assert!(cards.len() <= max_cards as usize);
    }

    #[test]
    fn apply_card_budget_returns_no_cards_when_max_cards_is_zero(
        entry_card in symbol_card_strategy(),
        sibling_cards in symbol_card_vec_strategy(0, 80),
        discovery_capped in any::<bool>(),
    ) {
        let (cards, _spillover) = apply_card_budget(entry_card, sibling_cards, 0, discovery_capped);

        prop_assert!(cards.is_empty());
    }

    #[test]
    fn apply_card_budget_keeps_entry_card_first_when_budget_allows_cards(
        entry_card in symbol_card_strategy(),
        sibling_cards in symbol_card_vec_strategy(0, 80),
        max_cards in 1_u32..=50,
        discovery_capped in any::<bool>(),
    ) {
        let expected_entry_id = entry_card.symbol.symbol_id.clone();
        let (cards, _spillover) =
            apply_card_budget(entry_card, sibling_cards, max_cards, discovery_capped);

        prop_assert_eq!(cards.first().map(|card| card.symbol.symbol_id.as_str()), Some(expected_entry_id.as_str()));
    }

    #[test]
    fn spillover_is_truncated_when_frontier_is_non_empty(
        entry_card in symbol_card_strategy(),
        sibling_cards in symbol_card_vec_strategy(1, 80),
        max_cards in 1_u32..=20,
        discovery_capped in any::<bool>(),
    ) {
        prop_assume!(sibling_cards.len() >= max_cards as usize);

        let (_cards, spillover) =
            apply_card_budget(entry_card, sibling_cards, max_cards, discovery_capped);

        prop_assert!(!spillover.frontier.is_empty());
        prop_assert!(spillover.truncated);
    }

    #[test]
    fn spillover_is_truncated_when_discovery_is_capped_and_budget_has_room(
        entry_card in symbol_card_strategy(),
        sibling_cards in symbol_card_vec_strategy(0, 20),
    ) {
        let max_cards = (sibling_cards.len() as u32) + 1;
        let (_cards, spillover) = apply_card_budget(entry_card, sibling_cards, max_cards, true);

        prop_assert!(spillover.frontier.is_empty());
        prop_assert!(spillover.truncated);
    }

    #[test]
    fn spillover_is_not_truncated_only_when_frontier_empty_and_discovery_uncapped(
        entry_card in symbol_card_strategy(),
        sibling_cards in symbol_card_vec_strategy(0, 20),
    ) {
        let max_cards = (sibling_cards.len() as u32) + 1;
        let (_cards, spillover) = apply_card_budget(entry_card, sibling_cards, max_cards, false);

        prop_assert!(spillover.frontier.is_empty());
        prop_assert!(!spillover.truncated);
    }

    #[test]
    fn spillover_frontier_contains_exactly_excluded_sibling_cards(
        entry_card in symbol_card_strategy(),
        sibling_cards in symbol_card_vec_strategy(0, 80),
        max_cards in 1_u32..=50,
        discovery_capped in any::<bool>(),
    ) {
        let remaining_capacity = max_cards.saturating_sub(1) as usize;
        let expected_frontier_ids = sibling_cards
            .iter()
            .skip(remaining_capacity)
            .map(|card| card.symbol.symbol_id.clone())
            .collect::<Vec<_>>();
        let (_cards, spillover) =
            apply_card_budget(entry_card, sibling_cards, max_cards, discovery_capped);
        let frontier_ids = spillover
            .frontier
            .iter()
            .map(|candidate| candidate.symbol_id.clone())
            .collect::<Vec<_>>();

        prop_assert_eq!(frontier_ids, expected_frontier_ids);
    }

    #[test]
    fn stable_card_order_sorts_same_cards_identically_on_repeated_runs(
        cards in symbol_card_vec_strategy(0, 80),
    ) {
        let mut first = cards.clone();
        let mut second = cards;

        first.sort_by(stable_card_order);
        second.sort_by(stable_card_order);

        prop_assert_eq!(ordered_symbol_ids(&first), ordered_symbol_ids(&second));
    }

    #[test]
    fn stable_card_order_sorts_permutations_to_same_order(
        cards in symbol_card_vec_strategy(0, 80),
    ) {
        prop_assume!(has_total_stable_order(&cards));
        let mut first = cards.clone();
        let mut second = cards;
        second.reverse();

        first.sort_by(stable_card_order);
        second.sort_by(stable_card_order);

        prop_assert_eq!(ordered_symbol_ids(&first), ordered_symbol_ids(&second));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn graph_slice_handler_never_returns_duplicate_symbol_ids(function_count in 2_usize..=20) {
        let (mut backends, temp_dir): (_, TempDir) =
            backends_fixture().map_err(TestCaseError::fail)?;
        let path = write_source(&temp_dir, "generated.rs", &rust_functions_source(function_count))
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let uri = Url::from_file_path(&path)
            .map_err(|()| TestCaseError::fail("file uri"))?
            .to_string();
        let request = make_request(&[
            "--uri",
            &uri,
            "--position",
            "1:4",
            "--max-cards",
            "64",
            "--entry-detail",
            detail_value(DetailLevel::Structure),
            "--node-detail",
            detail_value(DetailLevel::Structure),
        ]);

        let (status, payload) = dispatch_payload(&request, &mut backends)
            .map_err(TestCaseError::fail)?;

        prop_assert_eq!(status, 0, "expected success exit status");
        prop_assert_eq!(payload["status"].as_str(), Some("success"));
        prop_assert_eq!(payload["schema_version"].as_str(), Some("graph_slice.v1"));
        let cards = payload["cards"]
            .as_array()
            .ok_or_else(|| TestCaseError::fail("cards array"))?;
        let symbol_ids = cards
            .iter()
            .map(|card| {
                card["symbol"]["symbol_id"]
                    .as_str()
                    .ok_or_else(|| TestCaseError::fail("card should include symbol.symbol_id"))
            })
            .collect::<Result<HashSet<_>, _>>()?;

        prop_assert_eq!(symbol_ids.len(), cards.len());
    }

    #[test]
    fn discover_same_file_cards_never_returns_duplicate_symbol_ids(function_count in 2_usize..=20) {
        let (backends, temp_dir): (_, TempDir) =
            backends_fixture().map_err(TestCaseError::fail)?;
        let source = rust_functions_source(function_count);
        let path = write_source(&temp_dir, "direct.rs", &source)
            .map_err(|error| TestCaseError::fail(error.to_string()))?;
        let uri = Url::from_file_path(&path)
            .map_err(|()| TestCaseError::fail("file uri"))?
            .to_string();
        let request = GraphSliceRequest::parse(&[
            String::from("--uri"),
            uri,
            String::from("--position"),
            String::from("1:4"),
            String::from("--node-detail"),
            String::from("structure"),
        ])
        .map_err(|error| TestCaseError::fail(error.to_string()))?;

        let (cards, _discovery_capped) = discover_same_file_cards(
            &request,
            SliceDocument {
                path: &path,
                source: &source,
            },
            "",
            &backends,
        )?;
        let symbol_ids = cards
            .iter()
            .map(|card| card.symbol.symbol_id.as_str())
            .collect::<HashSet<_>>();

        prop_assert_eq!(symbol_ids.len(), cards.len());
    }
}
