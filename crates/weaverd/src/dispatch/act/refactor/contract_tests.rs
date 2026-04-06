//! Unit tests for the `rename-symbol` request mapping contract.

use std::sync::Mutex;

use rstest::{fixture, rstest};
use tempfile::TempDir;
use weaver_plugins::{CapabilityId, PluginError, PluginOutput, PluginRequest, PluginResponse};
use weaver_test_macros::allow_fixture_expansion_lints;

#[expect(
    clippy::duplicate_mod,
    reason = "Shared test helpers loaded by multiple test modules"
)]
#[path = "refactor_helpers.rs"]
mod refactor_helpers;

use refactor_helpers::{build_backends, command_request};

use crate::dispatch::act::refactor::{
    RefactorContext,
    RefactorPluginRuntime,
    ResponseWriter,
    handle,
    resolution::{
        CandidateEvaluation,
        CapabilityResolutionDetails,
        CapabilityResolutionEnvelope,
        ResolutionOutcome,
        ResolutionRequest,
        SelectionMode,
    },
    rust_analyzer_manifest,
};

struct InspectingRuntime {
    captured: Mutex<Option<PluginRequest>>,
    response: PluginResponse,
    provider: &'static str,
    language: &'static str,
}

impl RefactorPluginRuntime for InspectingRuntime {
    fn resolve(
        &self,
        _request: ResolutionRequest<'_>,
    ) -> Result<CapabilityResolutionEnvelope, PluginError> {
        Ok(CapabilityResolutionEnvelope::from_details(
            CapabilityResolutionDetails {
                capability: CapabilityId::RenameSymbol,
                language: Some(String::from(self.language)),
                requested_provider: None,
                selected_provider: Some(String::from(self.provider)),
                selection_mode: SelectionMode::Automatic,
                outcome: ResolutionOutcome::Selected,
                refusal_reason: None,
                candidates: vec![CandidateEvaluation {
                    provider: String::from(self.provider),
                    accepted: true,
                    reason: super::resolution::CandidateReason::MatchedLanguageAndCapability,
                }],
            },
        ))
    }

    fn execute(
        &self,
        _provider: &str,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        *self.captured.lock().expect("lock") = Some(request.clone());
        Ok(self.response.clone())
    }
}

const NOTES_DIFF: &str = concat!(
    "diff --git a/notes.txt b/notes.txt\n",
    "<<<<<<< SEARCH\n",
    "hello world\n",
    "=======\n",
    "hello woven\n",
    ">>>>>>> REPLACE\n",
);

#[allow_fixture_expansion_lints]
#[fixture]
fn socket_dir() -> TempDir { TempDir::new().expect("socket dir") }

struct RenameDispatch<'a> {
    file: &'a str,
    provider: &'static str,
    language: &'static str,
    extra_args: Vec<String>,
    socket_dir: &'a TempDir,
}

/// Dispatches a rename request through the handler and returns the captured
/// `PluginRequest` for inspection.
fn dispatch_inspecting_rename(config: RenameDispatch<'_>) -> PluginRequest {
    let workspace = TempDir::new().expect("workspace");
    std::fs::write(workspace.path().join(config.file), "hello world\n").expect("write");
    let runtime = InspectingRuntime {
        captured: Mutex::new(None),
        response: PluginResponse::success(PluginOutput::Diff {
            content: NOTES_DIFF.replace("notes.txt", config.file),
        }),
        provider: config.provider,
        language: config.language,
    };
    let mut args = vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from(config.file),
    ];
    args.extend(config.extra_args);
    let request = command_request(args);
    let socket_path = config.socket_dir.path().join("socket.sock");
    let mut backends = build_backends(&socket_path);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    handle(
        &request,
        &mut writer,
        RefactorContext {
            backends: &mut backends,
            workspace_root: workspace.path(),
            runtime: &runtime,
        },
    )
    .expect("dispatch result");
    runtime
        .captured
        .into_inner()
        .expect("lock")
        .expect("request should be captured")
}

#[rstest]
fn handler_sends_rename_symbol_contract_conforming_request(socket_dir: TempDir) {
    let plugin_request = dispatch_inspecting_rename(RenameDispatch {
        file: "notes.py",
        provider: "rope",
        language: "python",
        extra_args: vec![String::from("offset=4"), String::from("new_name=woven")],
        socket_dir: &socket_dir,
    });

    assert_eq!(plugin_request.operation(), "rename-symbol");
    let args = plugin_request.arguments();
    assert_eq!(
        args.get("uri").and_then(|value| value.as_str()),
        Some("file:///notes.py"),
    );
    assert_eq!(
        args.get("position").and_then(|value| value.as_str()),
        Some("4")
    );
    assert!(!args.contains_key("offset"));
    assert_eq!(
        args.get("new_name").and_then(|value| value.as_str()),
        Some("woven")
    );
}

#[rstest]
fn handler_overwrites_pre_existing_uri_with_file_path(socket_dir: TempDir) {
    let plugin_request = dispatch_inspecting_rename(RenameDispatch {
        file: "notes.py",
        provider: "rope",
        language: "python",
        extra_args: vec![
            String::from("uri=stale_value"),
            String::from("offset=4"),
            String::from("new_name=woven"),
        ],
        socket_dir: &socket_dir,
    });

    assert_eq!(
        plugin_request
            .arguments()
            .get("uri")
            .and_then(|value| value.as_str()),
        Some("file:///notes.py"),
    );
}

#[rstest]
fn handler_omits_position_when_offset_not_provided(socket_dir: TempDir) {
    let plugin_request = dispatch_inspecting_rename(RenameDispatch {
        file: "notes.py",
        provider: "rope",
        language: "python",
        extra_args: vec![String::from("new_name=woven")],
        socket_dir: &socket_dir,
    });

    assert!(!plugin_request.arguments().contains_key("position"));
}

#[rstest]
fn rust_analyzer_provider_uses_rename_symbol_contract(socket_dir: TempDir) {
    let plugin_request = dispatch_inspecting_rename(RenameDispatch {
        file: "notes.rs",
        provider: "rust-analyzer",
        language: "rust",
        extra_args: vec![String::from("offset=4"), String::from("new_name=woven")],
        socket_dir: &socket_dir,
    });

    assert_eq!(plugin_request.operation(), "rename-symbol");
    assert_eq!(
        plugin_request
            .arguments()
            .get("uri")
            .and_then(|value| value.as_str()),
        Some("file:///notes.rs"),
    );
    assert_eq!(
        plugin_request
            .arguments()
            .get("position")
            .and_then(|value| value.as_str()),
        Some("4"),
    );
}

#[test]
fn rust_analyzer_manifest_declares_rename_symbol_capability() {
    let manifest = rust_analyzer_manifest(std::path::PathBuf::from(
        "/usr/bin/weaver-plugin-rust-analyzer",
    ));

    assert_eq!(manifest.capabilities(), &[CapabilityId::RenameSymbol]);
}
