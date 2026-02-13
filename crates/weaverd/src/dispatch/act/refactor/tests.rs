//! Unit tests for the `act refactor` handler.

use std::path::Path;

use rstest::rstest;
use tempfile::TempDir;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_plugins::{PluginError, PluginOutput, PluginRequest, PluginResponse};

use super::{
    DispatchError, FusionBackends, RefactorDependencies, RefactorPluginRuntime, ResponseWriter,
    default_runtime, handle, resolve_rope_plugin_path,
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

fn build_backends() -> FusionBackends<SemanticBackendProvider> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix("/tmp/weaver-test/socket.sock"),
        ..Config::default()
    };
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
    FusionBackends::new(config, provider)
}

#[test]
fn handle_returns_error_for_missing_provider() {
    let request = command_request(vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("notes.txt"),
    ]);
    let mut backends = build_backends();
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);
    let runtime = MockRuntime {
        result: MockRuntimeResult::NotFound(String::from("rope")),
    };

    let result = handle(
        &request,
        &mut writer,
        &mut backends,
        RefactorDependencies::new(Path::new("/tmp/workspace"), &runtime),
    );

    assert!(matches!(
        result,
        Err(DispatchError::InvalidArguments { .. })
    ));
}

#[test]
fn handle_runtime_error_returns_status_one() {
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
    let mut backends = build_backends();
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = handle(
        &request,
        &mut writer,
        &mut backends,
        RefactorDependencies::new(workspace.path(), &runtime),
    )
    .expect("dispatch result");

    assert_eq!(result.status, 1);
    let stderr = String::from_utf8(output).expect("stderr utf8");
    assert!(stderr.contains("act refactor failed"));
}

#[rstest]
#[case::analysis(PluginOutput::Analysis { data: serde_json::json!({"k": "v"}) })]
#[case::empty(PluginOutput::Empty)]
fn handle_non_diff_output_returns_status_one(#[case] output_variant: PluginOutput) {
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
    let mut backends = build_backends();
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = handle(
        &request,
        &mut writer,
        &mut backends,
        RefactorDependencies::new(workspace.path(), &runtime),
    )
    .expect("dispatch result");

    assert_eq!(result.status, 1);
    let stderr = String::from_utf8(output).expect("stderr utf8");
    assert!(stderr.contains("did not return diff output"));
}

#[test]
fn handle_diff_output_applies_patch_through_apply_patch_pipeline() {
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
    let mut backends = build_backends();
    let mut output = Vec::new();
    let mut writer = ResponseWriter::new(&mut output);

    let result = handle(
        &request,
        &mut writer,
        &mut backends,
        RefactorDependencies::new(workspace.path(), &runtime),
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
fn default_runtime_returns_shared_trait_object() {
    let runtime = default_runtime();
    let request = PluginRequest::new("rename", Vec::new());
    let result = runtime.execute("rope", &request);
    assert!(result.is_err());
}
