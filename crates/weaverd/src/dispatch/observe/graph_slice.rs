//! Handler for `observe graph-slice` stable schema responses.
//! Full graph traversal is deferred to later roadmap items. For the schema milestone,
//! this handler returns a deterministic same-file slice bounded by `max_cards`.

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[path = "graph_slice/status.rs"]
mod status;

use url::Url;
use weaver_cards::{
    CardExtractionError,
    CardExtractionInput,
    DetailLevel,
    GraphSliceRequest,
    GraphSliceResponse,
    SliceSpillover,
    SymbolCard,
    TreeSitterCardExtractor,
    graph_slice::{SliceConstraints, SliceEntry, SliceRefusalReason, SpilloverCandidate},
};

use self::status::{GRAPH_SLICE_SCHEMA_VERSION, exit_status, refusal};
use super::enrich::{self, EnrichmentOutcome};
use crate::{
    backends::FusionBackends,
    dispatch::{
        errors::DispatchError,
        request::CommandRequest,
        response::ResponseWriter,
        router::DispatchResult,
    },
    semantic_provider::SemanticBackendProvider,
};

const MAX_SAME_FILE_DISCOVERY_POSITIONS: usize = 256;

fn display_filename(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<unknown>")
}

/// Handles the `observe graph-slice` command and serializes a deterministic same-file response.
///
/// # Errors
/// Returns a [`DispatchError`] if the request arguments are malformed
/// or the response cannot be serialized.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<DispatchResult, DispatchError> {
    let slice_request = GraphSliceRequest::parse(&request.arguments)
        .map_err(|error| DispatchError::invalid_arguments(error.to_string()))?;
    let parsed_uri = Url::parse(slice_request.uri())
        .map_err(|error| DispatchError::invalid_arguments(format!("invalid URI: {error}")))?;
    let path = resolve_file_path(&parsed_uri)?;
    let path = fs::canonicalize(&path).map_err(|error| {
        DispatchError::invalid_arguments(format!(
            "unable to read source file '{}': {error}",
            display_filename(&path)
        ))
    })?;
    let source = read_slice_source(&path)?;
    let response = build_response(&slice_request, &path, &source, backends)?;

    let status = exit_status(&response);
    let json = serde_json::to_string(&response)?;
    writer.write_stdout(json)?;
    Ok(DispatchResult::with_status(status))
}
/// Orchestrates the full graph-slice response for a validated `file://` URI.
///
/// **Execution order:** extracts the entry card, optionally enriches it via LSP, discovers
/// sibling symbols via [`discover_same_file_cards`], applies the card budget via
/// [`apply_card_budget`], enriches surviving sibling cards, and returns a
/// [`GraphSliceResponse::Success`].
///
/// **Error contract:** returns `Ok(GraphSliceResponse::Refusal { … })` for client-attributable
/// extraction errors; returns `Err(DispatchError)` only for unexpected internal failures.
fn build_response(
    request: &GraphSliceRequest,
    path: &Path,
    source: &str,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<GraphSliceResponse, DispatchError> {
    let extractor = backends.provider().card_extractor().clone();
    let mut entry_card = match extractor.extract(CardExtractionInput {
        path,
        source,
        line: request.line(),
        column: request.column(),
        detail: request.entry_detail(),
    }) {
        Ok(card) => card,
        Err(error) => return map_extraction_error(error),
    };
    enrich_card_if_requested(&mut entry_card, request.entry_detail(), source, backends);
    let entry_symbol_id = entry_card.symbol.symbol_id.clone();
    let (sibling_cards, discovery_capped) = discover_same_file_cards(
        request,
        SliceDocument { path, source },
        &entry_symbol_id,
        backends,
    )?;
    let (mut cards, spillover) = apply_card_budget(
        entry_card,
        sibling_cards,
        request.budget().max_cards(),
        discovery_capped,
    );
    // Enrich only the cards that survive budget truncation (entry card is already enriched).
    for card in cards.iter_mut().skip(1) {
        enrich_card_if_requested(card, request.node_detail(), source, backends);
    }

    Ok(GraphSliceResponse::Success {
        schema_version: String::from(GRAPH_SLICE_SCHEMA_VERSION),
        slice_version: 1,
        entry: SliceEntry {
            symbol_id: entry_symbol_id,
        },
        constraints: SliceConstraints {
            depth: request.depth(),
            direction: request.direction(),
            edge_types: request.edge_types().to_vec(),
            min_confidence: request.min_confidence(),
            budget: *request.budget(),
            entry_detail: request.entry_detail(),
            node_detail: request.node_detail(),
        },
        cards,
        edges: Vec::new(),
        spillover,
    })
}
/// Bundles the filesystem path and source text for same-file slice operations.
#[derive(Clone, Copy)]
struct SliceDocument<'a> {
    path: &'a Path,
    source: &'a str,
}
/// Extracts sibling `SymbolCard` values from the same source file, bounded by the discovery cap.
fn discover_same_file_cards(
    request: &GraphSliceRequest,
    document: SliceDocument<'_>,
    entry_symbol_id: &str,
    backends: &FusionBackends<SemanticBackendProvider>,
) -> Result<(Vec<SymbolCard>, bool), DispatchError> {
    let extractor = backends.provider().card_extractor().clone();
    let mut cards = BTreeMap::new();
    let (candidate_positions, discovery_capped) = candidate_positions(document.source);
    for (line, column) in candidate_positions {
        if (line, column) == (request.line(), request.column()) {
            continue;
        }
        let Some(card) =
            extract_same_file_card(&extractor, document, (line, column), request.node_detail())?
        else {
            continue;
        };
        if card.symbol.symbol_id == entry_symbol_id {
            continue;
        }
        cards.entry(card.symbol.symbol_id.clone()).or_insert(card);
    }
    let mut ordered_cards = cards.into_values().collect::<Vec<_>>();
    ordered_cards.sort_by(stable_card_order);
    Ok((ordered_cards, discovery_capped))
}

