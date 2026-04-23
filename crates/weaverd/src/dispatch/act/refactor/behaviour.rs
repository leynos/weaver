//! Behavioural tests for the `act refactor` handler.

use std::path::{Path, PathBuf};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use weaver_plugins::{PluginError, PluginOutput, PluginRequest, PluginResponse};
use weaver_syntax::SupportedLanguage;

use super::refactor_helpers::*;
use super::resolution::{
    CandidateEvaluation, CandidateReason, CapabilityResolutionEnvelope, RefusalReason,
    ResolutionRequest, SelectionMode,
};
use super::*;

#[derive(Clone, Copy, Default)]
enum RuntimeMode {
    #[default]
    DiffSuccess,
    RuntimeError,
    MalformedDiff,
    EmptySuccess,
}

#[derive(Clone, Copy, Default)]
enum RoutingMode {
    #[default]
    AutomaticPython,
    AutomaticRust,
    UnsupportedLanguage,
    ExplicitProviderMismatch,
}

struct StubRuntime {
    routing: RoutingMode,
    execution: RuntimeMode,
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

        // Automatic routing modes require a valid language
        let requires_language = matches!(
            self.routing,
            RoutingMode::AutomaticPython | RoutingMode::AutomaticRust
        );
        if requires_language && language_name.is_none() {
            return Ok(refused_resolution(RefusedResolution {
                capability: request.capability(),
                language: None,
                requested_provider,
                selection_mode,
                refusal_reason: RefusalReason::UnsupportedLanguage,
                candidates: refused_candidates(
                    requested_provider,
                    CandidateReason::UnsupportedLanguage,
                ),
            }));
        }

        Ok(match self.routing {
            RoutingMode::AutomaticPython => selected_resolution(SelectedResolution {
                capability: request.capability(),
                language: language_name.expect("language validated above"),
                provider: "rope",
                selection_mode,
                requested_provider,
            }),
            RoutingMode::AutomaticRust => selected_resolution(SelectedResolution {
                capability: request.capability(),
                language: language_name.expect("language validated above"),
                provider: "rust-analyzer",
                selection_mode,
                requested_provider,
            }),
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
        let relative_path = request
            .files()
            .first()
            .expect("file payload")
            .path()
            .to_string_lossy()
            .into_owned();

        match self.execution {
            RuntimeMode::DiffSuccess => Ok(PluginResponse::success(PluginOutput::Diff {
                content: routed_diff_for(Path::new(&relative_path)),
            })),
            RuntimeMode::RuntimeError => Err(PluginError::NotFound {
                name: String::from("rope"),
            }),
            RuntimeMode::MalformedDiff => Ok(PluginResponse::success(PluginOutput::Diff {
                content: routed_malformed_diff_for(Path::new(&relative_path)),
            })),
            RuntimeMode::EmptySuccess => Ok(PluginResponse::success(PluginOutput::Empty)),
        }
    }
}

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
            routing_mode: RoutingMode::AutomaticPython,
            dispatch_result: None,
            response_stream: String::new(),
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.workspace.path().join(relative)
    }

    fn target_file(&self) -> String {
        self.request
            .arguments
            .windows(2)
            .find_map(|pair| (pair[0] == "--file").then(|| pair[1].clone()))
            .expect("target file argument")
    }

    fn write_file(&self, relative: &str, content: &str) {
        std::fs::write(self.path(relative), content).expect("write file");
    }

    fn prepare_routed_fixture(&self, target_file: &str) {
        let target_path = Path::new(target_file);
        self.write_file(target_file, original_content_for(target_path));
        let patch_path = routed_patch_path(target_path);
        if patch_path != target_path {
            self.write_file(
                patch_path.to_str().expect("valid UTF-8 path"),
                original_content_for(target_path),
            );
        }
    }

    fn read_file(&self, relative: &str) -> String {
        std::fs::read_to_string(self.path(relative)).expect("read file")
    }

    fn execute(&mut self) {
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
        self.response_stream = String::from_utf8(output).expect("response utf8");
    }
}

#[fixture]
fn world() -> RefactorWorld {
    RefactorWorld::new()
}

