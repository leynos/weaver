//! Unit tests for daemon-side capability resolution.

use std::path::{Path, PathBuf};

use weaver_plugins::{
    CapabilityId,
    PluginManifest,
    PluginRegistry,
    manifest::{PluginKind, PluginMetadata},
};

use crate::{
    dispatch::act::refactor::resolution::{
        CandidateReason,
        CapabilityResolutionDetails,
        RefusalReason,
        ResolutionOutcome,
        ResolutionRequest,
        SelectionMode,
        resolve_provider,
    },
    test_helpers::ensure_same,
};

fn registry() -> Result<PluginRegistry, String> {
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
    registry
        .register(rope)
        .map_err(|e| format!("register rope: {e}"))?;
    registry
        .register(rust_analyzer)
        .map_err(|e| format!("register rust-analyzer: {e}"))?;
    Ok(registry)
}

fn resolution_for(
    path: &str,
    provider: Option<&str>,
) -> Result<super::resolution::CapabilityResolutionEnvelope, String> {
    let reg = registry()?;
    Ok(resolve_provider(
        &reg,
        ResolutionRequest::new(CapabilityId::RenameSymbol, Path::new(path), provider),
    ))
}

fn assert_provider_selected(
    details: &CapabilityResolutionDetails,
    provider: &str,
    mode: SelectionMode,
    language: &str,
) -> Result<(), String> {
    ensure_same(
        details.selected_provider(),
        Some(provider),
        "selected provider",
    )?;
    ensure_same(details.selection_mode, mode, "selection mode")?;
    ensure_same(details.outcome, ResolutionOutcome::Selected, "outcome")?;
    ensure_same(details.language.as_deref(), Some(language), "language")
}

fn assert_provider_refused(
    details: &CapabilityResolutionDetails,
    reason: RefusalReason,
) -> Result<(), String> {
    ensure_same(details.selected_provider(), None, "selected provider")?;
    ensure_same(details.outcome, ResolutionOutcome::Refused, "outcome")?;
    ensure_same(details.refusal_reason, Some(reason), "refusal reason")
}

#[test]
fn automatic_python_selection_prefers_rope() -> Result<(), String> {
    let envelope = resolution_for("src/main.py", None)?;
    let details = envelope.details();

    assert_provider_selected(details, "rope", SelectionMode::Automatic, "python")?;
    ensure_same(details.candidates.len(), 2, "candidate count")?;
    let first_reason = details.candidates.first().map(|candidate| candidate.reason);
    ensure_same(
        first_reason,
        Some(CandidateReason::MatchedLanguageAndCapability),
        "first candidate reason",
    )
}

#[test]
fn automatic_rust_selection_prefers_rust_analyzer() -> Result<(), String> {
    let envelope = resolution_for("src/main.rs", None)?;
    let details = envelope.details();

    assert_provider_selected(details, "rust-analyzer", SelectionMode::Automatic, "rust")?;
    let rope_reason = details
        .candidates
        .iter()
        .find(|candidate| candidate.provider == "rope")
        .map(|candidate| candidate.reason);
    ensure_same(
        rope_reason,
        Some(CandidateReason::UnsupportedLanguage),
        "rope candidate reason",
    )
}

#[test]
fn explicit_provider_mismatch_is_refused() -> Result<(), String> {
    let envelope = resolution_for("src/main.py", Some("rust-analyzer"))?;
    let details = envelope.details();

    ensure_same(
        details.selection_mode,
        SelectionMode::ExplicitProvider,
        "selection mode",
    )?;
    assert_provider_refused(details, RefusalReason::ExplicitProviderMismatch)
}

#[test]
fn explicit_provider_selection_succeeds_when_language_matches() -> Result<(), String> {
    let envelope = resolution_for("src/main.py", Some("rope"))?;
    let details = envelope.details();

    assert_provider_selected(details, "rope", SelectionMode::ExplicitProvider, "python")?;
    ensure_same(
        details.requested_provider.as_deref(),
        Some("rope"),
        "requested provider",
    )
}

#[test]
fn unknown_explicit_provider_is_refused() -> Result<(), String> {
    let envelope = resolution_for("src/main.py", Some("missing-provider"))?;
    let details = envelope.details();

    ensure_same(
        details.selection_mode,
        SelectionMode::ExplicitProvider,
        "selection mode",
    )?;
    assert_provider_refused(details, RefusalReason::ProviderNotFound)
}

#[test]
fn unsupported_language_is_refused() -> Result<(), String> {
    let envelope = resolution_for("README.md", None)?;
    let details = envelope.details();

    assert_provider_refused(details, RefusalReason::UnsupportedLanguage)
}

#[test]
fn supported_language_without_provider_is_refused_deterministically() -> Result<(), String> {
    let envelope = resolution_for("src/main.ts", None)?;
    let details = envelope.details();

    ensure_same(details.language.as_deref(), Some("typescript"), "language")?;
    assert_provider_refused(details, RefusalReason::NoMatchingProvider)?;
    if details
        .candidates
        .iter()
        .all(|candidate| candidate.reason == CandidateReason::UnsupportedLanguage)
    {
        Ok(())
    } else {
        Err(format!(
            "expected deterministic unsupported-language rejections, got: {:?}",
            details.candidates
        ))
    }
}
