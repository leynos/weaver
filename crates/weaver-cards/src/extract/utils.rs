//! URI, provenance, and language-mapping helpers for card assembly.

use std::{path::Path, sync::OnceLock};

use url::Url;
use weaver_syntax::SupportedLanguage;

use crate::{CardExtractionError, CardLanguage, DetailLevel};

/// Lists the extraction sources that contributed to a card at `detail`.
///
/// Tree-sitter extraction always contributes; the higher detail levels are
/// reported as "degraded" because this extractor never runs LSP-backed
/// semantic or full analysis, it only fills those levels from syntax alone.
pub(super) fn provenance_sources(detail: DetailLevel) -> Vec<String> {
    static TREE_SITTER_ONLY: OnceLock<Vec<String>> = OnceLock::new();
    let base = TREE_SITTER_ONLY.get_or_init(|| vec![String::from("tree_sitter")]);
    let mut sources = base.clone();
    if detail >= DetailLevel::Semantic {
        sources.push(String::from("tree_sitter_degraded_semantic"));
    }
    if detail >= DetailLevel::Full {
        sources.push(String::from("tree_sitter_degraded_full"));
    }
    sources
}

/// Maps the internal Tree-sitter language enum onto the public card schema's
/// language enum, keeping the two vocabularies independently evolvable.
pub(super) const fn to_card_language(language: SupportedLanguage) -> CardLanguage {
    match language {
        SupportedLanguage::Rust => CardLanguage::Rust,
        SupportedLanguage::Python => CardLanguage::Python,
        SupportedLanguage::TypeScript => CardLanguage::TypeScript,
    }
}

/// Builds the `file://` URI used to identify a symbol's source location.
///
/// # Errors
///
/// Returns [`CardExtractionError::InvalidPath`] when `path` is not absolute
/// or cannot otherwise be converted to a file URI (for example, malformed
/// path components on the host platform).
pub(super) fn file_uri(path: &Path) -> Result<String, CardExtractionError> {
    if !path.is_absolute() {
        return Err(CardExtractionError::InvalidPath {
            path: path.to_path_buf(),
        });
    }

    Url::from_file_path(path)
        .map(|uri| uri.to_string())
        .map_err(|()| CardExtractionError::InvalidPath {
            path: path.to_path_buf(),
        })
}
