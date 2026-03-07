//! Deterministic symbol fingerprinting for Tree-sitter cards.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use weaver_syntax::SupportedLanguage;

use super::EntityCandidate;

/// Builds a stable `SymbolId` string for an extracted entity.
pub(super) fn symbol_id(
    candidate: &EntityCandidate,
    language: SupportedLanguage,
    path: &Path,
) -> String {
    let mut hasher = DefaultHasher::new();
    language.as_str().hash(&mut hasher);
    candidate.kind.hash(&mut hasher);
    candidate.name.hash(&mut hasher);
    candidate.container.hash(&mut hasher);
    candidate.params.hash(&mut hasher);
    candidate.returns.hash(&mut hasher);
    candidate.structure_fingerprint.hash(&mut hasher);
    normalise_path_hint(path).hash(&mut hasher);
    format!("sym_{:016x}", hasher.finish())
}

/// Normalises the file path so it contributes weakly but deterministically to
/// the symbol fingerprint.
fn normalise_path_hint(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