/// Attempts to extract one `SymbolCard` at `position`, returning `None` for benign misses.
fn extract_same_file_card(
    extractor: &TreeSitterCardExtractor,
    document: SliceDocument<'_>,
    position: (u32, u32),
    detail: DetailLevel,
) -> Result<Option<SymbolCard>, DispatchError> {
    let (line, column) = position;
    match extractor.extract(CardExtractionInput {
        path: document.path,
        source: document.source,
        line,
        column,
        detail,
    }) {
        Ok(card) => Ok(Some(card)),
        Err(CardExtractionError::NoSymbolAtPosition { .. })
        | Err(CardExtractionError::UnsupportedLanguage { .. }) => Ok(None),
        Err(error @ CardExtractionError::PositionOutOfRange { .. }) => {
            Err(DispatchError::internal(format!(
                "computed position {line}:{column} should be valid during same-file slice \
                 discovery: {error}"
            )))
        }
        Err(CardExtractionError::InvalidPath { path }) => Err(DispatchError::internal(format!(
            "Tree-sitter extractor requires an absolute path: {}",
            display_filename(&path)
        ))),
        Err(CardExtractionError::Parse { language, message }) => Err(DispatchError::internal(
            format!("Tree-sitter parse failed for {language}: {message}"),
        )),
    }
}
/// Yields `(line, column)` pairs for each non-blank line, capped at discovery limit.
fn candidate_positions(source: &str) -> (Vec<(u32, u32)>, bool) {
    let mut positions = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            first_non_whitespace_column(line).map(|column| ((index as u32) + 1, column))
        })
        // Bound extraction work until later symbol-table discovery milestones.
        .take(MAX_SAME_FILE_DISCOVERY_POSITIONS + 1)
        .collect::<Vec<_>>();
    let discovery_capped = positions.len() > MAX_SAME_FILE_DISCOVERY_POSITIONS;
    positions.truncate(MAX_SAME_FILE_DISCOVERY_POSITIONS);
    (positions, discovery_capped)
}
/// Returns the 1-based character column of the first non-whitespace character in `line`, or `None`
/// if the line is blank.
fn first_non_whitespace_column(line: &str) -> Option<u32> {
    line.chars()
        .position(|ch| !ch.is_whitespace())
        .map(|index| (index as u32) + 1)
}
/// Defines a deterministic total order over `SymbolCard` values for reproducible slice output.
fn stable_card_order(left: &SymbolCard, right: &SymbolCard) -> std::cmp::Ordering {
    let left_ref = &left.symbol.symbol_ref;
    let right_ref = &right.symbol.symbol_ref;
    (
        left_ref.container.as_deref().unwrap_or_default(),
        left_ref.name.as_str(),
        left_ref.kind,
        left_ref.range.start.line,
        left_ref.range.start.column,
        left_ref.range.end.line,
        left_ref.range.end.column,
    )
        .cmp(&(
            right_ref.container.as_deref().unwrap_or_default(),
            right_ref.name.as_str(),
            right_ref.kind,
            right_ref.range.start.line,
            right_ref.range.start.column,
            right_ref.range.end.line,
            right_ref.range.end.column,
        ))
}
/// Partitions cards into an included set and a spillover frontier according to `max_cards`.
fn apply_card_budget(
    entry_card: SymbolCard,
    sibling_cards: Vec<SymbolCard>,
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
        return (
            Vec::new(),
            SliceSpillover {
                truncated: true,
                frontier,
            },
        );
    }
    let remaining_capacity = max_cards.saturating_sub(1) as usize;
    let included_siblings = sibling_cards
        .iter()
        .take(remaining_capacity)
        .cloned()
        .collect::<Vec<_>>();
    let frontier = sibling_cards
        .iter()
        .skip(remaining_capacity)
        .map(|card| SpilloverCandidate {
            symbol_id: card.symbol.symbol_id.clone(),
            depth: 1,
        })
        .collect::<Vec<_>>();

    let mut cards = Vec::with_capacity(1 + included_siblings.len());
    cards.push(entry_card);
    cards.extend(included_siblings);
    let spillover = if frontier.is_empty() {
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
    (cards, spillover)
}
/// Applies LSP semantic enrichment to `card` when `detail` is at least `Semantic`.
///
/// Enrichment is **best-effort**: if the LSP backend is unavailable or returns
/// no hover information, the card is left unchanged and the function returns
/// normally. Callers must not rely on enrichment having occurred; use the
/// card's `provenance.sources` field to confirm which providers contributed.
///
/// On successful enrichment, `normalize_lsp_provenance` is called to remove
/// degraded tree-sitter entries and ensure `lsp_hover` appears in sources.
fn enrich_card_if_requested(
    card: &mut SymbolCard,
    detail: DetailLevel,
    source: &str,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) {
    if detail < DetailLevel::Semantic {
        return;
    }
    if enrich::try_lsp_enrichment(card, source, backends) == EnrichmentOutcome::Enriched {
        normalize_lsp_provenance(card);
    }
}
/// Replaces degraded Tree-sitter provenance entries with `lsp_hover` after successful enrichment.
fn normalize_lsp_provenance(card: &mut SymbolCard) {
    card.provenance
        .sources
        .retain(|source_name| source_name != "tree_sitter_degraded_semantic");
    if !card
        .provenance
        .sources
        .iter()
        .any(|source_name| source_name == "lsp_hover")
    {
        card.provenance.sources.push(String::from("lsp_hover"));
    }
}
/// Validates that `uri` uses the `file` scheme and converts it to a `PathBuf`.
fn resolve_file_path(uri: &Url) -> Result<PathBuf, DispatchError> {
    if uri.scheme() != "file" {
        return Err(DispatchError::invalid_arguments(format!(
            "unsupported URI scheme '{}': expected a file URI",
            uri.scheme()
        )));
    }
    uri.to_file_path()
        .map_err(|_| DispatchError::invalid_arguments("URI is not a valid file path"))
}
/// Reads the source file at `path`, mapping IO failures to invalid-arguments errors.
fn read_slice_source(path: &Path) -> Result<String, DispatchError> {
    fs::read_to_string(path).map_err(|error| {
        DispatchError::invalid_arguments(format!(
            "unable to read source file '{}': {error}",
            display_filename(path)
        ))
    })
}
/// Converts a `CardExtractionError` into either a structured `GraphSliceResponse::Refusal` or a
/// `DispatchError`.
fn map_extraction_error(error: CardExtractionError) -> Result<GraphSliceResponse, DispatchError> {
    match error {
        CardExtractionError::UnsupportedLanguage { path } => Ok(refusal(
            SliceRefusalReason::UnsupportedLanguage,
            format!(
                "observe graph-slice: unsupported language for '{}'",
                display_filename(&path)
            ),
        )),
        CardExtractionError::InvalidPath { path } => Err(DispatchError::internal(format!(
            "Tree-sitter extractor requires an absolute path: {}",
            display_filename(&path)
        ))),
        CardExtractionError::NoSymbolAtPosition { line, column } => Ok(refusal(
            SliceRefusalReason::NoSymbolAtPosition,
            format!("observe graph-slice: no symbol found at {line}:{column}"),
        )),
        CardExtractionError::PositionOutOfRange { line, column } => Ok(refusal(
            SliceRefusalReason::PositionOutOfRange,
            format!(
                "observe graph-slice: position {line}:{column} is outside the bounds of the file"
            ),
        )),
        CardExtractionError::Parse { language, message } => Err(DispatchError::internal(format!(
            "Tree-sitter parse failed for {language}: {message}"
        ))),
    }
}
#[cfg(test)]
#[path = "graph_slice_tests.rs"]
mod tests;
