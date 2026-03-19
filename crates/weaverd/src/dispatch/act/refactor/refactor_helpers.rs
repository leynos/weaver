//! Shared helper data and pure functions for `act refactor` behaviour tests.

use std::path::Path;

use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_plugins::CapabilityId;

use crate::backends::FusionBackends;
use crate::dispatch::request::{CommandDescriptor, CommandRequest};
use crate::semantic_provider::SemanticBackendProvider;

use super::resolution::{
    CandidateEvaluation, CandidateReason, CapabilityResolutionDetails,
    CapabilityResolutionEnvelope, RefusalReason, ResolutionOutcome, SelectionMode,
};

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
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
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

pub(super) fn format_diff(relative_path: &str, git_header: &str) -> String {
    let original = original_content_for(relative_path);
    let updated = updated_content_for(relative_path);
    format!("{git_header}\n<<<<<<< SEARCH\n{original}=======\n{updated}>>>>>>> REPLACE\n",)
}

pub(super) fn diff_for(relative_path: &str) -> String {
    format_diff(
        relative_path,
        &format!("diff --git a/{0} b/{0}", relative_path),
    )
}

pub(super) fn malformed_diff_for(relative_path: &str) -> String {
    format_diff(relative_path, &format!("diff --git a/{0}", relative_path))
}

pub(super) fn routed_patch_path(relative_path: &str) -> &str {
    match classify_file(relative_path) {
        FileKind::Python | FileKind::Rust => "notes.txt",
        FileKind::Other => relative_path,
    }
}

pub(super) fn routed_diff_for(relative_path: &str) -> String {
    let patch_path = routed_patch_path(relative_path);
    if patch_path == relative_path {
        diff_for(relative_path)
    } else {
        format_diff(
            relative_path,
            &format!("diff --git a/{0} b/{0}", patch_path),
        )
    }
}

pub(super) fn routed_malformed_diff_for(relative_path: &str) -> String {
    let patch_path = routed_patch_path(relative_path);
    if patch_path == relative_path {
        malformed_diff_for(relative_path)
    } else {
        format_diff(relative_path, &format!("diff --git a/{0}", patch_path))
    }
}

pub(super) enum FileKind {
    Python,
    Rust,
    Other,
}

pub(super) fn classify_file(relative_path: &str) -> FileKind {
    if relative_path.ends_with(".py") {
        FileKind::Python
    } else if relative_path.ends_with(".rs") {
        FileKind::Rust
    } else {
        FileKind::Other
    }
}

pub(super) fn original_content_for(relative_path: &str) -> &'static str {
    match classify_file(relative_path) {
        FileKind::Python => "old_name = 1\nprint(old_name)\n",
        FileKind::Rust => concat!(
            "fn main() {\n",
            "    let old_name = 1;\n",
            "    println!(\"{}\", old_name);\n",
            "}\n",
        ),
        FileKind::Other => "hello world\n",
    }
}

pub(super) fn updated_content_for(relative_path: &str) -> &'static str {
    match classify_file(relative_path) {
        FileKind::Python => "woven = 1\nprint(woven)\n",
        FileKind::Rust => concat!(
            "fn main() {\n",
            "    let woven = 1;\n",
            "    println!(\"{}\", woven);\n",
            "}\n",
        ),
        FileKind::Other => "hello woven\n",
    }
}
