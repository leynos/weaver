//! Candidate evaluation helpers for capability resolution.

use weaver_plugins::manifest::PluginManifest;
use weaver_syntax::SupportedLanguage;

use super::resolution::{CandidateEvaluation, CandidateReason};

/// Checks if a plugin manifest supports a given language.
pub(super) fn manifest_supports_language(
    manifest: &PluginManifest,
    language: SupportedLanguage,
) -> bool {
    manifest
        .languages()
        .iter()
        .any(|entry| entry == language.as_str())
}

/// Returns a ranking tuple for provider prioritization.
pub(super) fn provider_rank(provider_name: &str, language: SupportedLanguage) -> (usize, &str) {
    let preferred = preferred_provider(language);
    let rank = if provider_name == preferred { 0 } else { 1 };
    (rank, provider_name)
}

/// Returns the preferred provider for a given language.
pub(super) fn preferred_provider(language: SupportedLanguage) -> &'static str {
    match language {
        SupportedLanguage::Python => "rope",
        SupportedLanguage::Rust => "rust-analyzer",
        // TODO: Implement TypeScript provider support - this placeholder will cause routing to fail
        // for TypeScript files
        SupportedLanguage::TypeScript => "typescript-unimplemented",
    }
}

/// Creates an accepted candidate evaluation.
pub(super) fn accepted_candidate(
    manifest: &PluginManifest,
    reason: CandidateReason,
) -> CandidateEvaluation {
    CandidateEvaluation {
        provider: String::from(manifest.name()),
        accepted: true,
        reason,
    }
}

/// Creates a rejected candidate evaluation.
pub(super) fn rejected_candidate(
    manifest: &PluginManifest,
    reason: CandidateReason,
) -> CandidateEvaluation {
    CandidateEvaluation {
        provider: String::from(manifest.name()),
        accepted: false,
        reason,
    }
}
