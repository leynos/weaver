//! Capability resolution for `act refactor`.
//!
//! The daemon uses this module to choose a plugin for `rename-symbol` based on
//! the requested capability, inferred language, and any explicit provider
//! override supplied by the operator.

use std::path::Path;

use serde::Serialize;
use weaver_plugins::PluginRegistry;
use weaver_plugins::capability::CapabilityId;
use weaver_plugins::manifest::PluginManifest;
use weaver_syntax::SupportedLanguage;

/// Stable envelope type written to the daemon output stream.
pub(crate) const CAPABILITY_RESOLUTION_TYPE: &str = "CapabilityResolution";

/// Machine-readable routing rationale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CapabilityResolutionEnvelope {
    status: &'static str,
    #[serde(rename = "type")]
    kind: &'static str,
    details: CapabilityResolutionDetails,
}

impl CapabilityResolutionEnvelope {
    /// Builds an envelope from a routing decision.
    #[must_use]
    pub(crate) fn from_details(details: CapabilityResolutionDetails) -> Self {
        let status = if details.selected_provider.is_some() {
            "ok"
        } else {
            "error"
        };
        Self {
            status,
            kind: CAPABILITY_RESOLUTION_TYPE,
            details,
        }
    }

    /// Returns the inner details payload.
    #[must_use]
    pub(crate) const fn details(&self) -> &CapabilityResolutionDetails {
        &self.details
    }
}

/// Detailed routing decision captured as data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CapabilityResolutionDetails {
    pub(crate) capability: CapabilityId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) requested_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) selected_provider: Option<String>,
    pub(crate) selection_mode: SelectionMode,
    pub(crate) outcome: ResolutionOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) refusal_reason: Option<RefusalReason>,
    pub(crate) candidates: Vec<CandidateEvaluation>,
}

impl CapabilityResolutionDetails {
    /// Returns the selected provider, if routing succeeded.
    #[must_use]
    pub(crate) fn selected_provider(&self) -> Option<&str> {
        self.selected_provider.as_deref()
    }
}

/// Resolution mode used for the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SelectionMode {
    /// The operator supplied `--provider`, so the daemon validates it.
    ExplicitProvider,
    /// The daemon chose the provider from language policy.
    Automatic,
}

/// High-level routing outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResolutionOutcome {
    /// A provider was selected successfully.
    Selected,
    /// Routing stopped before plugin execution.
    Refused,
}

/// Stable refusal reasons for route-level failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RefusalReason {
    /// The daemon could not infer a supported language from the target file.
    UnsupportedLanguage,
    /// The requested provider name does not exist in the registry.
    ProviderNotFound,
    /// The requested provider does not support the inferred language.
    ExplicitProviderMismatch,
    /// No registered provider matched the inferred language and capability.
    NoMatchingProvider,
}

/// Candidate-by-candidate explanation of the routing choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CandidateEvaluation {
    pub(crate) provider: String,
    pub(crate) accepted: bool,
    pub(crate) reason: CandidateReason,
}

/// Stable reasons attached to individual candidate evaluations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CandidateReason {
    /// The candidate supports both the requested language and capability.
    MatchedLanguageAndCapability,
    /// The candidate was ignored because a different provider was requested.
    NotRequested,
    /// The candidate supports the capability but not the inferred language.
    UnsupportedLanguage,
    /// The candidate matched, but a higher-priority policy choice won.
    NotSelectedByPolicy,
    /// The requested provider exists but does not support the inferred language.
    ExplicitProviderMismatch,
}

/// Resolver input for a single capability request.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolutionRequest<'a> {
    capability: CapabilityId,
    target_file: &'a Path,
    explicit_provider: Option<&'a str>,
}

#[derive(Debug)]
struct RefusalContext {
    capability: CapabilityId,
    language: Option<SupportedLanguage>,
    requested_provider: Option<String>,
    selection_mode: SelectionMode,
    refusal_reason: RefusalReason,
    candidates: Vec<CandidateEvaluation>,
}

impl<'a> ResolutionRequest<'a> {
    /// Creates a new resolution request.
    #[must_use]
    pub(crate) const fn new(
        capability: CapabilityId,
        target_file: &'a Path,
        explicit_provider: Option<&'a str>,
    ) -> Self {
        Self {
            capability,
            target_file,
            explicit_provider,
        }
    }
}

/// Resolves a provider from the registry using the built-in rename policy.
#[must_use]
pub(crate) fn resolve_provider(
    registry: &PluginRegistry,
    request: ResolutionRequest<'_>,
) -> CapabilityResolutionEnvelope {
    let language = SupportedLanguage::from_path(request.target_file);
    let selection_mode = if request.explicit_provider.is_some() {
        SelectionMode::ExplicitProvider
    } else {
        SelectionMode::Automatic
    };
    let candidates = sorted_capability_manifests(registry, request.capability);
    let requested_provider = request.explicit_provider.map(String::from);

    let Some(language) = language else {
        return refused(RefusalContext {
            capability: request.capability,
            language: None,
            requested_provider,
            selection_mode,
            refusal_reason: RefusalReason::UnsupportedLanguage,
            candidates: candidates
                .iter()
                .map(|manifest| rejected_candidate(manifest, CandidateReason::UnsupportedLanguage))
                .collect(),
        });
    };

    if let Some(provider_name) = request.explicit_provider {
        return resolve_explicit_provider(request.capability, language, provider_name, candidates);
    }

    resolve_automatic_provider(request.capability, language, candidates)
}

