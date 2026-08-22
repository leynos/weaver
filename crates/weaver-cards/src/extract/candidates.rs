//! Candidate selection and synthetic module-card helpers.

use std::path::Path;

use super::positions::usize_to_u32;
use crate::{CardSymbolKind, SourcePosition, SourceRange};

/// One symbol found in a parsed file, prior to being turned into a
/// [`crate::SymbolCard`]. Fields carry both the identity used for candidate
/// selection and the pre-computed pieces later assembled into card sections.
#[derive(Debug, Clone)]
pub(crate) struct EntityCandidate {
    /// Symbol kind (function, class, module, and so on).
    pub(crate) kind: CardSymbolKind,
    /// Symbol name as it appears in source.
    pub(crate) name: String,
    /// Name of the enclosing symbol, if any (e.g. the class containing a
    /// method), used to disambiguate identically named symbols.
    pub(crate) container: Option<String>,
    /// Byte range of the symbol in the source text, used for position-based
    /// candidate selection (narrowest enclosing range wins).
    pub(crate) byte_range: std::ops::Range<usize>,
    /// Line/column range of the symbol, reported in the resulting card.
    pub(crate) range: SourceRange,
    /// Rendered signature text, when the language extractor produced one.
    /// `None` for symbols without a signature, such as plain variables.
    pub(crate) signature_display: Option<String>,
    /// Parameters extracted from the symbol's signature, if any.
    pub(crate) params: Vec<crate::ParamInfo>,
    /// Rendered return type text; empty when the symbol has none.
    pub(crate) returns: String,
    /// Local variables discovered within the symbol's body.
    pub(crate) locals: Vec<crate::LocalInfo>,
    /// Branch points discovered within the symbol's body, used to derive the
    /// cyclomatic-complexity metric.
    pub(crate) branches: Vec<crate::BranchInfo>,
    /// Raw decorator tokens attached to the symbol, in source order.
    pub(crate) decorators: Vec<String>,
    /// Byte offset to scan backwards from when collecting leading doc
    /// comments and decorators. `None` when the language extractor found no
    /// suitable anchor point.
    pub(crate) attachment_anchor: Option<usize>,
    /// Docstring captured directly from the parse tree (e.g. a Python
    /// triple-quoted string), distinct from comment-based doc extraction.
    pub(crate) docstring: Option<String>,
    /// Number of source lines the symbol spans, used for the lines metric.
    pub(crate) lines: u32,
    /// Structural digest fed into the symbol-id fingerprint, distinguishing
    /// symbols that otherwise share name, container, and signature.
    pub(crate) structure_fingerprint: String,
    /// Import interstitial captured alongside this candidate, present only
    /// on the synthetic module candidate produced by [`build_module_candidate`].
    pub(crate) interstitial: Option<InterstitialCandidate>,
}

/// The leading import block of a module, captured so a request positioned
/// within it can still resolve to a card describing the imports.
#[derive(Debug, Clone)]
pub(crate) struct InterstitialCandidate {
    /// Byte range spanned by the import block.
    pub(crate) byte_range: std::ops::Range<usize>,
    /// Raw, unmodified import block text.
    pub(crate) raw: String,
    /// Normalised, one-entry-per-line rendering of the imports.
    pub(crate) normalized: Vec<String>,
    /// Imports grouped by the language extractor's grouping rules (e.g.
    /// std/external/local), each inner vector being one group's entries.
    pub(crate) groups: Vec<Vec<String>>,
}

/// Builds a synthetic whole-file candidate representing the module itself,
/// used as a fallback when no narrower symbol covers the requested position
/// (for example, a request landing in the import block or blank lines
/// between symbols). Returns `None` for empty source, since there is nothing
/// meaningful to describe.
pub(super) fn build_module_candidate(
    path: &Path,
    source: &str,
    interstitial: Option<InterstitialCandidate>,
) -> Option<EntityCandidate> {
    if source.is_empty() {
        return None;
    }

    let line_count = usize_to_u32(source.lines().count());
    let end_column = source
        .lines()
        .last()
        .map_or(0, |line| usize_to_u32(line.len()));
    Some(EntityCandidate {
        kind: CardSymbolKind::Module,
        name: module_name(path),
        container: None,
        byte_range: 0..source.len(),
        range: SourceRange {
            start: SourcePosition { line: 0, column: 0 },
            end: SourcePosition {
                line: line_count.saturating_sub(1),
                column: end_column,
            },
        },
        signature_display: None,
        params: Vec::new(),
        returns: String::new(),
        locals: Vec::new(),
        branches: Vec::new(),
        decorators: Vec::new(),
        attachment_anchor: Some(0),
        docstring: None,
        lines: line_count.max(1),
        structure_fingerprint: String::from("module"),
        interstitial,
    })
}

/// Picks the candidate that best matches `byte`, preferring the smallest
/// enclosing entity so that a nested symbol (e.g. a method inside a class)
/// wins over its container. Falls back to the module candidate's import
/// interstitial when `byte` falls within it and no entity matches, and
/// returns `None` when nothing covers the position at all.
pub(super) fn select_candidate<'a>(
    entities: &'a [EntityCandidate],
    module_candidate: Option<&'a EntityCandidate>,
    byte: usize,
) -> Option<&'a EntityCandidate> {
    let entity = entities
        .iter()
        .filter(|candidate| contains_byte(candidate, byte))
        .min_by_key(|candidate| {
            candidate
                .byte_range
                .end
                .saturating_sub(candidate.byte_range.start)
        });
    if entity.is_some() {
        return entity;
    }

    module_candidate.and_then(|candidate| {
        candidate
            .interstitial
            .as_ref()
            .filter(|interstitial| {
                byte >= interstitial.byte_range.start && byte < interstitial.byte_range.end
            })
            .map(|_| candidate)
    })
}

/// Reports whether `byte` falls within `candidate`'s half-open byte range.
const fn contains_byte(candidate: &EntityCandidate, byte: usize) -> bool {
    byte >= candidate.byte_range.start && byte < candidate.byte_range.end
}

/// Derives the module candidate's display name from `path`'s file stem,
/// falling back to the full file name, then to `"module"` when neither is
/// valid UTF-8 (e.g. an unusual path with no name component at all).
fn module_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(String::from)
        .or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(String::from)
        })
        .unwrap_or_else(|| String::from("module"))
}
