//! Refusal construction for capability resolution.

use weaver_plugins::capability::CapabilityId;
use weaver_syntax::SupportedLanguage;

use super::resolution::{
    CandidateEvaluation,
    CapabilityResolutionDetails,
    CapabilityResolutionEnvelope,
    RefusalReason,
    ResolutionOutcome,
    SelectionMode,
};

/// Groups routing metadata for refusal construction.
#[derive(Debug)]
pub(super) struct RoutingContext {
    /// Capability that was being resolved.
    pub(super) capability: CapabilityId,
    /// Target language, when the request identified one.
    pub(super) language: Option<SupportedLanguage>,
    /// Provider the request asked for explicitly, if any.
    pub(super) requested_provider: Option<String>,
    /// How the provider would have been selected.
    pub(super) selection_mode: SelectionMode,
}

/// Constructs a refused resolution envelope.
pub(super) fn refused(
    context: RoutingContext,
    refusal_reason: RefusalReason,
    candidates: Vec<CandidateEvaluation>,
) -> CapabilityResolutionEnvelope {
    CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
        capability: context.capability,
        language: context.language.map(|l| l.as_str().to_owned()),
        requested_provider: context.requested_provider,
        selected_provider: None,
        selection_mode: context.selection_mode,
        outcome: ResolutionOutcome::Refused,
        refusal_reason: Some(refusal_reason),
        candidates,
    })
}
