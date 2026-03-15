//! Deterministic symbol fingerprinting for Tree-sitter cards.
//!
//! Fingerprints are SHA-256 digests over the canonical field order:
//! language, symbol kind, symbol name, container, parameter list, return type,
//! structure fingerprint, and the normalised path hint.

use std::path::Path;

use sha2::{Digest, Sha256};
use weaver_syntax::SupportedLanguage;

use super::EntityCandidate;

/// Builds a stable `SymbolId` string for an extracted entity.
pub(super) fn symbol_id(
    candidate: &EntityCandidate,
    language: SupportedLanguage,
    path: &Path,
) -> String {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, language.as_str());
    update_field(&mut hasher, &format!("{:?}", candidate.kind));
    update_field(&mut hasher, &candidate.name);
    update_field(
        &mut hasher,
        candidate.container.as_deref().unwrap_or_default(),
    );
    for param in &candidate.params {
        update_field(&mut hasher, &param.name);
        update_field(&mut hasher, &param.type_annotation);
    }
    update_field(&mut hasher, &candidate.returns);
    update_field(&mut hasher, &candidate.structure_fingerprint);
    update_field(&mut hasher, &normalise_path_hint(path));

    let digest = hasher.finalize();
    format!("sym_{}", hex_prefix(digest.iter().take(8).copied()))
}

fn update_field(hasher: &mut Sha256, field: &str) {
    hasher.update(field.len().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(field.as_bytes());
}

fn hex_prefix(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut output = String::with_capacity(16);
    for byte in bytes {
        output.push(nibble_to_hex(byte >> 4));
        output.push(nibble_to_hex(byte & 0x0f));
    }
    output
}

const fn nibble_to_hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + nibble - 10) as char,
        _ => '0',
    }
}

/// Normalises the file path so it contributes weakly but deterministically to
/// the symbol fingerprint.
fn normalise_path_hint(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
