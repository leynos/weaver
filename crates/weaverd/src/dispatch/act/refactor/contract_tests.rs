//! Unit tests for the `rename-symbol` request mapping contract.

use std::{path::PathBuf, sync::Mutex};

use rstest::{fixture, rstest};
use tempfile::TempDir;
use url::Url;
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
        let mut guard = self
            .captured
            .lock()
            .map_err(|error| PluginError::NotFound {
                name: format!("lock poisoned: {error}"),
            })?;
        *guard = Some(request.clone());
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
fn socket_dir() -> Result<TempDir, String> {
    TempDir::new().map_err(|e| format!("failed to create socket dir: {e}"))
}

struct RenameDispatch<'a> {
    file: &'a str,
    provider: &'static str,
    language: &'static str,
    extra_args: Vec<String>,
    socket_dir: &'a TempDir,
}

struct RenameExpectation<'a> {
    position: Option<&'a str>,
    new_name: Option<&'a str>,
}

struct RenameContractCase {
    file: &'static str,
    provider: &'static str,
    language: &'static str,
    extra_args: Vec<String>,
    position: Option<&'static str>,
    new_name: Option<&'static str>,
}

/// Dispatches a rename request through the handler and returns the captured
/// `PluginRequest` for inspection.
fn dispatch_inspecting_rename(
    config: RenameDispatch<'_>,
) -> Result<(PluginRequest, PathBuf), String> {
    let workspace = TempDir::new().map_err(|e| format!("workspace: {e}"))?;
    let file_path = workspace.path().join(config.file);
    std::fs::write(&file_path, "hello world\n").map_err(|e| format!("write: {e}"))?;
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
    .map_err(|e| format!("dispatch result: {e}"))?;

    let captured = runtime
        .captured
        .into_inner()
        .map_err(|error| format!("captured request lock poisoned: {error}"))?
        .ok_or_else(|| String::from("request should be captured"))?;
    Ok((captured, file_path))
}

fn assert_rename_request(
    config: RenameDispatch<'_>,
    expectation: RenameExpectation<'_>,
) -> Result<(PluginRequest, PathBuf), String> {
    let (plugin_request, file_path) = dispatch_inspecting_rename(config)?;
    let expected_uri = Url::from_file_path(&file_path)
        .map_err(|()| format!("failed to build URI for '{}'", file_path.display()))?
        .to_string();

    assert_eq!(plugin_request.operation(), "rename-symbol");
    let args = plugin_request.arguments();
    assert_eq!(
        args.get("uri").and_then(|value| value.as_str()),
        Some(expected_uri.as_str()),
    );
    assert_eq!(
        args.get("position").and_then(|value| value.as_str()),
        expectation.position
    );
    assert!(!args.contains_key("offset"));
    assert_eq!(
        args.get("new_name").and_then(|value| value.as_str()),
        expectation.new_name
    );

    Ok((plugin_request, file_path))
}

#[rstest]
#[case(RenameContractCase {
    file: "notes.py",
    provider: "rope",
    language: "python",
    extra_args: vec![String::from("offset=4"), String::from("new_name=woven")],
    position: Some("4"),
    new_name: Some("woven"),
})]
#[case(RenameContractCase {
    file: "notes.py",
    provider: "rope",
    language: "python",
    extra_args: vec![
        String::from("uri=stale_value"),
        String::from("offset=4"),
        String::from("new_name=woven"),
    ],
    position: Some("4"),
    new_name: Some("woven"),
})]
#[case(RenameContractCase {
    file: "notes.py",
    provider: "rope",
    language: "python",
    extra_args: vec![String::from("new_name=woven")],
    position: None,
    new_name: Some("woven"),
})]
#[case(RenameContractCase {
    file: "notes.rs",
    provider: "rust-analyzer",
    language: "rust",
    extra_args: vec![String::from("offset=4"), String::from("new_name=woven")],
    position: Some("4"),
    new_name: Some("woven"),
})]
fn handler_rename_contract_parametrised(
    socket_dir: Result<TempDir, String>,
    #[case] case: RenameContractCase,
) -> Result<(), String> {
    let socket_dir = socket_dir?;
    let _ = assert_rename_request(
        RenameDispatch {
            file: case.file,
            provider: case.provider,
            language: case.language,
            extra_args: case.extra_args,
            socket_dir: &socket_dir,
        },
        RenameExpectation {
            position: case.position,
            new_name: case.new_name,
        },
    )?;
    Ok(())
}

#[test]
fn rust_analyzer_manifest_declares_rename_symbol_capability() {
    let manifest = rust_analyzer_manifest(std::path::PathBuf::from(
        "/usr/bin/weaver-plugin-rust-analyzer",
    ));

    assert_eq!(manifest.capabilities(), &[CapabilityId::RenameSymbol]);
}
