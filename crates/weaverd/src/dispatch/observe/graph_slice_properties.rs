//! Property tests for the `observe graph-slice` invariants this change relies on.
//!
//! Three behaviours are documented as holding for *any* input rather than for a
//! handful of fixtures:
//!
//! - the non-blank line count the discovery-cap event reports must agree with the positions
//!   `candidate_positions` actually considers,
//! - the budget partition must split candidates into included and frontier cards without losing or
//!   duplicating any, and
//! - `truncated` must report both causes: an excluded candidate or a capped discovery.
//!
//! Fixed-example tests cannot establish any of these, so they use `proptest`.

use proptest::prelude::*;
use weaver_cards::{SourcePosition, SymbolCard, graph_slice::SliceSpillover};

use super::{
    super::{
        MAX_SAME_FILE_DISCOVERY_POSITIONS,
        budget::apply_card_budget,
        candidate_positions,
        trace::line_counts,
    },
    support::render_padded_functions,
};
use crate::dispatch::observe::enrich::enrich_test_utils::test_symbol_card_with_pos;

/// Builds source text from a sequence of line kinds.
///
/// Lines are either non-blank (a distinct function declaration) or blank, which
/// lets a strategy produce inputs where the total line count and the non-blank
/// count diverge.
fn render_source(lines: &[bool]) -> String {
    lines
        .iter()
        .enumerate()
        .fold(String::new(), |mut source, (index, is_non_blank)| {
            if *is_non_blank {
                source.push_str("fn item_");
                source.push_str(&index.to_string());
                source.push_str("() {}\n");
            } else {
                source.push('\n');
            }
            source
        })
}

/// Builds `count` cards with distinct symbol IDs.
///
/// The prefix keeps the entry card's ID distinct from every sibling's; sharing
/// an ID across the two sets would make the disjointness assertion meaningless.
fn cards(prefix: &str, count: usize) -> Vec<SymbolCard> {
    (0..count)
        .map(|index| {
            let mut card = test_symbol_card_with_pos(
                SourcePosition { line: 1, column: 0 },
                SourcePosition { line: 1, column: 1 },
            );
            card.symbol.symbol_id = format!("{prefix}_{index}");
            card
        })
        .collect()
}

/// Returns the symbol IDs recorded in `spillover`.
fn frontier_ids(spillover: &SliceSpillover) -> Vec<String> {
    spillover
        .frontier
        .iter()
        .map(|candidate| candidate.symbol_id.clone())
        .collect()
}

/// A blank/non-blank layout that reaches below, at, and above the discovery cap.
///
/// Sampling each line independently would almost never produce more than 256
/// non-blank lines, leaving the capped branch effectively untested. Choosing the
/// non-blank count explicitly puts the interesting sizes on the same footing as
/// small ones, and a blank inserted every `stride` lines keeps the fixture
/// interleaved rather than clustering every blank at the end.
fn line_layout() -> impl Strategy<Value = Vec<bool>> {
    (0..=(MAX_SAME_FILE_DISCOVERY_POSITIONS + 2), 1usize..=8usize).prop_map(interleave_blanks)
}

/// Places a blank line after every `stride` non-blank lines.
///
/// Interleaving keeps every generated layout's total line count different from
/// its non-blank count, so the two fields cannot be confused by accident.
fn interleave_blanks((non_blank, stride): (usize, usize)) -> Vec<bool> {
    // Capacity is only a hint; over-allocating avoids integer division here.
    let mut lines = Vec::with_capacity(non_blank.saturating_add(stride));
    let mut placed = 0;
    while placed < non_blank {
        let chunk = stride.min(non_blank - placed);
        lines.extend(std::iter::repeat_n(true, chunk));
        placed += chunk;
        lines.push(false);
    }
    lines
}

