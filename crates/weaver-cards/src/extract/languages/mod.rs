//! Language-specific entity and interstitial extraction rules.

mod common;
mod python;
mod rust;
mod typescript;

use tree_sitter::Node;
use weaver_syntax::SupportedLanguage;

use super::{EntityCandidate, InterstitialCandidate};

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

pub(super) fn collect_import_interstitial(
    language: SupportedLanguage,
    root: Node<'_>,
    source: &str,
) -> Option<InterstitialCandidate> {
    let imports = common::top_level_imports(language, root, source);
    if imports.is_empty() {
        return None;
    }

    let start = imports.first().map(|block| block.byte_start)?;
    let end = imports.last().map(|block| block.byte_end)?;
    let raw = source.get(start..end).unwrap_or_default().to_owned();
    let mut normalized = Vec::new();
    let mut groups = Vec::new();
    for block in &imports {
        normalized.extend(block.normalized.clone());
        groups.push(block.normalized.clone());
    }

    Some(InterstitialCandidate {
        byte_range: start..end,
        raw,
        normalized,
        groups,
    })
}
