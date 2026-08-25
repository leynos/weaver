//! Behavioural tests for the `act refactor` handler.

use std::path::{Path, PathBuf};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use weaver_test_macros::allow_fixture_expansion_lints;

use super::{
    refactor_helpers::{
        builders::{
            build_backends,
            command_request,
            configure_request,
            standard_rename_args_for_provider,
        },
        content::{original_content_for, routed_patch_path, updated_content_for},
        stub_runtime::{RoutingMode, RuntimeMode, StubRuntime},
    },
    *,
};
use crate::tests::support::fs as test_fs;

struct RefactorWorld {
    workspace: TempDir,
    socket_dir: TempDir,
    request: CommandRequest,
    runtime_mode: RuntimeMode,
    routing_mode: RoutingMode,
    dispatch_result: Option<Result<i32, DispatchError>>,
    response_stream: String,
}

impl RefactorWorld {
    fn new() -> Result<Self, String> {
        Ok(Self {
            workspace: TempDir::new().map_err(|e| format!("workspace: {e}"))?,
            socket_dir: TempDir::new().map_err(|e| format!("socket dir: {e}"))?,
            request: command_request(vec![
                String::from("--provider"),
                String::from("rope"),
                String::from("--refactoring"),
                String::from("rename"),
                String::from("--file"),
                String::from("notes.txt"),
            ]),
            runtime_mode: RuntimeMode::DiffSuccess,
            routing_mode: RoutingMode::AutomaticPython,
            dispatch_result: None,
            response_stream: String::new(),
        })
    }

    fn path(&self, relative: &str) -> PathBuf { self.workspace.path().join(relative) }

    fn target_file(&self) -> Result<String, String> {
        self.request
            .arguments
            .windows(2)
            .find_map(|pair| (pair[0] == "--file").then(|| pair[1].clone()))
            .ok_or_else(|| "target file argument missing".to_string())
    }

    fn write_file(&self, relative: &str, content: &str) -> Result<(), String> {
        test_fs::write(self.path(relative), content).map_err(|e| format!("write file: {e}"))
    }

    fn prepare_routed_fixture(&self, target_file: &str) -> Result<(), String> {
        let target_path = Path::new(target_file);
        self.write_file(target_file, original_content_for(target_path))?;
        let patch_path = routed_patch_path(target_path);
        if patch_path != target_path {
            let path_str = patch_path.to_str().ok_or("invalid UTF-8 path")?;
            self.write_file(path_str, original_content_for(target_path))?;
        }
        Ok(())
    }

    fn read_file(&self, relative: &str) -> Result<String, String> {
        test_fs::read_to_string(self.path(relative)).map_err(|e| format!("read file: {e}"))
    }

    fn execute(&mut self) -> Result<(), String> {
        let runtime = StubRuntime {
            routing: self.routing_mode,
            execution: self.runtime_mode,
        };
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
        self.response_stream =
            String::from_utf8(output).map_err(|e| format!("response utf8: {e}"))?;
        Ok(())
    }
}

/// Fixture payload: world construction can fail, so steps unwrap it themselves.
type RefactorWorldFixture = Result<RefactorWorld, String>;

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> RefactorWorldFixture { RefactorWorld::new() }

/// Borrows the world, surfacing fixture construction failure as a step failure.
fn refactor_world(world: &mut RefactorWorldFixture) -> Result<&mut RefactorWorld, String> {
    world.as_mut().map_err(|error| error.clone())
}

/// Configures the request and routing mode, then lays down the routed fixture.
fn arrange_rename_request(
    world: &mut RefactorWorldFixture,
    file: &str,
    provider: &str,
    routing_mode: RoutingMode,
) -> Result<(), String> {
    let state = refactor_world(world)?;
    configure_request(
        &mut state.request,
        standard_rename_args_for_provider(file, provider),
    );
    state.routing_mode = routing_mode;
    state.prepare_routed_fixture(file)
}

#[given("a workspace file for refactoring")]
fn given_workspace_file(world: &mut RefactorWorldFixture) -> Result<(), String> {
    refactor_world(world).map(|_| ())
}

#[given("a valid act refactor request for rope")]
fn given_valid_rope_request(world: &mut RefactorWorldFixture) -> Result<(), String> {
    arrange_rename_request(world, "notes.py", "rope", RoutingMode::AutomaticPython)
}

#[given("a valid act refactor request for rust-analyzer")]
fn given_valid_rust_request(world: &mut RefactorWorldFixture) -> Result<(), String> {
    arrange_rename_request(
        world,
        "notes.rs",
        "rust-analyzer",
        RoutingMode::AutomaticRust,
    )
}