/// The cap must engage exactly above `MAX_SAME_FILE_DISCOVERY_POSITIONS`.
///
/// The property strategy can reach the boundary, but only as one of hundreds of
/// possible counts, so an off-by-one at the cap could escape a run. These cases
/// are deterministic and interleave blank lines, which also distinguishes the
/// non-blank count from the file's total line count.
#[rstest::rstest]
#[case::below_cap(MAX_SAME_FILE_DISCOVERY_POSITIONS - 1, false)]
#[case::at_cap(MAX_SAME_FILE_DISCOVERY_POSITIONS, false)]
#[case::above_cap(MAX_SAME_FILE_DISCOVERY_POSITIONS + 1, true)]
fn capping_begins_only_above_the_cap(#[case] non_blank: usize, #[case] expected_capped: bool) {
    let blanks_between = 2;
    let source = render_padded_functions(non_blank, blanks_between);

    let (positions, capped) = candidate_positions(&source);

    assert_eq!(line_counts(&source).1, non_blank, "non-blank count");
    assert_ne!(
        source.lines().count(),
        non_blank,
        "fixture must distinguish total from non-blank lines"
    );
    assert_eq!(
        positions.len(),
        non_blank.min(MAX_SAME_FILE_DISCOVERY_POSITIONS),
        "retained positions"
    );
    assert_eq!(capped, expected_capped, "capping flag");
}

proptest! {
    /// The reported non-blank count equals the number of positions discovery
    /// would probe, bounded by the cap, over arbitrary blank/non-blank layouts.
    #[test]
    fn non_blank_count_agrees_with_candidate_positions(lines in line_layout()) {
        let source = render_source(&lines);
        let expected_non_blank = lines.iter().filter(|is_non_blank| **is_non_blank).count();
        prop_assert_eq!(line_counts(&source).1, expected_non_blank);

        let (positions, capped) = candidate_positions(&source);
        prop_assert_eq!(positions.len(), expected_non_blank.min(MAX_SAME_FILE_DISCOVERY_POSITIONS));
        prop_assert_eq!(capped, expected_non_blank > MAX_SAME_FILE_DISCOVERY_POSITIONS);
    }

    /// `apply_card_budget` conserves every candidate and respects the budget,
    /// for positive budgets and both discovery outcomes.
    #[test]
    fn budget_partition_conserves_cards(
        sibling_count in 0usize..600,
        max_cards in 1u32..=400,
        discovery_capped: bool,
    ) {
        let mut entry_cards = cards("entry", 1);
        let entry_card = entry_cards.pop().expect("one entry card");
        let siblings = cards("sib", sibling_count);

        let (included, spillover) =
            apply_card_budget(entry_card, &siblings, max_cards, discovery_capped);

        // No candidate is lost or counted twice.
        prop_assert_eq!(included.len() + spillover.frontier.len(), 1 + sibling_count);
        // The entry card costs one slot, so the whole response fits the budget.
        prop_assert!(included.len() <= max_cards as usize);
        // The entry card is always kept and comes first.
        prop_assert_eq!(included.first().map(|card| card.symbol.symbol_id.as_str()), Some("entry_0"));
        // Included and frontier symbol IDs are disjoint.
        for candidate in frontier_ids(&spillover) {
            prop_assert!(!included.iter().any(|card| card.symbol.symbol_id == candidate));
        }
        // The frontier is non-empty exactly when capacity was exceeded.
        prop_assert_eq!(
            !spillover.frontier.is_empty(),
            sibling_count > (max_cards as usize).saturating_sub(1),
        );
        // `truncated` reports either cause.
        prop_assert_eq!(
            spillover.truncated,
            !spillover.frontier.is_empty() || discovery_capped,
        );
    }

    /// The defensive zero-budget branch reports every candidate as spillover.
    #[test]
    fn zero_budget_reports_every_candidate(sibling_count in 0usize..600) {
        let mut entry_cards = cards("entry", 1);
        let entry_card = entry_cards.pop().expect("one entry card");
        let siblings = cards("sib", sibling_count);

        let (included, spillover) = apply_card_budget(entry_card, &siblings, 0, false);

        prop_assert!(included.is_empty());
        prop_assert_eq!(spillover.frontier.len(), 1 + sibling_count);
        prop_assert!(spillover.truncated);
        // The entry candidate is reported at depth 0, siblings at depth 1.
        prop_assert_eq!(spillover.frontier.first().map(|candidate| candidate.depth), Some(0));
        prop_assert!(spillover.frontier.iter().skip(1).all(|candidate| candidate.depth == 1));
    }
}
