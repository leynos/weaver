//! Refusal construction for capability resolution.

use weaver_plugins::capability::CapabilityId;
use weaver_syntax::SupportedLanguage;

use super::resolution::{
    CandidateEvaluation, CapabilityResolutionDetails, CapabilityResolutionEnvelope, RefusalReason,
    ResolutionOutcome, SelectionMode,
};

/// Groups routing metadata for refusal construction.
#[derive(Debug)]
pub(super) struct RoutingContext {
    pub(super) capability: CapabilityId,
    pub(super) language: Option<SupportedLanguage>,
    pub(super) requested_provider: Option<String>,
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
