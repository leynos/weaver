//! Shared resolution envelopes and routing helpers for refactor tests.

use weaver_plugins::CapabilityId;

use crate::dispatch::act::refactor::resolution::{
    CandidateEvaluation,
    CandidateReason,
    CapabilityResolutionDetails,
    CapabilityResolutionEnvelope,
    RefusalReason,
    ResolutionOutcome,
    SelectionMode,
};

pub(crate) struct RefusedResolution<'a> {
    pub(crate) capability: CapabilityId,
    pub(crate) language: Option<&'a str>,
    pub(crate) requested_provider: Option<&'a str>,
    pub(crate) selection_mode: SelectionMode,
    pub(crate) refusal_reason: RefusalReason,
    pub(crate) candidates: Vec<CandidateEvaluation>,
}

pub(crate) struct SelectedResolution<'a> {
    pub(crate) capability: CapabilityId,
    pub(crate) language: &'a str,
    pub(crate) provider: &'a str,
    pub(crate) selection_mode: SelectionMode,
    pub(crate) requested_provider: Option<&'a str>,
}

pub(crate) fn selected_resolution(config: SelectedResolution<'_>) -> CapabilityResolutionEnvelope {
    CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
        capability: config.capability,
        language: Some(String::from(config.language)),
        requested_provider: config.requested_provider.map(String::from),
        selected_provider: Some(String::from(config.provider)),
        selection_mode: config.selection_mode,
        outcome: ResolutionOutcome::Selected,
        refusal_reason: None,
        candidates: vec![CandidateEvaluation {
            provider: String::from(config.provider),
            accepted: true,
            reason: CandidateReason::MatchedLanguageAndCapability,
        }],
    })
}

pub(crate) fn refused_resolution(config: RefusedResolution<'_>) -> CapabilityResolutionEnvelope {
    CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
        capability: config.capability,
        language: config.language.map(String::from),
        requested_provider: config.requested_provider.map(String::from),
        selected_provider: None,
        selection_mode: config.selection_mode,
        outcome: ResolutionOutcome::Refused,
        refusal_reason: Some(config.refusal_reason),
        candidates: config.candidates,
    })
}

pub(crate) fn rejected_candidate(provider: &str, reason: CandidateReason) -> CandidateEvaluation {
    CandidateEvaluation {
        provider: String::from(provider),
        accepted: false,
        reason,
    }
}

pub(crate) struct AutoResolutionContext<'a> {
    pub(crate) capability: CapabilityId,
    pub(crate) requested_provider: Option<&'a str>,
    pub(crate) selection_mode: SelectionMode,
}

pub(crate) fn resolve_auto_language(
    context: AutoResolutionContext<'_>,
    language_name: Option<&'static str>,
    provider: &'static str,
    candidates: Vec<CandidateEvaluation>,
) -> CapabilityResolutionEnvelope {
    if let Some(language) = language_name {
        selected_resolution(SelectedResolution {
            capability: context.capability,
            language,
            provider,
            selection_mode: context.selection_mode,
            requested_provider: context.requested_provider,
        })
    } else {
        refused_resolution(RefusedResolution {
            capability: context.capability,
            language: None,
            requested_provider: context.requested_provider,
            selection_mode: context.selection_mode,
            refusal_reason: RefusalReason::UnsupportedLanguage,
            candidates,
        })
    }
}

const _: for<'a> fn(SelectedResolution<'a>) -> CapabilityResolutionEnvelope = selected_resolution;
const _: for<'a> fn(RefusedResolution<'a>) -> CapabilityResolutionEnvelope = refused_resolution;
const _: fn(&str, CandidateReason) -> CandidateEvaluation = rejected_candidate;
const _: for<'a> fn(
    AutoResolutionContext<'a>,
    Option<&'static str>,
    &'static str,
    Vec<CandidateEvaluation>,
) -> CapabilityResolutionEnvelope = resolve_auto_language;
