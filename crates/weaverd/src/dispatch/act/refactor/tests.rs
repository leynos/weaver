//! Unit tests for the `act refactor` handler.

use std::path::Path;

use rstest::{fixture, rstest};
use tempfile::TempDir;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_plugins::{CapabilityId, PluginError, PluginOutput, PluginRequest, PluginResponse};

use super::{
    DispatchError, FusionBackends, RefactorContext, RefactorPluginRuntime, ResponseWriter,
    default_runtime, handle, resolve_rope_plugin_path, resolve_rust_analyzer_plugin_path,
    rust_analyzer_manifest,
};
use crate::dispatch::request::{CommandDescriptor, CommandRequest};
use crate::semantic_provider::SemanticBackendProvider;

enum MockRuntimeResult {
    Success(PluginResponse),
    NotFound(String),
}

struct MockRuntime {
    result: MockRuntimeResult,
}

impl RefactorPluginRuntime for MockRuntime {
    fn execute(
        &self,
        _provider: &str,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        match &self.result {
            MockRuntimeResult::Success(response) => Ok(response.clone()),
            MockRuntimeResult::NotFound(name) => Err(PluginError::NotFound { name: name.clone() }),
        }
    }
}

fn command_request(arguments: Vec<String>) -> CommandRequest {
    CommandRequest {
        command: CommandDescriptor {
            domain: String::from("act"),
            operation: String::from("refactor"),
        },
        arguments,
        patch: None,
    }
}

#[fixture]
fn socket_dir() -> TempDir {
    TempDir::new().expect("socket dir")
}

fn build_backends(socket_path: &Path) -> FusionBackends<SemanticBackendProvider> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().as_ref()),
        ..Config::default()
    };
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
    FusionBackends::new(config, provider)
}

#[rstest]
fn handle_returns_error_for_missing_provider(socket_dir: TempDir) {
    let request = command_request(vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("notes.txt"),
    ]);
    let socket_path = socket_dir.path().join("socket.sock");
    let mut backends = build_backends(&socket_path);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let runtime = MockRuntime {
        result: MockRuntimeResult::NotFound(String::from("rope")),
    };

    let result = handle(
        &request,
        &mut writer,
        RefactorContext {
            backends: &mut backends,
            workspace_root: Path::new("/tmp/workspace"),
            runtime: &runtime,
        },
    );

    assert!(matches!(
        result,
        Err(DispatchError::InvalidArguments { .. })
    ));
}

#[rstest]
fn handle_runtime_error_returns_status_one(socket_dir: TempDir) {
    let workspace = TempDir::new().expect("workspace");
    let file_path = workspace.path().join("notes.txt");
    std::fs::write(&file_path, "hello\n").expect("write");

    let request = command_request(vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("notes.txt"),
    ]);
    let runtime = MockRuntime {
        result: MockRuntimeResult::NotFound(String::from("rope")),
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
    let file_path = workspace.path().join("notes.txt");
    std::fs::write(&file_path, "hello\n").expect("write");

    let request = command_request(vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("notes.txt"),
    ]);
    let runtime = MockRuntime {
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
        result: MockRuntimeResult::Success(PluginResponse::success(PluginOutput::Diff {
            content: String::from(diff),
        })),
    };
    let request = command_request(vec![
        String::from("--provider"),
        String::from("rope"),
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
    let stdout = String::from_utf8(output).expect("stdout utf8");
    assert!(stdout.contains("\"kind\":\"stream\""));
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

/// Captures the `PluginRequest` sent to the runtime for inspection.
struct InspectingRuntime {
    captured: std::sync::Mutex<Option<PluginRequest>>,
    response: PluginResponse,
}

impl RefactorPluginRuntime for InspectingRuntime {
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

/// Dispatches a rename request through the handler and returns the captured
/// `PluginRequest` for inspection.
fn dispatch_inspecting_rename(
    provider: &str,
    extra_args: Vec<String>,
    socket_dir: &TempDir,
) -> PluginRequest {
    let workspace = TempDir::new().expect("workspace");
    std::fs::write(workspace.path().join("notes.txt"), "hello world\n").expect("write");
    let runtime = InspectingRuntime {
        captured: std::sync::Mutex::new(None),
        response: PluginResponse::success(PluginOutput::Diff {
            content: String::from(NOTES_DIFF),
        }),
    };
    let mut args = vec![
        String::from("--provider"),
        String::from(provider),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("notes.txt"),
    ];
    args.extend(extra_args);
    let request = command_request(args);
    let socket_path = socket_dir.path().join("socket.sock");
    let mut backends = build_backends(&socket_path);
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let _result = handle(
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
    let plugin_request = dispatch_inspecting_rename(
        "rope",
        vec![String::from("offset=4"), String::from("new_name=woven")],
        &socket_dir,
    );

    assert_eq!(plugin_request.operation(), "rename-symbol");
    let args = plugin_request.arguments();
    assert_eq!(
        args.get("uri").and_then(|v| v.as_str()),
        Some("file://notes.txt"),
        "uri should be injected from --file"
    );
    assert_eq!(
        args.get("position").and_then(|v| v.as_str()),
        Some("4"),
        "offset should be mapped to position"
    );
    assert!(
        !args.contains_key("offset"),
        "offset key should be removed after mapping to position"
    );
    assert_eq!(
        args.get("new_name").and_then(|v| v.as_str()),
        Some("woven"),
        "new_name should be forwarded"
    );
}

#[rstest]
fn handler_overwrites_pre_existing_uri_with_file_path(socket_dir: TempDir) {
    let plugin_request = dispatch_inspecting_rename(
        "rope",
        vec![
            String::from("uri=stale_value"),
            String::from("offset=4"),
            String::from("new_name=woven"),
        ],
        &socket_dir,
    );

    assert_eq!(
        plugin_request
            .arguments()
            .get("uri")
            .and_then(|v| v.as_str()),
        Some("file://notes.txt"),
        "uri should be overwritten with --file value, not pre-existing extra"
    );
}

#[rstest]
fn handler_omits_position_when_offset_not_provided(socket_dir: TempDir) {
    let plugin_request =
        dispatch_inspecting_rename("rope", vec![String::from("new_name=woven")], &socket_dir);

    assert!(
        !plugin_request.arguments().contains_key("position"),
        "position should not be injected when offset is absent"
    );
}

#[rstest]
fn rust_analyzer_provider_uses_rename_symbol_contract(socket_dir: TempDir) {
    let plugin_request = dispatch_inspecting_rename(
        "rust-analyzer",
        vec![String::from("offset=4"), String::from("new_name=woven")],
        &socket_dir,
    );

    assert_eq!(plugin_request.operation(), "rename-symbol");
    assert_eq!(
        plugin_request
            .arguments()
            .get("uri")
            .and_then(|value| value.as_str()),
        Some("file://notes.txt"),
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
