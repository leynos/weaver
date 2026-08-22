//! Stub refactor plugin runtime driving the `act refactor` behavioural suite.
//!
//! The stub replaces both capability resolution and plugin execution so the
//! scenarios can pin handler behaviour without launching real plugins.

use weaver_plugins::{PluginError, PluginOutput, PluginRequest, PluginResponse};
use weaver_syntax::SupportedLanguage;

use super::{
    content::{routed_diff_for, routed_malformed_diff_for},
    resolutions::{
        AutoResolutionContext,
        RefusedResolution,
        refused_resolution,
        rejected_candidate,
        resolve_auto_language,
    },
};
use crate::dispatch::act::refactor::{
    RefactorPluginRuntime,
    resolution::{
        CandidateEvaluation,
        CandidateReason,
        CapabilityResolutionEnvelope,
        RefusalReason,
        ResolutionRequest,
        SelectionMode,
    },
};

/// Plugin execution outcome the stub should produce.
#[derive(Clone, Copy, Default)]
pub(crate) enum RuntimeMode {
    #[default]
    DiffSuccess,
    RuntimeError,
    MalformedDiff,
    EmptySuccess,
}

/// Capability resolution outcome the stub should produce.
#[derive(Clone, Copy, Default)]
pub(crate) enum RoutingMode {
    #[default]
    AutomaticPython,
    AutomaticRust,
    UnsupportedLanguage,
    ExplicitProviderMismatch,
}

pub(crate) struct StubRuntime {
    pub(crate) routing: RoutingMode,
    pub(crate) execution: RuntimeMode,
}

fn refused_candidates(
    requested_provider: Option<&str>,
    default_reason: CandidateReason,
) -> Vec<CandidateEvaluation> {
    ["rope", "rust-analyzer"]
        .iter()
        .map(|&p| {
            let reason = if requested_provider == Some(p) {
                CandidateReason::ExplicitProviderMismatch
            } else {
                default_reason
            };
            rejected_candidate(p, reason)
        })
        .collect()
}

fn provider_for_auto(mode: RoutingMode) -> &'static str {
    match mode {
        RoutingMode::AutomaticPython => "rope",
        RoutingMode::AutomaticRust => "rust-analyzer",
        _ => unreachable!("provider_for_auto is only for automatic modes"),
    }
}

impl RefactorPluginRuntime for StubRuntime {
    fn resolve(
        &self,
        request: ResolutionRequest<'_>,
    ) -> Result<CapabilityResolutionEnvelope, PluginError> {
        let language = SupportedLanguage::from_path(request.target_file());
        let language_name = language.map(SupportedLanguage::as_str);
        let requested_provider = request.explicit_provider();
        let selection_mode = if requested_provider.is_some() {
            SelectionMode::ExplicitProvider
        } else {
            SelectionMode::Automatic
        };
        let auto_context = AutoResolutionContext {
            capability: request.capability(),
            requested_provider,
            selection_mode,
        };

        Ok(match self.routing {
            mode @ (RoutingMode::AutomaticPython | RoutingMode::AutomaticRust) => {
                resolve_auto_language(
                    auto_context,
                    language_name,
                    provider_for_auto(mode),
                    refused_candidates(requested_provider, CandidateReason::UnsupportedLanguage),
                )
            }
            RoutingMode::UnsupportedLanguage => refused_resolution(RefusedResolution {
                capability: request.capability(),
                language: language_name,
                requested_provider,
                selection_mode,
                refusal_reason: RefusalReason::UnsupportedLanguage,
                candidates: refused_candidates(
                    requested_provider,
                    CandidateReason::UnsupportedLanguage,
                ),
            }),
            RoutingMode::ExplicitProviderMismatch => refused_resolution(RefusedResolution {
                capability: request.capability(),
                language: language_name,
                requested_provider,
                selection_mode,
                refusal_reason: RefusalReason::ExplicitProviderMismatch,
                candidates: refused_candidates(requested_provider, CandidateReason::NotRequested),
            }),
        })
    }

    fn execute(
        &self,
        _provider: &str,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        let file_payload = request
            .files()
            .first()
            .ok_or_else(|| PluginError::NotFound {
                name: String::from("file payload"),
            })?;

        match self.execution {
            RuntimeMode::DiffSuccess => Ok(PluginResponse::success(PluginOutput::Diff {
                content: routed_diff_for(file_payload.path()),
            })),
            RuntimeMode::RuntimeError => Err(PluginError::NotFound {
                name: String::from("rope"),
            }),
            RuntimeMode::MalformedDiff => Ok(PluginResponse::success(PluginOutput::Diff {
                content: routed_malformed_diff_for(file_payload.path()),
            })),
            RuntimeMode::EmptySuccess => Ok(PluginResponse::success(PluginOutput::Empty)),
        }
    }
}
