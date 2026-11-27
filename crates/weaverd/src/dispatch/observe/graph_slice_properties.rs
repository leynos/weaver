//! Property tests for the `observe graph-slice` invariants this change relies on.
//!
//! Two behaviours are documented as holding for *any* input rather than for a
//! handful of fixtures:
//!
//! - the non-blank line count the discovery-cap event reports must agree with the positions
//!   `candidate_positions` actually considers, and
//! - the budget partition must split candidates into included and frontier cards without losing or
//!   duplicating any.
//!
//! Fixed-example tests cannot establish either, so these use `proptest`.

use proptest::prelude::*;

use super::super::{
    MAX_SAME_FILE_DISCOVERY_POSITIONS,
    budget::partition_counts,
    candidate_positions,
    trace::count_non_blank_lines,
};

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

proptest! {
    /// The reported non-blank count equals the number of positions discovery
    /// would probe, bounded by the cap, over arbitrary blank/non-blank layouts.
    #[test]
    fn non_blank_count_agrees_with_candidate_positions(
        // Long enough to straddle the cap, short enough to stay fast.
        lines in prop::collection::vec(any::<bool>(), 0..(MAX_SAME_FILE_DISCOVERY_POSITIONS + 40)),
    ) {
        let source = render_source(&lines);
        let expected_non_blank = lines.iter().filter(|is_non_blank| **is_non_blank).count();
        prop_assert_eq!(count_non_blank_lines(&source), expected_non_blank);

        let (positions, capped) = candidate_positions(&source);
        prop_assert_eq!(positions.len(), expected_non_blank.min(MAX_SAME_FILE_DISCOVERY_POSITIONS));
        prop_assert_eq!(capped, expected_non_blank > MAX_SAME_FILE_DISCOVERY_POSITIONS);
    }

    /// Included and frontier counts always partition the candidate set, and the
    /// entry card never pushes included cards past the requested budget.
    #[test]
    fn budget_partition_is_total_and_within_capacity(
        sibling_count in 0usize..600,
        max_cards in 1u32..=400,
    ) {
        let (included, frontier) = partition_counts(sibling_count, max_cards);

        // No candidate is lost or counted twice.
        prop_assert_eq!(included + frontier, sibling_count);
        // The entry card costs one slot.
        prop_assert!(included <= (max_cards as usize).saturating_sub(1));
        // The frontier is non-empty exactly when capacity was exceeded.
        prop_assert_eq!(frontier > 0, sibling_count > (max_cards as usize).saturating_sub(1));
    }
}
