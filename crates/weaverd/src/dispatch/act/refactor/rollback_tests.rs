//! Rollback-oriented tests for `act refactor` failure paths.

use tempfile::TempDir;
use weaver_plugins::{PluginOutput, PluginResponse};

use super::refactor_helpers::{
    ExecuteResult, RefusedResolution, RollbackRuntime, SelectedResolution, build_backends,
    command_request, original_content_for, refused_resolution, rollback_runtime, selected_runtime,
    standard_rename_args_for_provider,
};
use crate::dispatch::act::refactor::{RefactorContext, ResponseWriter, handle};

struct RollbackOutcome {
    status: i32,
    stderr: String,
    content: String,
}

fn refused_runtime() -> RollbackRuntime {
    rollback_runtime(
        refused_resolution(RefusedResolution {
            capability: weaver_plugins::CapabilityId::RenameSymbol,
            language: Some("python"),
            requested_provider: Some("rope"),
            selection_mode: super::resolution::SelectionMode::ExplicitProvider,
            refusal_reason: super::resolution::RefusalReason::UnsupportedLanguage,
            candidates: Vec::new(),
        }),
        ExecuteResult::Success(PluginResponse::success(PluginOutput::Empty)),
    )
}

fn rope_python_runtime(execute_result: ExecuteResult) -> RollbackRuntime {
    selected_runtime(
        SelectedResolution {
            capability: weaver_plugins::CapabilityId::RenameSymbol,
            language: "python",
            provider: "rope",
            selection_mode: super::resolution::SelectionMode::ExplicitProvider,
            requested_provider: Some("rope"),
        },
        execute_result,
    )
}

fn run_failure_case(runtime: RollbackRuntime) -> RollbackOutcome {
    let workspace = TempDir::new().expect("workspace");
    let file = "notes.py";
    let file_path = workspace.path().join(file);
    std::fs::write(&file_path, original_content_for(file_path.as_path())).expect("write file");

    let request = command_request(standard_rename_args_for_provider(file, "rope"));
    let socket_dir = TempDir::new().expect("socket dir");
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
    .expect("dispatch should complete");

    RollbackOutcome {
        status: result.status,
        stderr: String::from_utf8(output).expect("stderr utf8"),
        content: std::fs::read_to_string(&file_path).expect("read file"),
    }
}

fn assert_rollback_invariants(outcome: &RollbackOutcome, stderr_fragment: &str) {
    assert_eq!(outcome.status, 1);
    assert_eq!(
        outcome.content,
        original_content_for(std::path::Path::new("notes.py"))
    );
    assert!(outcome.stderr.contains(stderr_fragment));
}

#[test]
fn refused_resolution_leaves_target_file_unchanged() {
    let outcome = run_failure_case(refused_runtime());
    assert_rollback_invariants(&outcome, "unsupported_language");
}

#[test]
fn plugin_runtime_error_leaves_target_file_unchanged() {
    let outcome = run_failure_case(rope_python_runtime(ExecuteResult::MissingPlugin("rope")));
    assert_rollback_invariants(&outcome, "act refactor failed");
}

#[test]
fn successful_non_diff_response_leaves_target_file_unchanged() {
    let outcome = run_failure_case(rope_python_runtime(ExecuteResult::Success(
        PluginResponse::success(PluginOutput::Analysis {
            data: serde_json::json!({ "unexpected": true }),
        }),
    )));
    assert_rollback_invariants(&outcome, "did not return diff output");
}
