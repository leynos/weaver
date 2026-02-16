//! Behavioural tests for the `act refactor` handler.

use std::path::PathBuf;

use mockall::mock;
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_plugins::{PluginError, PluginOutput, PluginRequest, PluginResponse};

use super::*;

const ORIGINAL_CONTENT: &str = "hello world\n";
const UPDATED_CONTENT: &str = "hello woven\n";
const VALID_DIFF: &str = concat!(
    "diff --git a/notes.txt b/notes.txt\n",
    "<<<<<<< SEARCH\n",
    "hello world\n",
    "=======\n",
    "hello woven\n",
    ">>>>>>> REPLACE\n",
);
const MALFORMED_DIFF: &str = concat!(
    "diff --git a/notes.txt\n",
    "<<<<<<< SEARCH\n",
    "hello world\n",
    "=======\n",
    "hello woven\n",
    ">>>>>>> REPLACE\n",
);

#[derive(Clone, Copy, Default)]
enum RuntimeMode {
    #[default]
    DiffSuccess,
    RuntimeError,
    MalformedDiff,
}

mock! {
    Runtime {}
    impl RefactorPluginRuntime for Runtime {
        fn execute(
            &self,
            provider: &str,
            request: &PluginRequest,
        ) -> Result<PluginResponse, PluginError>;
    }
}

const REQUIRED_FLAGS: &[&str] = &["--provider", "--refactoring", "--file"];

fn request_has_required_flags(request: &CommandRequest) -> bool {
    REQUIRED_FLAGS
        .iter()
        .all(|flag| request.arguments.iter().any(|argument| argument == flag))
}

fn configure_runtime_for_mode(runtime: &mut MockRuntime, mode: RuntimeMode) {
    runtime
        .expect_execute()
        .once()
        .returning(
            move |_provider: &str, _request: &PluginRequest| match mode {
                RuntimeMode::DiffSuccess => Ok(PluginResponse::success(PluginOutput::Diff {
                    content: String::from(VALID_DIFF),
                })),
                RuntimeMode::RuntimeError => Err(PluginError::NotFound {
                    name: String::from("rope"),
                }),
                RuntimeMode::MalformedDiff => Ok(PluginResponse::success(PluginOutput::Diff {
                    content: String::from(MALFORMED_DIFF),
                })),
            },
        );
}

struct RefactorWorld {
    workspace: TempDir,
    socket_dir: TempDir,
    request: CommandRequest,
    runtime_mode: RuntimeMode,
    dispatch_result: Option<Result<i32, DispatchError>>,
    response_stream: String,
}

impl RefactorWorld {
    fn new() -> Self {
        Self {
            workspace: TempDir::new().expect("workspace"),
            socket_dir: TempDir::new().expect("socket dir"),
            request: command_request(vec![
                String::from("--provider"),
                String::from("rope"),
                String::from("--refactoring"),
                String::from("rename"),
                String::from("--file"),
                String::from("notes.txt"),
            ]),
            runtime_mode: RuntimeMode::DiffSuccess,
            dispatch_result: None,
            response_stream: String::new(),
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.workspace.path().join(relative)
    }

    fn write_file(&self, relative: &str, content: &str) {
        std::fs::write(self.path(relative), content).expect("write file");
    }

    fn read_file(&self, relative: &str) -> String {
        std::fs::read_to_string(self.path(relative)).expect("read file")
    }

    fn execute(&mut self) {
        let mut runtime = MockRuntime::new();
        if request_has_required_flags(&self.request) {
            configure_runtime_for_mode(&mut runtime, self.runtime_mode);
        }
        let mut output = Vec::new();
        let mut writer = ResponseWriter::new(&mut output);
        let socket_path = self.socket_dir.path().join("socket.sock");
        let mut backends = build_backends(&socket_path);
        let result = handle(
            &self.request,
            &mut writer,
            RefactorContext {
                backends: &mut backends,
                workspace_root: self.workspace.path(),
                runtime: &runtime,
            },
        )
        .map(|dispatch| dispatch.status);

        self.dispatch_result = Some(result);
        self.response_stream = String::from_utf8(output).expect("response utf8");
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

fn build_backends(socket_path: &std::path::Path) -> FusionBackends<SemanticBackendProvider> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().as_ref()),
        ..Config::default()
    };
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
    FusionBackends::new(config, provider)
}

#[fixture]
fn world() -> RefactorWorld {
    RefactorWorld::new()
}

#[given("a workspace file for refactoring")]
fn given_workspace_file(world: &mut RefactorWorld) {
    world.write_file("notes.txt", ORIGINAL_CONTENT);
}

#[given("a valid act refactor request")]
fn given_valid_request(world: &mut RefactorWorld) {
    world.request = command_request(vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("notes.txt"),
        String::from("offset=1"),
        String::from("new_name=woven"),
    ]);
}

#[given("a refactor request missing provider")]
fn given_missing_provider_request(world: &mut RefactorWorld) {
    world.request = command_request(vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("notes.txt"),
    ]);
}

#[given("a runtime error from the refactor plugin")]
fn given_runtime_error(world: &mut RefactorWorld) {
    world.runtime_mode = RuntimeMode::RuntimeError;
}

#[given("a malformed diff response from the refactor plugin")]
fn given_malformed_diff(world: &mut RefactorWorld) {
    world.runtime_mode = RuntimeMode::MalformedDiff;
}

#[when("the act refactor command executes")]
fn when_refactor_executes(world: &mut RefactorWorld) {
    world.execute();
}

#[then("the refactor command succeeds")]
fn then_refactor_succeeds(world: &mut RefactorWorld) {
    let result = world.dispatch_result.as_ref().expect("result missing");
    let status = result.as_ref().expect("status should be present");
    assert_eq!(*status, 0);
}

#[then("the refactor command fails with status 1")]
fn then_refactor_fails_status_one(world: &mut RefactorWorld) {
    let result = world.dispatch_result.as_ref().expect("result missing");
    let status = result.as_ref().expect("status should be present");
    assert_eq!(*status, 1);
}

#[then("the refactor command is rejected as invalid arguments")]
fn then_refactor_rejected_invalid_arguments(world: &mut RefactorWorld) {
    let result = world.dispatch_result.as_ref().expect("result missing");
    assert!(matches!(
        result,
        Err(DispatchError::InvalidArguments { .. })
    ));
}

#[then("the target file is updated")]
fn then_target_file_updated(world: &mut RefactorWorld) {
    let updated = world.read_file("notes.txt");
    assert_eq!(updated, UPDATED_CONTENT);
}

#[then("the target file is unchanged")]
fn then_target_file_unchanged(world: &mut RefactorWorld) {
    let updated = world.read_file("notes.txt");
    assert_eq!(updated, ORIGINAL_CONTENT);
}

#[then("the stderr stream contains {text}")]
fn then_stderr_contains(world: &mut RefactorWorld, text: String) {
    let needle = text.trim_matches('"');
    assert!(
        world.response_stream.contains(needle),
        "expected response stream to contain '{needle}', got: {}",
        world.response_stream
    );
}

#[scenario(path = "tests/features/refactor.feature")]
fn refactor_behaviour(#[from(world)] _world: RefactorWorld) {}
