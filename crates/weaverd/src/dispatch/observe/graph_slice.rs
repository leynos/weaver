//! Handler for the `observe graph-slice` operation.
//!
//! This module parses graph-slice requests through the stable schema
//! types in `weaver-cards` and produces schema-valid JSON responses.
//! The full graph traversal engine is deferred to roadmap items 7.2.2
//! through 7.2.5. For the schema milestone, the handler returns a
//! deterministic same-file slice: the entry card plus additional cards
//! discovered from the same file, bounded by `max_cards`, with
//! spillover metadata when extra local symbols do not fit.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use url::Url;
use weaver_cards::graph_slice::{
    SliceConstraints, SliceEntry, SliceRefusal, SliceRefusalReason, SpilloverCandidate,
};
use weaver_cards::{
    CardExtractionError, CardExtractionInput, DetailLevel, GraphSliceRequest, GraphSliceResponse,
    SliceSpillover, SymbolCard, TreeSitterCardExtractor,
};

use crate::backends::FusionBackends;
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::DispatchResult;
use crate::semantic_provider::SemanticBackendProvider;

use super::enrich::{self, EnrichmentOutcome};

const MAX_SAME_FILE_DISCOVERY_POSITIONS: usize = 256;

/// Maps a graph-slice response to its exit status code.
///
/// Returns `0` for success, `1` for refusals.
fn exit_status(response: &GraphSliceResponse) -> i32 {
    match response {
        GraphSliceResponse::Success { .. } => 0,
        GraphSliceResponse::Refusal { .. } => 1,
        _ => 1,
    }
}

/// Handles the `observe graph-slice` command.
///
/// Parses the request through [`GraphSliceRequest`] and serializes a
/// typed response. The schema milestone returns a deterministic
/// same-file slice while later roadmap items add true graph traversal
/// and edge extraction.
///
/// # Errors
///
/// Returns a [`DispatchError`] if the request arguments are malformed
/// or the response cannot be serialized.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<DispatchResult, DispatchError> {
    let slice_request = GraphSliceRequest::parse(&request.arguments)
        .map_err(|error| DispatchError::invalid_arguments(error.to_string()))?;
    let parsed_uri = Url::parse(slice_request.uri()).map_err(|error| {
        DispatchError::invalid_arguments(format!("invalid URI '{}': {error}", slice_request.uri()))
    })?;
    let path = resolve_file_path(&parsed_uri)?;
    let source = read_slice_source(&path)?;
    let response = build_response(&slice_request, &path, &source, backends)?;

    let status = exit_status(&response);
    let json = serde_json::to_string(&response)?;
    writer.write_stdout(json)?;

    Ok(DispatchResult::with_status(status))
}

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
    let sibling_cards = discover_same_file_cards(
        request,
        SliceDocument { path, source },
        &entry_symbol_id,
        backends,
    )?;
    let (cards, spillover) =
        apply_card_budget(entry_card, sibling_cards, request.budget().max_cards());

    Ok(GraphSliceResponse::Success {
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

#[derive(Clone, Copy)]
struct SliceDocument<'a> {
    path: &'a Path,
    source: &'a str,
}

fn discover_same_file_cards(
    request: &GraphSliceRequest,
    document: SliceDocument<'_>,
    entry_symbol_id: &str,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<Vec<SymbolCard>, DispatchError> {
    let extractor = backends.provider().card_extractor().clone();
    let mut cards = BTreeMap::new();

    for (line, column) in candidate_positions(document.source) {
        if (line, column) == (request.line(), request.column()) {
            continue;
        }

        let Some(mut card) =
            extract_same_file_card(&extractor, document, (line, column), request.node_detail())?
        else {
            continue;
        };

        if card.symbol.symbol_id == entry_symbol_id {
            continue;
        }

        enrich_card_if_requested(&mut card, request.node_detail(), document.source, backends);
        cards.entry(card.symbol.symbol_id.clone()).or_insert(card);
    }

    let mut ordered_cards = cards.into_values().collect::<Vec<_>>();
    ordered_cards.sort_by(stable_card_order);
    Ok(ordered_cards)
}

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
                "computed position {line}:{column} should be valid during same-file slice discovery: {error}"
            )))
        }
        Err(CardExtractionError::InvalidPath { path }) => Err(DispatchError::internal(format!(
            "Tree-sitter extractor requires an absolute path: {}",
            path.display()
        ))),
        Err(CardExtractionError::Parse { language, message }) => Err(DispatchError::internal(
            format!("Tree-sitter parse failed for {language}: {message}"),
        )),
    }
}