#[given("a workspace file for refactoring")]
fn given_workspace_file(
    #[expect(
        unused_variables,
        reason = "BDD step exists for readability; file creation happens in prepare_routed_fixture()"
    )]
    world: &mut RefactorWorld,
) {
    // File creation is handled by prepare_routed_fixture() in subsequent steps
    // This step exists for BDD readability but performs no action
}

#[given("a valid act refactor request for rope")]
fn given_valid_rope_request(world: &mut RefactorWorld) {
    configure_request(
        &mut world.request,
        standard_rename_args_for_provider("notes.py", "rope"),
    );
    world.routing_mode = RoutingMode::AutomaticPython;
    world.prepare_routed_fixture("notes.py");
}

#[given("a valid act refactor request for rust-analyzer")]
fn given_valid_rust_request(world: &mut RefactorWorld) {
    configure_request(
        &mut world.request,
        standard_rename_args_for_provider("notes.rs", "rust-analyzer"),
    );
    world.routing_mode = RoutingMode::AutomaticRust;
    world.prepare_routed_fixture("notes.rs");
}

#[given("an unsupported-language act refactor request")]
fn given_unsupported_language_request(world: &mut RefactorWorld) {
    configure_request(
        &mut world.request,
        standard_rename_args_for_provider("notes.txt", "rope"),
    );
    world.routing_mode = RoutingMode::UnsupportedLanguage;
}

#[given("a Python act refactor request with an incompatible provider override")]
fn given_explicit_provider_mismatch_request(world: &mut RefactorWorld) {
    configure_request(
        &mut world.request,
        vec![
            String::from("--provider"),
            String::from("rust-analyzer"),
            String::from("--refactoring"),
            String::from("rename"),
            String::from("--file"),
            String::from("notes.py"),
            String::from("offset=1"),
            String::from("new_name=woven"),
        ],
    );
    world.routing_mode = RoutingMode::ExplicitProviderMismatch;
    world.prepare_routed_fixture("notes.py");
}

#[given("a runtime error from the refactor plugin")]
fn given_runtime_error(world: &mut RefactorWorld) {
    world.runtime_mode = RuntimeMode::RuntimeError;
}

#[given("a malformed diff response from the refactor plugin")]
fn given_malformed_diff(world: &mut RefactorWorld) {
    world.runtime_mode = RuntimeMode::MalformedDiff;
}

#[given("a non-diff success response from the refactor plugin")]
fn given_non_diff_success(world: &mut RefactorWorld) {
    world.runtime_mode = RuntimeMode::EmptySuccess;
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
    match result {
        Ok(status) => assert_eq!(*status, 1),
        Err(error) => assert_eq!(error.exit_status(), 1),
    }
}

#[then("the target file is updated")]
fn then_target_file_updated(world: &mut RefactorWorld) {
    let target_file = world.target_file();
    let target_path = Path::new(&target_file);
    let patch_target = routed_patch_path(target_path);
    let updated = world.read_file(patch_target.to_str().expect("valid UTF-8 path"));
    assert_eq!(updated, updated_content_for(target_path));
}

#[then("the target file is unchanged")]
fn then_target_file_unchanged(world: &mut RefactorWorld) {
    let target_file = world.target_file();
    let target_path = Path::new(&target_file);
    let patch_target = routed_patch_path(target_path);
    let updated = world.read_file(patch_target.to_str().expect("valid UTF-8 path"));
    assert_eq!(updated, original_content_for(target_path));
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

#[then("the dispatch error contains {text}")]
fn then_dispatch_error_contains(world: &mut RefactorWorld, text: String) {
    let needle = text.trim_matches('"');
    let result = world.dispatch_result.as_ref().expect("result missing");
    let Err(error) = result else {
        panic!("expected dispatch error, got status: {result:?}");
    };
    let rendered = error.to_string();
    assert!(
        rendered.contains(needle),
        "expected dispatch error to contain '{needle}', got: {rendered}"
    );
}

#[scenario(path = "tests/features/refactor.feature")]
fn refactor_behaviour(#[from(world)] world: RefactorWorld) {
    let _ = world;
}
