//! Candidate selection and synthetic module-card helpers.

use std::path::Path;

use crate::{CardSymbolKind, SourcePosition, SourceRange};

use super::positions::usize_to_u32;

#[derive(Debug, Clone)]
pub(crate) struct EntityCandidate {
    pub(crate) kind: CardSymbolKind,
    pub(crate) name: String,
    pub(crate) container: Option<String>,
    pub(crate) byte_range: std::ops::Range<usize>,
    pub(crate) range: SourceRange,
    pub(crate) signature_display: Option<String>,
    pub(crate) params: Vec<crate::ParamInfo>,
    pub(crate) returns: String,
    pub(crate) locals: Vec<crate::LocalInfo>,
    pub(crate) branches: Vec<crate::BranchInfo>,
    pub(crate) decorators: Vec<String>,
    pub(crate) attachment_anchor: Option<usize>,
    pub(crate) docstring: Option<String>,
    pub(crate) lines: u32,
    pub(crate) structure_fingerprint: String,
    pub(crate) interstitial: Option<InterstitialCandidate>,
}

#[derive(Debug, Clone)]
pub(crate) struct InterstitialCandidate {
    pub(crate) byte_range: std::ops::Range<usize>,
    pub(crate) raw: String,
    pub(crate) normalized: Vec<String>,
    pub(crate) groups: Vec<Vec<String>>,
}

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

const fn contains_byte(candidate: &EntityCandidate, byte: usize) -> bool {
    byte >= candidate.byte_range.start && byte < candidate.byte_range.end
}

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
