//! Shared helper data and pure functions for `act refactor` behaviour tests.
//!
//! This file is intentionally loaded multiple times by different test modules
//! using `#[path = "refactor_helpers.rs"]` to provide shared test utilities.

#![allow(dead_code)]

use std::path::Path;

use weaver_cards::DEFAULT_CACHE_CAPACITY;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_plugins::{CapabilityId, PluginError, PluginRequest, PluginResponse};

use crate::backends::FusionBackends;
use crate::dispatch::act::refactor::RefactorPluginRuntime;
use crate::dispatch::act::refactor::resolution::{
    CandidateEvaluation, CandidateReason, CapabilityResolutionDetails,
    CapabilityResolutionEnvelope, RefusalReason, ResolutionOutcome, ResolutionRequest,
    SelectionMode,
};
use crate::dispatch::request::{CommandDescriptor, CommandRequest};
use crate::semantic_provider::SemanticBackendProvider;

pub(super) struct RefusedResolution<'a> {
    pub(super) capability: CapabilityId,
    pub(super) language: Option<&'a str>,
    pub(super) requested_provider: Option<&'a str>,
    pub(super) selection_mode: SelectionMode,
    pub(super) refusal_reason: RefusalReason,
    pub(super) candidates: Vec<CandidateEvaluation>,
}

pub(super) struct SelectedResolution<'a> {
    pub(super) capability: CapabilityId,
    pub(super) language: &'a str,
    pub(super) provider: &'a str,
    pub(super) selection_mode: SelectionMode,
    pub(super) requested_provider: Option<&'a str>,
}

pub(super) struct RollbackRuntime {
    pub(super) resolution: CapabilityResolutionEnvelope,
    pub(super) execute_result: ExecuteResult,
}

pub(super) enum ExecuteResult {
    Success(PluginResponse),
    MissingPlugin(&'static str),
}

impl RefactorPluginRuntime for RollbackRuntime {
    fn resolve(
        &self,
        _request: ResolutionRequest<'_>,
    ) -> Result<CapabilityResolutionEnvelope, PluginError> {
        Ok(self.resolution.clone())
    }

    fn execute(
        &self,
        _provider: &str,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        match &self.execute_result {
            ExecuteResult::Success(response) => Ok(response.clone()),
            ExecuteResult::MissingPlugin(name) => Err(PluginError::NotFound {
                name: String::from(*name),
            }),
        }
    }
}

pub(super) fn command_request(arguments: Vec<String>) -> CommandRequest {
    CommandRequest {
        command: CommandDescriptor {
            domain: String::from("act"),
            operation: String::from("refactor"),
        },
        arguments,
        patch: None,
    }
}

pub(super) fn build_backends(socket_path: &Path) -> FusionBackends<SemanticBackendProvider> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().as_ref()),
        ..Config::default()
    };
    let provider =
        SemanticBackendProvider::new(CapabilityMatrix::default(), DEFAULT_CACHE_CAPACITY);
    FusionBackends::new(config, provider)
}

pub(super) fn standard_rename_args(file: &str) -> Vec<String> {
    vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from(file),
        String::from("offset=1"),
        String::from("new_name=woven"),
    ]
}

pub(super) fn configure_request(request: &mut CommandRequest, args: Vec<String>) {
    *request = command_request(args);
}

pub(super) fn selected_resolution(config: SelectedResolution<'_>) -> CapabilityResolutionEnvelope {
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

pub(super) fn selected_runtime(
    config: SelectedResolution<'_>,
    execute_result: ExecuteResult,
) -> RollbackRuntime {
    RollbackRuntime {
        resolution: selected_resolution(config),
        execute_result,
    }
}

pub(super) fn rollback_runtime(
    resolution: CapabilityResolutionEnvelope,
    execute_result: ExecuteResult,
) -> RollbackRuntime {
    RollbackRuntime {
        resolution,
        execute_result,
    }
}

pub(super) fn refused_resolution(config: RefusedResolution<'_>) -> CapabilityResolutionEnvelope {
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

pub(super) fn rejected_candidate(provider: &str, reason: CandidateReason) -> CandidateEvaluation {
    CandidateEvaluation {
        provider: String::from(provider),
        accepted: false,
        reason,
    }
}

fn format_diff(path: &Path, git_header: &str) -> String {
    let original = original_content_for(path);
    let updated = updated_content_for(path);
    format!("{git_header}\n<<<<<<< SEARCH\n{original}=======\n{updated}>>>>>>> REPLACE\n",)
}

pub(super) fn routed_patch_path(path: &Path) -> &Path {
    match classify_file(path) {
        FileKind::Python | FileKind::Rust => Path::new("notes.txt"),
        FileKind::Other => path,
    }
}

fn routed_format_diff(path: &Path, make_header: impl Fn(&str) -> String) -> String {
    let patch_path = routed_patch_path(path);
    format_diff(path, &make_header(&patch_path.to_string_lossy()))
}

pub(super) fn routed_diff_for(path: &Path) -> String {
    routed_format_diff(path, |p| format!("diff --git a/{p} b/{p}"))
}

pub(super) fn routed_malformed_diff_for(path: &Path) -> String {
    routed_format_diff(path, |p| format!("diff --git a/{p}"))
}

pub(super) enum FileKind {
    Python,
    Rust,
    Other,
}

pub(super) fn classify_file(path: &Path) -> FileKind {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("py") => FileKind::Python,
        Some("rs") => FileKind::Rust,
        _ => FileKind::Other,
    }
}

struct FileContents {
    original: &'static str,
    updated: &'static str,
}

fn content_table(kind: FileKind) -> FileContents {
    match kind {
        FileKind::Python => FileContents {
            original: "old_name = 1\nprint(old_name)\n",
            updated: "woven = 1\nprint(woven)\n",
        },
        FileKind::Rust => FileContents {
            original: concat!(
                "fn main() {\n",
                "    let old_name = 1;\n",
                "    println!(\"{}\", old_name);\n",
                "}\n",
            ),
            updated: concat!(
                "fn main() {\n",
                "    let woven = 1;\n",
                "    println!(\"{}\", woven);\n",
                "}\n",
            ),
        },
        FileKind::Other => FileContents {
            original: "hello world\n",
            updated: "hello woven\n",
        },
    }
}

pub(super) fn original_content_for(path: &Path) -> &'static str {
    content_table(classify_file(path)).original
}

pub(super) fn updated_content_for(path: &Path) -> &'static str {
    content_table(classify_file(path)).updated
}
