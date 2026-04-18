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
    ResolutionRequest,
    RefusalReason,
    SelectionMode,
    CandidateReason,
    CapabilityResolutionEnvelope,
    resolution::{,
    CandidateEvaluation,
};

#[derive(Clone, Copy, Default)]
use weaver_test_macros::allow_fixture_expansion_lints;
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

fn provider_for_auto(mode: RoutingMode) -> &'static str {
    match mode {
        RoutingMode::AutomaticPython => "rope",
        RoutingMode::AutomaticRust => "rust-analyzer",
        _ => unreachable!("provider_for_auto is only for automatic modes"),
    }
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
        let auto_context = AutoResolutionContext {
            capability: request.capability(),
            requested_provider,
            selection_mode,
        };

        Ok(match self.routing {
            mode @ (RoutingMode::AutomaticPython | RoutingMode::AutomaticRust) => {
                resolve_auto_language(auto_context, language_name, provider_for_auto(mode))
            }
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
        let file_payload = request
            .files()
            .first()
            .ok_or_else(|| PluginError::NotFound {
                name: String::from("file payload"),
            })?;

        match self.execution {
            RuntimeMode::DiffSuccess => Ok(PluginResponse::success(PluginOutput::Diff {
                content: routed_diff_for(file_payload.path()),
            })),
            RuntimeMode::RuntimeError => Err(PluginError::NotFound {
                name: String::from("rope"),
            }),
            RuntimeMode::MalformedDiff => Ok(PluginResponse::success(PluginOutput::Diff {
                content: routed_malformed_diff_for(file_payload.path()),
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
    fn new() -> Result<Self, String> {
        Ok(Self {
            workspace: TempDir::new().map_err(|e| format!("workspace: {e}"))?,
            socket_dir: TempDir::new().map_err(|e| format!("socket dir: {e}"))?,
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
        std::fs::write(self.path(relative), content).map_err(|e| format!("write file: {e}"))
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
        std::fs::read_to_string(self.path(relative)).map_err(|e| format!("read file: {e}"))
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

#[allow_fixture_expansion_lints]
#[fixture]
fn world() -> RefactorWorld {
    match RefactorWorld::new() {
        Ok(world) => world,
        Err(e) => panic!("failed to create refactor world: {}", e),
    }
}

#[given("a workspace file for refactoring")]
fn given_workspace_file(
    #[expect(
        unused_variables,
        reason = "BDD step exists for readability; file creation happens in \
                  prepare_routed_fixture()"
    )]
    world: &mut RefactorWorld,
) {
    // File creation is handled by prepare_routed_fixture() in subsequent steps
    // This step exists for BDD readability but performs no action
}

#[given("a valid auto-routed act refactor request resolved to rope")]
fn given_valid_rope_request(world: &mut RefactorWorld) -> Result<(), String> {
    configure_request(&mut world.request, standard_rename_args("notes.py"));
    world.routing_mode = RoutingMode::AutomaticPython;
    world.prepare_routed_fixture("notes.py")
}

#[given("a valid auto-routed act refactor request resolved to rust-analyzer")]
fn given_valid_rust_request(world: &mut RefactorWorld) -> Result<(), String> {
    configure_request(&mut world.request, standard_rename_args("notes.rs"));
    world.routing_mode = RoutingMode::AutomaticRust;
    world.prepare_routed_fixture("notes.rs")
}

#[given("an unsupported-language act refactor request")]
fn given_unsupported_language_request(world: &mut RefactorWorld) {
    configure_request(&mut world.request, standard_rename_args("notes.txt"));
    world.routing_mode = RoutingMode::UnsupportedLanguage;
}

#[given("a Python act refactor request with an incompatible provider override")]
fn given_explicit_provider_mismatch_request(world: &mut RefactorWorld) -> Result<(), String> {
    let mut args = vec![String::from("--provider"), String::from("rust-analyzer")];
    args.extend(standard_rename_args("notes.py"));
    configure_request(&mut world.request, args);
    world.routing_mode = RoutingMode::ExplicitProviderMismatch;
    world.prepare_routed_fixture("notes.py")
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
fn when_refactor_executes(world: &mut RefactorWorld) -> Result<(), String> { world.execute() }

fn extract_status(world: &RefactorWorld) -> Result<i32, String> {
    let result = world.dispatch_result.as_ref().ok_or("result missing")?;
    result
        .as_ref()
        .map(|status| *status)
        .map_err(|e| format!("status error: {e}"))
}
fn then_refactor_succeeds(world: &mut RefactorWorld) -> Result<(), String> {
    assert_eq!(extract_status(world)?, 0);
    Ok(())
}

#[then("the refactor command fails with status 1")]
fn then_refactor_fails_status_one(world: &mut RefactorWorld) -> Result<(), String> {
    assert_eq!(extract_status(world)?, 1);
    Ok(())
}

#[then("the target file is updated")]
fn then_target_file_updated(world: &mut RefactorWorld) -> Result<(), String> {
    let target_file = world.target_file()?;
    assert_eq!(
        read_routed_target(world)?,
        updated_content_for(Path::new(&target_file))
    );
    Ok(())
}

#[then("the target file is unchanged")]
fn then_target_file_unchanged(world: &mut RefactorWorld) -> Result<(), String> {
    let target_file = world.target_file()?;
    assert_eq!(
        read_routed_target(world)?,
        original_content_for(Path::new(&target_file))
    );
    Ok(())
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
fn refactor_behaviour(#[from(world)] world: RefactorWorld) {
    let _ = world;
}

struct AutoResolutionContext<'a> {
    capability: weaver_plugins::CapabilityId,
    requested_provider: Option<&'a str>,
    selection_mode: SelectionMode,
}

fn resolve_auto_language(
    context: AutoResolutionContext<'_>,
    language_name: Option<&'static str>,
    provider: &'static str,
) -> CapabilityResolutionEnvelope {
    if let Some(language) = language_name {
        selected_resolution(SelectedResolution {
            capability: context.capability,
            language,
            provider,
            selection_mode: context.selection_mode,
            requested_provider: context.requested_provider,
        })
    } else {
        refused_resolution(RefusedResolution {
            capability: context.capability,
            language: None,
            requested_provider: context.requested_provider,
            selection_mode: context.selection_mode,
            refusal_reason: RefusalReason::UnsupportedLanguage,
            candidates: refused_candidates(
                context.requested_provider,
                CandidateReason::UnsupportedLanguage,
            ),
        })
    }
}

fn read_routed_target(world: &RefactorWorld) -> Result<String, String> {
    let target_file = world.target_file()?;
    let patch_target = routed_patch_path(Path::new(&target_file));
    let path_str = patch_target.to_str().ok_or("invalid UTF-8 path")?;
    world.read_file(path_str)
}
