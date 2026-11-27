//! Card-budget partitioning for `observe graph-slice`.
//!
//! The handler keeps the entry card and as many sibling cards as `max_cards`
//! allows, and reports every card it left out through the spillover frontier.
//! The frontier is what an operator inspects when a slice is missing symbols,
//! so the truncation event is emitted here, next to the partitioning that
//! decides which cards were dropped.

use weaver_cards::{SliceSpillover, SymbolCard, graph_slice::SpilloverCandidate};

use super::trace::log_budget_spillover;

/// Partitions cards into an included set and a spillover frontier according to `max_cards`.
///
/// The `max_cards == 0` branch is a defensive internal check: the public request
/// parser rejects `--max-cards 0` before dispatch, so a CLI caller cannot reach
/// it. It still reports every candidate as spillover rather than panicking,
/// because a future caller may construct the budget directly.
pub(super) fn apply_card_budget(
    entry_card: SymbolCard,
    sibling_cards: &[SymbolCard],
    max_cards: u32,
    discovery_capped: bool,
) -> (Vec<SymbolCard>, SliceSpillover) {
    if max_cards == 0 {
        let frontier = std::iter::once(SpilloverCandidate {
            symbol_id: entry_card.symbol.symbol_id.clone(),
            depth: 0,
        })
        .chain(sibling_cards.iter().map(|card| SpilloverCandidate {
            symbol_id: card.symbol.symbol_id.clone(),
            depth: 1,
        }))
        .collect();
        let spillover = SliceSpillover {
            truncated: true,
            frontier,
        };
        log_budget_spillover(0, spillover.frontier.len());
        return (Vec::new(), spillover);
    }

    let (included_count, _) = partition_counts(sibling_cards.len(), max_cards);
    let included_siblings = sibling_cards
        .iter()
        .take(included_count)
        .cloned()
        .collect::<Vec<_>>();
    let frontier = sibling_cards
        .iter()
        .skip(included_count)
        .map(|card| SpilloverCandidate {
            symbol_id: card.symbol.symbol_id.clone(),
            depth: 1,
        })
        .collect::<Vec<_>>();

    let mut cards = Vec::with_capacity(1 + included_siblings.len());
    cards.push(entry_card);
    cards.extend(included_siblings);
    let spillover = if frontier.is_empty() {
        // Discovery, not the card budget, may still have truncated the slice.
        SliceSpillover {
            truncated: discovery_capped,
            frontier,
        }
    } else {
        SliceSpillover {
            truncated: true,
            frontier,
        }
    };
    log_budget_spillover(cards.len(), spillover.frontier.len());
    (cards, spillover)
}

/// Splits `sibling_count` siblings into included and frontier counts.
///
/// The entry card always consumes one unit of `max_cards`, so siblings may take
/// at most `max_cards - 1` slots. The two returned counts always sum to
/// `sibling_count`, which the property tests in `graph_slice_properties.rs`
/// assert over arbitrary inputs.
///
/// `max_cards == 0` is handled before this helper is called, by the branch that
/// reports every candidate as spillover.
pub(super) fn partition_counts(sibling_count: usize, max_cards: u32) -> (usize, usize) {
    let capacity = max_cards.saturating_sub(1) as usize;
    let included = sibling_count.min(capacity);
    (included, sibling_count - included)
}
