//! Language-specific entity and interstitial extraction rules.

mod common;
mod python;
mod rust;
mod typescript;

use tree_sitter::Node;
use weaver_syntax::SupportedLanguage;

use super::{EntityCandidate, ImportBlock, InterstitialCandidate};

/// Dispatches entity extraction to the language-specific collector for
/// `language`, so callers need not match on [`SupportedLanguage`] themselves.
pub(super) fn collect_entities(
    language: SupportedLanguage,
    root: Node<'_>,
    source: &str,
) -> Vec<EntityCandidate> {
    match language {
        SupportedLanguage::Rust => rust::collect(root, source),
        SupportedLanguage::Python => python::collect(root, source),
        SupportedLanguage::TypeScript => typescript::collect(root, source),
    }
}

/// Builds an [`InterstitialCandidate`] spanning the first contiguous run of
/// top-level import blocks, or `None` when the file has no imports. Only the
/// leading run is kept: a later, separated import group would otherwise widen
/// the span across intervening code that is not itself import material.
pub(super) fn collect_import_interstitial(
    language: SupportedLanguage,
    root: Node<'_>,
    source: &str,
) -> Option<InterstitialCandidate> {
    let imports = common::top_level_imports(language, root, source);
    let merged_group = leading_import_run(&imports);
    let start = merged_group.first().map(|block| block.byte_start)?;
    let end = merged_group.last().map(|block| block.byte_end)?;
    let raw = source.get(start..end).unwrap_or_default().to_owned();
    let (normalized, groups) = flatten_import_groups(merged_group);

    Some(InterstitialCandidate {
        byte_range: start..end,
        raw,
        normalized,
        groups,
    })
}

/// Keeps the leading run of import blocks that touch or overlap each other.
///
/// Splitting here is what stops a later import group from widening the
/// interstitial span across the code between them.
fn leading_import_run(imports: &[ImportBlock]) -> &[ImportBlock] {
    let run_length = imports
        .iter()
        .zip(imports.iter().skip(1))
        .position(|(previous, next)| !blocks_touch(previous, next))
        .map_or(imports.len(), |index| index + 1);

    imports.get(..run_length).unwrap_or(imports)
}

/// Reports whether `next` starts at most one byte past the end of `previous`.
const fn blocks_touch(previous: &ImportBlock, next: &ImportBlock) -> bool {
    next.byte_start <= previous.byte_end + 1
}

/// Concatenates the run's normalised lines, keeping the per-block grouping.
fn flatten_import_groups(blocks: &[ImportBlock]) -> (Vec<String>, Vec<Vec<String>>) {
    let groups: Vec<Vec<String>> = blocks
        .iter()
        .map(|block| block.normalized.clone())
        .collect();
    let normalized = groups.iter().flatten().cloned().collect();

    (normalized, groups)
}
