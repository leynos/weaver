//! Behavioural tests for the `act refactor` handler.

use std::path::{Path, PathBuf};

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_plugins::{CapabilityId, PluginError, PluginOutput, PluginRequest, PluginResponse};

use super::resolution::{
    CandidateEvaluation, CandidateReason, CapabilityResolutionDetails,
    CapabilityResolutionEnvelope, RefusalReason, ResolutionOutcome, ResolutionRequest,
    SelectionMode,
};
use super::*;

#[derive(Clone, Copy, Default)]
enum RuntimeMode {
    #[default]
    DiffSuccess,
    RuntimeError,
    MalformedDiff,
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

struct RefusedResolution<'a> {
    language: Option<&'a str>,
    requested_provider: Option<&'a str>,
    selection_mode: SelectionMode,
    refusal_reason: RefusalReason,
    candidates: Vec<CandidateEvaluation>,
}

impl RefactorPluginRuntime for StubRuntime {
    fn resolve(
        &self,
        _request: ResolutionRequest<'_>,
    ) -> Result<CapabilityResolutionEnvelope, PluginError> {
        Ok(match self.routing {
            RoutingMode::AutomaticPython => selected_resolution("python", "rope"),
            RoutingMode::AutomaticRust => selected_resolution("rust", "rust-analyzer"),
            RoutingMode::UnsupportedLanguage => refused_resolution(RefusedResolution {
                language: None,
                requested_provider: None,
                selection_mode: SelectionMode::Automatic,
                refusal_reason: RefusalReason::UnsupportedLanguage,
                candidates: vec![
                    rejected_candidate("rope", CandidateReason::UnsupportedLanguage),
                    rejected_candidate("rust-analyzer", CandidateReason::UnsupportedLanguage),
                ],
            }),
            RoutingMode::ExplicitProviderMismatch => refused_resolution(RefusedResolution {
                language: Some("python"),
                requested_provider: Some("rust-analyzer"),
                selection_mode: SelectionMode::ExplicitProvider,
                refusal_reason: RefusalReason::ExplicitProviderMismatch,
                candidates: vec![
                    rejected_candidate("rope", CandidateReason::NotRequested),
                    rejected_candidate("rust-analyzer", CandidateReason::ExplicitProviderMismatch),
                ],
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
                content: diff_for(&relative_path),
            })),
            RuntimeMode::RuntimeError => Err(PluginError::NotFound {
                name: String::from("rope"),
            }),
            RuntimeMode::MalformedDiff => Ok(PluginResponse::success(PluginOutput::Diff {
                content: malformed_diff_for(&relative_path),
            })),
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

fn build_backends(socket_path: &Path) -> FusionBackends<SemanticBackendProvider> {
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path.to_string_lossy().as_ref()),
        ..Config::default()
    };
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
    FusionBackends::new(config, provider)
}

fn standard_rename_args(file: &str) -> Vec<String> {
    vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from(file),
        String::from("offset=1"),
        String::from("new_name=woven"),
    ]
}

fn configure_request(world: &mut RefactorWorld, args: Vec<String>, routing_mode: RoutingMode) {
    world.request = command_request(args);
    world.routing_mode = routing_mode;
}

fn selected_resolution(language: &str, provider: &str) -> CapabilityResolutionEnvelope {
    CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
        capability: CapabilityId::RenameSymbol,
        language: Some(String::from(language)),
        requested_provider: None,
        selected_provider: Some(String::from(provider)),
        selection_mode: SelectionMode::Automatic,
        outcome: ResolutionOutcome::Selected,
        refusal_reason: None,
        candidates: vec![CandidateEvaluation {
            provider: String::from(provider),
            accepted: true,
            reason: CandidateReason::MatchedLanguageAndCapability,
        }],
    })
}