fn candidate_positions(source: &str) -> Vec<(u32, u32)> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            first_non_whitespace_column(line).map(|column| ((index as u32) + 1, column))
        })
        // Bound extraction work for large files until symbol-table discovery
        // lands in the later graph-slice milestones.
        .take(MAX_SAME_FILE_DISCOVERY_POSITIONS)
        .collect()
}

fn first_non_whitespace_column(line: &str) -> Option<u32> {
    line.chars()
        .position(|ch| !ch.is_whitespace())
        .map(|index| (index as u32) + 1)
}

fn stable_card_order(left: &SymbolCard, right: &SymbolCard) -> std::cmp::Ordering {
    let left_ref = &left.symbol.symbol_ref;
    let right_ref = &right.symbol.symbol_ref;
    (
        left_ref.container.as_deref().unwrap_or_default(),
        left_ref.name.as_str(),
        format!("{:?}", left_ref.kind),
        left_ref.range.start.line,
        left_ref.range.start.column,
        left_ref.range.end.line,
        left_ref.range.end.column,
    )
        .cmp(&(
            right_ref.container.as_deref().unwrap_or_default(),
            right_ref.name.as_str(),
            format!("{:?}", right_ref.kind),
            right_ref.range.start.line,
            right_ref.range.start.column,
            right_ref.range.end.line,
            right_ref.range.end.column,
        ))
}

fn apply_card_budget(
    entry_card: SymbolCard,
    sibling_cards: Vec<SymbolCard>,
    max_cards: u32,
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
        SliceSpillover::empty()
    } else {
        SliceSpillover {
            truncated: true,
            frontier,
        }
    };

    (cards, spillover)
}

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

fn resolve_file_path(uri: &Url) -> Result<PathBuf, DispatchError> {
    if uri.scheme() != "file" {
        return Err(DispatchError::invalid_arguments(format!(
            "unsupported URI scheme '{}': expected file",
            uri.scheme()
        )));
    }

    uri.to_file_path().map_err(|_| {
        DispatchError::invalid_arguments(format!("URI is not a valid file path: {uri}"))
    })
}

fn read_slice_source(path: &Path) -> Result<String, DispatchError> {
    fs::read_to_string(path).map_err(|error| {
        DispatchError::invalid_arguments(format!(
            "unable to read source file '{}': {error}",
            path.display()
        ))
    })
}

fn map_extraction_error(error: CardExtractionError) -> Result<GraphSliceResponse, DispatchError> {
    match error {
        CardExtractionError::UnsupportedLanguage { path } => Ok(GraphSliceResponse::Refusal {
            refusal: SliceRefusal {
                reason: SliceRefusalReason::UnsupportedLanguage,
                message: format!(
                    "observe graph-slice: unsupported language for path {}",
                    path.display()
                ),
            },
        }),
        CardExtractionError::InvalidPath { path } => Err(DispatchError::internal(format!(
            "Tree-sitter extractor requires an absolute path: {}",
            path.display()
        ))),
        CardExtractionError::NoSymbolAtPosition { line, column } => {
            Ok(GraphSliceResponse::Refusal {
                refusal: SliceRefusal {
                    reason: SliceRefusalReason::NoSymbolAtPosition,
                    message: format!("observe graph-slice: no symbol found at {line}:{column}"),
                },
            })
        }
        CardExtractionError::PositionOutOfRange { line, column } => {
            Ok(GraphSliceResponse::Refusal {
                refusal: SliceRefusal {
                    reason: SliceRefusalReason::PositionOutOfRange,
                    message: format!(
                        "observe graph-slice: position {line}:{column} is outside the bounds of the file"
                    ),
                },
            })
        }
        CardExtractionError::Parse { language, message } => Err(DispatchError::internal(format!(
            "Tree-sitter parse failed for {language}: {message}"
        ))),
    }
}

#[cfg(test)]
#[path = "graph_slice_tests.rs"]
mod tests;
