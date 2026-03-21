//! Unit tests for the `act refactor` handler.

use rstest::{fixture, rstest};
use tempfile::TempDir;
use weaver_plugins::{PluginError, PluginOutput, PluginRequest, PluginResponse};

#[path = "refactor_helpers.rs"]
mod refactor_helpers;

use refactor_helpers::{build_backends, command_request};
use crate::dispatch::act::refactor::resolution::{
    CandidateEvaluation, CapabilityResolutionDetails, CapabilityResolutionEnvelope,
    ResolutionOutcome, ResolutionRequest, SelectionMode,
};
use crate::dispatch::act::refactor::{
    DispatchError, RefactorContext, RefactorPluginRuntime, ResponseWriter, default_runtime,
    handle, resolve_rope_plugin_path, resolve_rust_analyzer_plugin_path,
};

enum MockRuntimeResult {
    Success(PluginResponse),
    NotFound(String),
    /// Causes execute() to panic if called - use this in tests where execute() must not run.
    Panic,
}

enum MockResolution {
    Success(CapabilityResolutionEnvelope),
    Error(String),
}

struct MockRuntime {
    resolution: MockResolution,
    result: MockRuntimeResult,
}

impl RefactorPluginRuntime for MockRuntime {
    fn resolve(
        &self,
        _request: ResolutionRequest<'_>,
    ) -> Result<CapabilityResolutionEnvelope, PluginError> {
        match &self.resolution {
            MockResolution::Success(envelope) => Ok(envelope.clone()),
            MockResolution::Error(message) => Err(PluginError::Manifest {
                message: message.clone(),
            }),
        }
    }

    fn execute(
        &self,
        _provider: &str,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        match &self.result {
            MockRuntimeResult::Success(response) => Ok(response.clone()),
            MockRuntimeResult::NotFound(name) => Err(PluginError::NotFound { name: name.clone() }),
            MockRuntimeResult::Panic => {
                panic!("MockRuntime::execute() was called when it should not have been")
            }
        }
    }
}

#[fixture]
fn socket_dir() -> TempDir {
    TempDir::new().expect("socket dir")
}

fn run_rename_handle(
    socket_dir: &TempDir,
    file: &str,
    resolution: MockResolution,
    result: MockRuntimeResult,
) -> (i32, String) {
    let workspace = TempDir::new().expect("workspace");
    std::fs::write(workspace.path().join(file), "hello\n").expect("write");

    let request = command_request(vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from(file),
    ]);
    let runtime = MockRuntime { resolution, result };
    let socket_path = socket_dir.path().join("socket.sock");
    let mut backends = build_backends(&socket_path);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let dispatch_result = handle(
        &request,
        &mut writer,
        RefactorContext {
            backends: &mut backends,
            workspace_root: workspace.path(),
            runtime: &runtime,
        },
    )
    .expect("dispatch result");

    let stderr = String::from_utf8(output).expect("stderr utf8");
    (dispatch_result.status, stderr)
}

fn automatic_selection(provider: &str, language: &str) -> CapabilityResolutionEnvelope {
    CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
        capability: weaver_plugins::CapabilityId::RenameSymbol,
        language: Some(String::from(language)),
        requested_provider: None,
        selected_provider: Some(String::from(provider)),
        selection_mode: SelectionMode::Automatic,
        outcome: ResolutionOutcome::Selected,
        refusal_reason: None,
        candidates: vec![CandidateEvaluation {
            provider: String::from(provider),
            accepted: true,
            reason: super::resolution::CandidateReason::MatchedLanguageAndCapability,
        }],
    })
}

#[rstest]
fn handle_runtime_error_returns_status_one(socket_dir: TempDir) {
    let (status, stderr) = run_rename_handle(
        &socket_dir,
        "notes.py",
        MockResolution::Success(automatic_selection("rope", "python")),
        MockRuntimeResult::NotFound(String::from("rope")),
    );

    assert_eq!(status, 1);
    assert!(stderr.contains("CapabilityResolution"));
    assert!(stderr.contains("act refactor failed"));
}

#[rstest]
#[case::analysis(PluginOutput::Analysis { data: serde_json::json!({"k": "v"}) })]
#[case::empty(PluginOutput::Empty)]
fn handle_non_diff_output_returns_status_one(
    #[case] output_variant: PluginOutput,
    socket_dir: TempDir,
) {
    let workspace = TempDir::new().expect("workspace");
    std::fs::write(workspace.path().join("notes.py"), "hello\n").expect("write");

    let request = command_request(vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("notes.py"),
    ]);
    let runtime = MockRuntime {
        resolution: MockResolution::Success(automatic_selection("rope", "python")),
        result: MockRuntimeResult::Success(PluginResponse::success(output_variant)),
    };
    let socket_path = socket_dir.path().join("socket.sock");
    let mut backends = build_backends(&socket_path);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = handle(
        &request,
        &mut writer,
        RefactorContext {
            backends: &mut backends,
            workspace_root: workspace.path(),
            runtime: &runtime,
        },
    )
    .expect("dispatch result");

    assert_eq!(result.status, 1);
    let stderr = String::from_utf8(output).expect("stderr utf8");
    assert!(stderr.contains("did not return diff output"));
}