fn resolve_explicit_provider(
    capability: CapabilityId,
    language: SupportedLanguage,
    provider_name: &str,
    candidates: Vec<&PluginManifest>,
) -> CapabilityResolutionEnvelope {
    let mut found_requested = false;
    let mut selected_provider: Option<String> = None;
    let mut evaluations = Vec::with_capacity(candidates.len());

    for manifest in candidates {
        if manifest.name() != provider_name {
            evaluations.push(rejected_candidate(manifest, CandidateReason::NotRequested));
            continue;
        }

        found_requested = true;
        if manifest_supports_language(manifest, language) {
            selected_provider = Some(String::from(manifest.name()));
            evaluations.push(accepted_candidate(
                manifest,
                CandidateReason::MatchedLanguageAndCapability,
            ));
        } else {
            evaluations.push(rejected_candidate(
                manifest,
                CandidateReason::ExplicitProviderMismatch,
            ));
        }
    }

    let details = if let Some(selected_provider) = selected_provider {
        CapabilityResolutionDetails {
            capability,
            language: Some(String::from(language.as_str())),
            requested_provider: Some(String::from(provider_name)),
            selected_provider: Some(selected_provider),
            selection_mode: SelectionMode::ExplicitProvider,
            outcome: ResolutionOutcome::Selected,
            refusal_reason: None,
            candidates: evaluations,
        }
    } else if found_requested {
        refused_details(RefusalContext {
            capability,
            language: Some(language),
            requested_provider: Some(String::from(provider_name)),
            selection_mode: SelectionMode::ExplicitProvider,
            refusal_reason: RefusalReason::ExplicitProviderMismatch,
            candidates: evaluations,
        })
    } else {
        refused_details(RefusalContext {
            capability,
            language: Some(language),
            requested_provider: Some(String::from(provider_name)),
            selection_mode: SelectionMode::ExplicitProvider,
            refusal_reason: RefusalReason::ProviderNotFound,
            candidates: evaluations,
        })
    };

    CapabilityResolutionEnvelope::from_details(details)
}

fn resolve_automatic_provider(
    capability: CapabilityId,
    language: SupportedLanguage,
    candidates: Vec<&PluginManifest>,
) -> CapabilityResolutionEnvelope {
    let matching: Vec<&PluginManifest> = candidates
        .iter()
        .copied()
        .filter(|manifest| manifest_supports_language(manifest, language))
        .collect();

    if matching.is_empty() {
        return refused(RefusalContext {
            capability,
            language: Some(language),
            requested_provider: None,
            selection_mode: SelectionMode::Automatic,
            refusal_reason: RefusalReason::NoMatchingProvider,
            candidates: candidates
                .iter()
                .map(|manifest| rejected_candidate(manifest, CandidateReason::UnsupportedLanguage))
                .collect(),
        });
    }

    let selected_name = matching
        .iter()
        .min_by_key(|manifest| provider_rank(manifest.name(), language))
        .map(|manifest| manifest.name())
        .unwrap_or("unreachable");

    let evaluations = candidates
        .iter()
        .map(|manifest| {
            if !manifest_supports_language(manifest, language) {
                rejected_candidate(manifest, CandidateReason::UnsupportedLanguage)
            } else if manifest.name() == selected_name {
                accepted_candidate(manifest, CandidateReason::MatchedLanguageAndCapability)
            } else {
                rejected_candidate(manifest, CandidateReason::NotSelectedByPolicy)
            }
        })
        .collect();

    CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
        capability,
        language: Some(String::from(language.as_str())),
        requested_provider: None,
        selected_provider: Some(String::from(selected_name)),
        selection_mode: SelectionMode::Automatic,
        outcome: ResolutionOutcome::Selected,
        refusal_reason: None,
        candidates: evaluations,
    })
}

fn refused(context: RefusalContext) -> CapabilityResolutionEnvelope {
    CapabilityResolutionEnvelope::from_details(refused_details(context))
}

fn refused_details(context: RefusalContext) -> CapabilityResolutionDetails {
    CapabilityResolutionDetails {
        capability: context.capability,
        language: context
            .language
            .map(|language| String::from(language.as_str())),
        requested_provider: context.requested_provider,
        selected_provider: None,
        selection_mode: context.selection_mode,
        outcome: ResolutionOutcome::Refused,
        refusal_reason: Some(context.refusal_reason),
        candidates: context.candidates,
    }
}

fn sorted_capability_manifests(
    registry: &PluginRegistry,
    capability: CapabilityId,
) -> Vec<&PluginManifest> {
    let mut manifests = registry.find_for_capability(capability);
    manifests.sort_by(|left, right| left.name().cmp(right.name()));
    manifests
}

fn manifest_supports_language(manifest: &PluginManifest, language: SupportedLanguage) -> bool {
    manifest
        .languages()
        .iter()
        .any(|entry| entry == language.as_str())
}

fn provider_rank(provider_name: &str, language: SupportedLanguage) -> (usize, &str) {
    let preferred = preferred_provider(language);
    let rank = if provider_name == preferred { 0 } else { 1 };
    (rank, provider_name)
}

fn preferred_provider(language: SupportedLanguage) -> &'static str {
    match language {
        SupportedLanguage::Python => "rope",
        SupportedLanguage::Rust => "rust-analyzer",
        SupportedLanguage::TypeScript => "typescript-unimplemented",
    }
}

fn accepted_candidate(manifest: &PluginManifest, reason: CandidateReason) -> CandidateEvaluation {
    CandidateEvaluation {
        provider: String::from(manifest.name()),
        accepted: true,
        reason,
    }
}

fn rejected_candidate(manifest: &PluginManifest, reason: CandidateReason) -> CandidateEvaluation {
    CandidateEvaluation {
        provider: String::from(manifest.name()),
        accepted: false,
        reason,
    }
}