#[given("an unsupported-language act refactor request")]
fn given_unsupported_language_request(world: &mut RefactorWorldFixture) -> Result<(), String> {
    let state = refactor_world(world)?;
    configure_request(
        &mut state.request,
        standard_rename_args_for_provider("notes.txt", "rope"),
    );
    state.routing_mode = RoutingMode::UnsupportedLanguage;
    Ok(())
}

#[given("a Python act refactor request with an incompatible provider override")]
fn given_explicit_provider_mismatch_request(
    world: &mut RefactorWorldFixture,
) -> Result<(), String> {
    arrange_rename_request(
        world,
        "notes.py",
        "rust-analyzer",
        RoutingMode::ExplicitProviderMismatch,
    )
}

/// Selects the stub plugin response the next execution should produce.
fn arrange_runtime_mode(
    world: &mut RefactorWorldFixture,
    runtime_mode: RuntimeMode,
) -> Result<(), String> {
    refactor_world(world)?.runtime_mode = runtime_mode;
    Ok(())
}

#[given("a runtime error from the refactor plugin")]
fn given_runtime_error(world: &mut RefactorWorldFixture) -> Result<(), String> {
    arrange_runtime_mode(world, RuntimeMode::RuntimeError)
}

#[given("a malformed diff response from the refactor plugin")]
fn given_malformed_diff(world: &mut RefactorWorldFixture) -> Result<(), String> {
    arrange_runtime_mode(world, RuntimeMode::MalformedDiff)
}

#[given("a non-diff success response from the refactor plugin")]
fn given_non_diff_success(world: &mut RefactorWorldFixture) -> Result<(), String> {
    arrange_runtime_mode(world, RuntimeMode::EmptySuccess)
}

#[when("the act refactor command executes")]
fn when_refactor_executes(world: &mut RefactorWorldFixture) -> Result<(), String> {
    refactor_world(world)?.execute()
}

fn extract_status(world: &RefactorWorld) -> Result<i32, String> {
    let result = world.dispatch_result.as_ref().ok_or("result missing")?;
    result
        .as_ref()
        .map(|status| *status)
        .map_err(|e| format!("status error: {e}"))
}

#[then("the refactor command succeeds")]
fn then_refactor_succeeds(world: &mut RefactorWorldFixture) -> Result<(), String> {
    assert_eq!(extract_status(refactor_world(world)?)?, 0);
    Ok(())
}

#[then("the refactor command fails with status 1")]
fn then_refactor_fails_status_one(world: &mut RefactorWorldFixture) -> Result<(), String> {
    let state = refactor_world(world)?;
    let result = state.dispatch_result.as_ref().ok_or("result missing")?;
    match result {
        Ok(status) => assert_eq!(*status, 1),
        Err(error) => assert_eq!(error.exit_status(), 1),
    }
    Ok(())
}

#[then("the target file is updated")]
fn then_target_file_updated(world: &mut RefactorWorldFixture) -> Result<(), String> {
    let state = refactor_world(world)?;
    let target_file = state.target_file()?;
    assert_eq!(
        read_routed_target(state)?,
        updated_content_for(Path::new(&target_file))
    );
    Ok(())
}

#[then("the target file is unchanged")]
fn then_target_file_unchanged(world: &mut RefactorWorldFixture) -> Result<(), String> {
    let state = refactor_world(world)?;
    let target_file = state.target_file()?;
    assert_eq!(
        read_routed_target(state)?,
        original_content_for(Path::new(&target_file))
    );
    Ok(())
}

#[then("the stderr stream contains {text}")]
fn then_stderr_contains(world: &mut RefactorWorldFixture, text: String) -> Result<(), String> {
    let state = refactor_world(world)?;
    let needle = text.trim_matches('"');
    assert!(
        state.response_stream.contains(needle),
        "expected response stream to contain '{needle}', got: {}",
        state.response_stream
    );
    Ok(())
}

#[then("the dispatch error contains {text}")]
fn then_dispatch_error_contains(
    world: &mut RefactorWorldFixture,
    text: String,
) -> Result<(), String> {
    let needle = text.trim_matches('"');
    let state = refactor_world(world)?;
    let result = state.dispatch_result.as_ref().ok_or("result missing")?;
    let error = match result {
        Err(error) => error,
        Ok(status) => return Err(format!("expected dispatch error, got status: {status:?}")),
    };
    let rendered = error.to_string();
    if !rendered.contains(needle) {
        return Err(format!(
            "expected dispatch error to contain '{needle}', got: {rendered}"
        ));
    }
    Ok(())
}

#[scenario(path = "tests/features/refactor.feature")]
fn refactor_behaviour(#[from(world)] _: RefactorWorldFixture) {}

fn read_routed_target(world: &RefactorWorld) -> Result<String, String> {
    let target_file = world.target_file()?;
    let patch_target = routed_patch_path(Path::new(&target_file));
    let path_str = patch_target.to_str().ok_or("invalid UTF-8 path")?;
    world.read_file(path_str)
}
