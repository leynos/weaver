//! Capability-resolution `stderr` payloads emitted by the fake daemon.
//!
//! These mirror the shape the real daemon reports when it selects a refactoring
//! provider, so the CLI snapshots exercise the same rendering path.

use std::path::Path;

use serde_json::json;

use super::{RequestedProvider, language_for_extension};

/// Builds a capability-resolution `stderr` JSON payload for automatic
/// provider selection based on the file extension.
///
/// Returns `None` when the file extension is not recognised.
pub(super) fn automatic_resolution_payload(file: &Path) -> Option<String> {
    match language_for_extension(file) {
        Some("python") => Some(
            json!({
                "status": "ok",
                "type": "CapabilityResolution",
                "details": {
                    "capability": "rename-symbol",
                    "language": "python",
                    "selected_provider": "rope",
                    "selection_mode": "automatic",
                    "outcome": "selected",
                    "candidates": [
                        { "provider": "rope", "accepted": true, "reason": "matched_language_and_capability" },
                        { "provider": "rust-analyzer", "accepted": false, "reason": "unsupported_language" }
                    ]
                }
            })
            .to_string(),
        ),
        Some("rust") => Some(
            json!({
                "status": "ok",
                "type": "CapabilityResolution",
                "details": {
                    "capability": "rename-symbol",
                    "language": "rust",
                    "selected_provider": "rust-analyzer",
                    "selection_mode": "automatic",
                    "outcome": "selected",
                    "candidates": [
                        { "provider": "rust-analyzer", "accepted": true, "reason": "matched_language_and_capability" },
                        { "provider": "rope", "accepted": false, "reason": "unsupported_language" }
                    ]
                }
            })
            .to_string(),
        ),
        _ => None,
    }
}

/// Builds a refused capability-resolution `stderr` JSON payload for the case
/// where an explicitly requested provider does not support the file's
/// language.
///
/// Returns `None` when the provider and language are compatible, or when the
/// file extension is not recognised.
pub(super) fn provider_mismatch_payload(
    file: &Path,
    provider: RequestedProvider,
) -> Option<String> {
    let language = language_for_extension(file)?;
    let mismatched = matches!(
        (language, provider),
        ("python", RequestedProvider::RustAnalyzer) | ("rust", RequestedProvider::Rope)
    );
    if !mismatched {
        return None;
    }

    // The real daemon sorts candidates alphabetically by provider name
    // (`sorted_capability_manifests`), so "rope" always precedes
    // "rust-analyzer" regardless of language or requested provider. Each
    // entry's reason then depends solely on whether it is the requested
    // (but mismatched) provider.
    let requested = provider.as_str();
    let candidates = [
        ("rope", "not_requested"),
        ("rust-analyzer", "not_requested"),
    ]
    .map(|(candidate_provider, default_reason)| {
        let reason = if candidate_provider == requested {
            "explicit_provider_mismatch"
        } else {
            default_reason
        };
        json!({
            "provider": candidate_provider,
            "accepted": false,
            "reason": reason
        })
    });

    Some(
        json!({
            "status": "error",
            "type": "CapabilityResolution",
            "details": {
                "capability": "rename-symbol",
                "language": language,
                "requested_provider": provider.as_str(),
                "selection_mode": "explicit_provider",
                "outcome": "refused",
                "refusal_reason": "explicit_provider_mismatch",
                "candidates": candidates
            }
        })
        .to_string(),
    )
}