#[rstest]
fn handle_diff_output_applies_patch_through_apply_patch_pipeline(socket_dir: TempDir) {
    let workspace = TempDir::new().expect("workspace");
    let relative_file = String::from("notes.txt");
    let file_path = workspace.path().join(&relative_file);
    std::fs::write(&file_path, "hello world\n").expect("write");

    let diff = concat!(
        "diff --git a/notes.txt b/notes.txt\n",
        "<<<<<<< SEARCH\n",
        "hello world\n",
        "=======\n",
        "hello woven\n",
        ">>>>>>> REPLACE\n",
    );
    let runtime = MockRuntime {
        resolution: MockResolution::Success(automatic_selection("rope", "python")),
        result: MockRuntimeResult::Success(PluginResponse::success(PluginOutput::Diff {
            content: String::from(diff),
        })),
    };
    let request = command_request(vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        relative_file.clone(),
    ]);
    let socket_path = socket_dir.path().join("socket.sock");
    let mut backends = build_backends(&socket_path);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = handle(
        &request,
        &mut writer,
        RefactorContext {
            backends: &mut backends,
            workspace_root: workspace.path(),
            runtime: &runtime,
        },
    )
    .expect("dispatch result");

    assert_eq!(result.status, 0);
    let updated = std::fs::read_to_string(workspace.path().join(relative_file)).expect("read");
    assert_eq!(updated, "hello woven\n");
    let stderr = String::from_utf8(output).expect("stderr utf8");
    assert!(stderr.contains("CapabilityResolution"));
    assert!(stderr.contains("\"kind\":\"stream\""));
}

#[test]
fn resolve_rope_plugin_path_makes_relative_overrides_absolute() {
    let path = resolve_rope_plugin_path(Some(std::ffi::OsString::from("bin/rope")));
    assert!(path.is_absolute());
}

#[test]
fn resolve_rust_analyzer_plugin_path_makes_relative_overrides_absolute() {
    let path = resolve_rust_analyzer_plugin_path(Some(std::ffi::OsString::from(
        "bin/rust-analyzer-plugin",
    )));
    assert!(path.is_absolute());
}

#[test]
fn default_runtime_returns_shared_trait_object() {
    let runtime = default_runtime();
    let request = PluginRequest::new("rename", Vec::new());
    let result = runtime.execute("rope", &request);
    assert!(result.is_err());
}

#[rstest]
fn handle_returns_error_for_unsupported_refactoring(socket_dir: TempDir) {
    let workspace = TempDir::new().expect("workspace");
    let request = command_request(vec![
        String::from("--refactoring"),
        String::from("extract-method"),
        String::from("--file"),
        String::from("notes.py"),
    ]);
    let socket_path = socket_dir.path().join("socket.sock");
    let mut backends = build_backends(&socket_path);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let runtime = MockRuntime {
        resolution: MockResolution::Success(automatic_selection("rope", "python")),
        result: MockRuntimeResult::Panic,
    };

    let result = handle(
        &request,
        &mut writer,
        RefactorContext {
            backends: &mut backends,
            workspace_root: workspace.path(),
            runtime: &runtime,
        },
    );

    assert!(matches!(
        result,
        Err(DispatchError::InvalidArguments { .. })
    ));
}

#[rstest]
fn handle_exits_with_error_when_resolution_fails(socket_dir: TempDir) {
    let (status, stderr) = run_rename_handle(
        &socket_dir,
        "notes.py",
        MockResolution::Error(String::from("bad manifest")),
        MockRuntimeResult::Panic,
    );

    assert_eq!(status, 1);
    assert!(
        stderr.contains("act refactor failed"),
        "stderr should contain the generic failure message, got: {stderr}"
    );
}

#[rstest]
fn handle_exits_with_error_when_resolution_refused_without_provider(socket_dir: TempDir) {
    use crate::dispatch::act::refactor::resolution::RefusalReason::UnsupportedLanguage;

    let refused_envelope =
        CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
            capability: weaver_plugins::CapabilityId::RenameSymbol,
            language: Some(String::from("unknown-lang")),
            requested_provider: None,
            selected_provider: None,
            selection_mode: SelectionMode::Automatic,
            outcome: ResolutionOutcome::Refused,
            refusal_reason: Some(UnsupportedLanguage),
            candidates: Vec::new(),
        });

    let (status, stderr) = run_rename_handle(
        &socket_dir,
        "notes.txt",
        MockResolution::Success(refused_envelope),
        MockRuntimeResult::Panic,
    );

    assert_eq!(status, 1);
    assert!(
        stderr.contains("CapabilityResolution"),
        "stderr should contain the capability resolution envelope, got: {stderr}"
    );
    assert!(
        stderr.contains("refused"),
        "stderr should indicate refusal, got: {stderr}"
    );
}