fn refused_resolution(config: RefusedResolution<'_>) -> CapabilityResolutionEnvelope {
    CapabilityResolutionEnvelope::from_details(CapabilityResolutionDetails {
        capability: CapabilityId::RenameSymbol,
        language: config.language.map(String::from),
        requested_provider: config.requested_provider.map(String::from),
        selected_provider: None,
        selection_mode: config.selection_mode,
        outcome: ResolutionOutcome::Refused,
        refusal_reason: Some(config.refusal_reason),
        candidates: config.candidates,
    })
}

fn rejected_candidate(provider: &str, reason: CandidateReason) -> CandidateEvaluation {
    CandidateEvaluation {
        provider: String::from(provider),
        accepted: false,
        reason,
    }
}

fn format_diff(relative_path: &str, git_header: &str) -> String {
    let original = original_content_for(relative_path);
    let updated = updated_content_for(relative_path);
    format!("{git_header}\n<<<<<<< SEARCH\n{original}=======\n{updated}>>>>>>> REPLACE\n",)
}

fn diff_for(relative_path: &str) -> String {
    format_diff(
        relative_path,
        &format!("diff --git a/{0} b/{0}", relative_path),
    )
}

fn malformed_diff_for(relative_path: &str) -> String {
    format_diff(relative_path, &format!("diff --git a/{0}", relative_path))
}

fn original_content_for(relative_path: &str) -> &'static str {
    if relative_path.ends_with(".py") {
        "old_name = 1\nprint(old_name)\n"
    } else if relative_path.ends_with(".rs") {
        concat!(
            "fn main() {\n",
            "    let old_name = 1;\n",
            "    println!(\"{}\", old_name);\n",
            "}\n",
        )
    } else {
        "hello world\n"
    }
}

fn updated_content_for(relative_path: &str) -> &'static str {
    if relative_path.ends_with(".py") {
        "new_name = 1\nprint(new_name)\n"
    } else if relative_path.ends_with(".rs") {
        concat!(
            "fn main() {\n",
            "    let new_name = 1;\n",
            "    println!(\"{}\", new_name);\n",
            "}\n",
        )
    } else {
        "hello woven\n"
    }
}

#[fixture]
fn world() -> RefactorWorld {
    RefactorWorld::new()
}

#[given("a workspace file for refactoring")]
fn given_workspace_file(world: &mut RefactorWorld) {
    let target_file = world.target_file();
    world.write_file(&target_file, original_content_for(&target_file));
}

#[given("a valid auto-routed act refactor request resolved to rope")]
fn given_valid_rope_request(world: &mut RefactorWorld) {
    configure_request(
        world,
        standard_rename_args("notes.txt"),
        RoutingMode::AutomaticPython,
    );
}

#[given("a valid auto-routed act refactor request resolved to rust-analyzer")]
fn given_valid_rust_request(world: &mut RefactorWorld) {
    configure_request(
        world,
        standard_rename_args("notes.txt"),
        RoutingMode::AutomaticRust,
    );
}

#[given("an unsupported-language act refactor request")]
fn given_unsupported_language_request(world: &mut RefactorWorld) {
    configure_request(
        world,
        standard_rename_args("notes.txt"),
        RoutingMode::UnsupportedLanguage,
    );
}

#[given("a Python act refactor request with an incompatible provider override")]
fn given_explicit_provider_mismatch_request(world: &mut RefactorWorld) {
    let mut args = vec![String::from("--provider"), String::from("rust-analyzer")];
    args.extend(standard_rename_args("notes.py"));
    configure_request(world, args, RoutingMode::ExplicitProviderMismatch);
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

#[then("the target file is updated")]
fn then_target_file_updated(world: &mut RefactorWorld) {
    let target_file = world.target_file();
    let updated = world.read_file(&target_file);
    assert_eq!(updated, updated_content_for(&target_file));
}

#[then("the target file is unchanged")]
fn then_target_file_unchanged(world: &mut RefactorWorld) {
    let target_file = world.target_file();
    let updated = world.read_file(&target_file);
    assert_eq!(updated, original_content_for(&target_file));
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
