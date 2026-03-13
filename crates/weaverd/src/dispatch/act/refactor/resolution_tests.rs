//! Unit tests for daemon-side capability resolution.

use std::path::{Path, PathBuf};

use weaver_plugins::manifest::{PluginKind, PluginMetadata};
use weaver_plugins::{CapabilityId, PluginManifest, PluginRegistry};

use super::resolution::{
    CandidateReason, RefusalReason, ResolutionOutcome, ResolutionRequest, SelectionMode,
    resolve_provider,
};

fn registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    let rope = PluginManifest::new(
        PluginMetadata::new("rope", "1.0.0", PluginKind::Actuator),
        vec![String::from("python")],
        PathBuf::from("/usr/bin/weaver-plugin-rope"),
    )
    .with_capabilities(vec![CapabilityId::RenameSymbol]);
    let rust_analyzer = PluginManifest::new(
        PluginMetadata::new("rust-analyzer", "1.0.0", PluginKind::Actuator),
        vec![String::from("rust")],
        PathBuf::from("/usr/bin/weaver-plugin-rust-analyzer"),
    )
    .with_capabilities(vec![CapabilityId::RenameSymbol]);
    registry.register(rope).expect("register rope");
    registry
        .register(rust_analyzer)
        .expect("register rust-analyzer");
    registry
}

fn resolution_for(
    path: &str,
    provider: Option<&str>,
) -> super::resolution::CapabilityResolutionEnvelope {
    resolve_provider(
        &registry(),
        ResolutionRequest::new(CapabilityId::RenameSymbol, Path::new(path), provider),
    )
}

#[test]
fn automatic_python_selection_prefers_rope() {
    let envelope = resolution_for("src/main.py", None);
    let details = envelope.details();

    assert_eq!(details.selected_provider(), Some("rope"));
    assert_eq!(details.selection_mode, SelectionMode::Automatic);
    assert_eq!(details.outcome, ResolutionOutcome::Selected);
    assert_eq!(details.language.as_deref(), Some("python"));
    assert_eq!(details.candidates.len(), 2);
    assert_eq!(
        details.candidates[0].reason,
        CandidateReason::MatchedLanguageAndCapability
    );
}

#[test]
fn automatic_rust_selection_prefers_rust_analyzer() {
    let envelope = resolution_for("src/main.rs", None);
    let details = envelope.details();

    assert_eq!(details.selected_provider(), Some("rust-analyzer"));
    assert_eq!(details.selection_mode, SelectionMode::Automatic);
    assert_eq!(details.outcome, ResolutionOutcome::Selected);
    assert_eq!(details.language.as_deref(), Some("rust"));
    assert_eq!(
        details
            .candidates
            .iter()
            .find(|candidate| candidate.provider == "rope")
            .map(|candidate| candidate.reason),
        Some(CandidateReason::UnsupportedLanguage)
    );
}

#[test]
fn explicit_provider_mismatch_is_refused() {
    let envelope = resolution_for("src/main.py", Some("rust-analyzer"));
    let details = envelope.details();

    assert_eq!(details.selected_provider(), None);
    assert_eq!(details.selection_mode, SelectionMode::ExplicitProvider);
    assert_eq!(details.outcome, ResolutionOutcome::Refused);
    assert_eq!(
        details.refusal_reason,
        Some(RefusalReason::ExplicitProviderMismatch)
    );
}

#[test]
fn unknown_explicit_provider_is_refused() {
    let envelope = resolution_for("src/main.py", Some("missing-provider"));
    let details = envelope.details();

    assert_eq!(details.selected_provider(), None);
    assert_eq!(details.selection_mode, SelectionMode::ExplicitProvider);
    assert_eq!(details.outcome, ResolutionOutcome::Refused);
    assert_eq!(
        details.refusal_reason,
        Some(RefusalReason::ProviderNotFound)
    );
}

#[test]
fn unsupported_language_is_refused() {
    let envelope = resolution_for("README.md", None);
    let details = envelope.details();

    assert_eq!(details.selected_provider(), None);
    assert_eq!(details.outcome, ResolutionOutcome::Refused);
    assert_eq!(
        details.refusal_reason,
        Some(RefusalReason::UnsupportedLanguage)
    );
}

#[test]
fn supported_language_without_provider_is_refused_deterministically() {
    let envelope = resolution_for("src/main.ts", None);
    let details = envelope.details();

    assert_eq!(details.selected_provider(), None);
    assert_eq!(details.language.as_deref(), Some("typescript"));
    assert_eq!(details.outcome, ResolutionOutcome::Refused);
    assert_eq!(
        details.refusal_reason,
        Some(RefusalReason::NoMatchingProvider)
    );
}
