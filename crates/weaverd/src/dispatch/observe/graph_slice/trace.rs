//! Tracing events for the `observe graph-slice` decision boundaries.
//!
//! Instrumentation is colocated with the handler it explains so the fields for
//! one boundary are defined in exactly one place. Each event targets
//! [`DISPATCH_TARGET`] and carries the identifiers an operator needs to
//! correlate a slice with its request; none of them log source text.
//!
//! **Scope and reuse policy:** these helpers serve `graph_slice` and its
//! submodules only. They are not a general logging facade: a sibling handler
//! that needs dispatch events emits them inline, as `get_definition` and
//! `enrich` do. Add a helper here only when two or more decision points in this
//! handler share the same event shape.

use std::path::Path;

use tracing::debug;
use weaver_cards::GraphSliceRequest;

use super::{MAX_SAME_FILE_DISCOVERY_POSITIONS, first_non_whitespace_column};
use crate::dispatch::{observe::enrich::EnrichmentOutcome, router::DISPATCH_TARGET};

/// Emits the handler-entry event for a validated `graph-slice` request.
///
/// This is the only event that records the full request shape, so a slice can
/// be reconstructed from logs without the original JSONL request. Detail levels
/// are formatted with `Debug` because their variant names identify the
/// requested depth more precisely than their serialized form, while integers
/// are recorded without sigils so log consumers can index them numerically.
pub(super) fn log_request_received(request: &GraphSliceRequest) {
    debug!(
        target: DISPATCH_TARGET,
        uri = %request.uri(),
        line = request.line(),
        column = request.column(),
        entry_detail = ?request.entry_detail(),
        node_detail = ?request.node_detail(),
        max_cards = request.budget().max_cards(),
        "handling graph-slice request"
    );
}

/// Emits the discovery-cap event, including how much input the cap discarded.
///
/// Without this event a capped file is indistinguishable from a file that
/// genuinely holds only the cards returned. Both counts are recorded because
/// they answer different questions: `total_lines` is the size of the file the
/// issue asks for, while `total_non_blank_lines` is the number of positions
/// discovery actually considered before the cap. A file padded with blank
/// lines has a large `total_lines` but few candidates.
///
/// The counts are computed here rather than by the caller so the cost is paid
/// only when the cap actually triggered, and both are gathered in one traversal
/// so a capped file is never scanned twice just to describe it.
pub(super) fn log_discovery_capped(path: &Path, source: &str) {
    let (total_lines, total_non_blank_lines) = line_counts(source);
    debug!(
        target: DISPATCH_TARGET,
        path = %path.display(),
        total_lines,
        total_non_blank_lines,
        cap = MAX_SAME_FILE_DISCOVERY_POSITIONS,
        "same-file discovery capped at maximum positions"
    );
}

/// Emits the budget-truncation event when the spillover frontier is non-empty.
///
/// A budget that fits every card is the common case, so logging it would bury
/// the truncations that explain missing cards. `frontier_candidates` is the
/// number of candidates the response left out.
pub(super) fn log_budget_spillover(included_cards: usize, frontier_candidates: usize) {
    if frontier_candidates == 0 {
        return;
    }
    debug!(
        target: DISPATCH_TARGET,
        included_cards, frontier_candidates, "budget truncation produced spillover frontier"
    );
}

/// Emits the event recorded when enrichment is skipped for a card.
///
/// The literal `"skipped"` outcome keeps this event distinguishable from
/// [`log_enrichment_completed`] by a single field, so a log query can count
/// attempted and skipped cards together.
pub(super) fn log_enrichment_skipped(symbol_id: &str) {
    debug!(
        target: DISPATCH_TARGET,
        symbol_id, outcome = "skipped", "enrichment skipped due to detail level"
    );
}

/// Emits the event recorded after an enrichment attempt completes.
///
/// Both [`EnrichmentOutcome::Enriched`] and [`EnrichmentOutcome::Degraded`]
/// reach this event, so a degraded backend stays visible without an error.
pub(super) fn log_enrichment_completed(symbol_id: &str, outcome: EnrichmentOutcome) {
    debug!(
        target: DISPATCH_TARGET,
        symbol_id, outcome = ?outcome, "card enrichment completed"
    );
}

/// Returns the file's total and non-blank line counts in a single pass.
///
/// The non-blank count is the number of positions same-file discovery would
/// probe before its cap: blankness is decided by [`first_non_whitespace_column`],
/// the same predicate `candidate_positions` uses. A property test in
/// `graph_slice_properties.rs` asserts that agreement.
///
/// Counting both together matters on the capped path: a large file would
/// otherwise be traversed once per field while debug logging is enabled.
pub(super) fn line_counts(source: &str) -> (usize, usize) {
    source.lines().fold((0, 0), |(total, non_blank), line| {
        let is_non_blank = usize::from(first_non_whitespace_column(line).is_some());
        (total + 1, non_blank + is_non_blank)
    })
}
