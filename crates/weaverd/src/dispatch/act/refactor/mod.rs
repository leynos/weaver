//! Handler for `act refactor`.
//!
//! Delegates refactoring operations to registered plugin processes via the
//! `weaver-plugins` crate. The plugin produces a unified diff which is fed
//! through the Double-Lock safety harness before any filesystem change is
//! committed.
//!
//! The handler validates the request arguments, resolves the target file, and
//! builds a [`PluginRequest`]. Successful plugin responses with diff output are
//! forwarded to the existing `act apply-patch` pipeline so syntactic and
//! semantic locks are reused without duplicating safety-critical logic.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use tracing::debug;

use weaver_plugins::manifest::{PluginKind, PluginManifest, PluginMetadata};
use weaver_plugins::process::SandboxExecutor;
use weaver_plugins::protocol::FilePayload;
use weaver_plugins::runner::PluginRunner;
use weaver_plugins::{PluginError, PluginOutput, PluginRegistry, PluginRequest, PluginResponse};

use crate::backends::{BackendKind, FusionBackends};
use crate::dispatch::act::apply_patch;
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::{CommandDescriptor, CommandRequest};
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::{DISPATCH_TARGET, DispatchResult};
use crate::semantic_provider::SemanticBackendProvider;

/// Environment variable overriding the rope plugin executable path.
pub(crate) const ROPE_PLUGIN_PATH_ENV: &str = "WEAVER_ROPE_PLUGIN_PATH";
const DEFAULT_ROPE_PLUGIN_PATH: &str = "/usr/bin/weaver-plugin-rope";
const ROPE_PLUGIN_NAME: &str = "rope";
const ROPE_PLUGIN_VERSION: &str = "0.1.0";

/// Runtime abstraction for executing refactor plugins.
pub(crate) trait RefactorPluginRuntime {
    /// Executes the named plugin with the provided request.
    fn execute(
        &self,
        provider: &str,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError>;
}

/// Dependencies required by the `act refactor` handler.
pub(crate) struct RefactorDependencies<'a> {
    workspace_root: &'a Path,
    runtime: &'a (dyn RefactorPluginRuntime + Send + Sync),
}

impl<'a> RefactorDependencies<'a> {
    /// Creates a dependency bundle for handler execution.
    pub(crate) const fn new(
        workspace_root: &'a Path,
        runtime: &'a (dyn RefactorPluginRuntime + Send + Sync),
    ) -> Self {
        Self {
            workspace_root,
            runtime,
        }
    }
}

/// Sandbox-backed runtime that resolves plugins from a registry.
pub(crate) struct SandboxRefactorRuntime {
    runner: Option<PluginRunner<SandboxExecutor>>,
    startup_error: Option<String>,
}

impl SandboxRefactorRuntime {
    /// Builds the default runtime from environment configuration.
    #[must_use]
    pub fn from_environment() -> Self {
        let mut registry = PluginRegistry::new();
        let executable = resolve_rope_plugin_path(std::env::var_os(ROPE_PLUGIN_PATH_ENV));
        let metadata =
            PluginMetadata::new(ROPE_PLUGIN_NAME, ROPE_PLUGIN_VERSION, PluginKind::Actuator);
        let manifest = PluginManifest::new(metadata, vec![String::from("python")], executable);

        match registry.register(manifest) {
            Ok(()) => Self {
                runner: Some(PluginRunner::new(registry, SandboxExecutor)),
                startup_error: None,
            },
            Err(error) => Self {
                runner: None,
                startup_error: Some(format!("failed to initialise refactor runtime: {error}")),
            },
        }
    }
}

impl RefactorPluginRuntime for SandboxRefactorRuntime {
    fn execute(
        &self,
        provider: &str,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        let runner = self.runner.as_ref().ok_or_else(|| PluginError::Manifest {
            message: self
                .startup_error
                .clone()
                .unwrap_or_else(|| String::from("refactor runtime is unavailable")),
        })?;
        runner.execute(provider, request)
    }
}

/// Constructs the default refactor plugin runtime for daemon dispatch.
#[must_use]
pub(crate) fn default_runtime() -> Arc<dyn RefactorPluginRuntime + Send + Sync> {
    Arc::new(SandboxRefactorRuntime::from_environment())
}

/// Converts an optional executable override to an absolute plugin path.
fn resolve_rope_plugin_path(raw_override: Option<OsString>) -> PathBuf {
    let candidate = raw_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ROPE_PLUGIN_PATH));
    if candidate.is_absolute() {
        return candidate;
    }

    match std::env::current_dir() {
        Ok(cwd) => cwd.join(candidate),
        Err(_) => PathBuf::from(DEFAULT_ROPE_PLUGIN_PATH),
    }
}

/// Handles `act refactor` requests.
///
/// Expects `--provider <plugin>` and `--refactoring <operation>` in the
/// request arguments, plus `--file <path>` identifying the target file.
///
/// The handler reads the file content, executes the plugin, and forwards
/// successful diff output through `act apply-patch` for Double-Lock
/// verification and atomic commit.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    dependencies: RefactorDependencies<'_>,
) -> Result<DispatchResult, DispatchError> {
    let args = parse_refactor_args(&request.arguments)?;

    debug!(
        target: DISPATCH_TARGET,
        provider = args.provider,
        refactoring = args.refactoring,
        file = args.file,
        "handling act refactor"
    );

    backends
        .ensure_started(BackendKind::Semantic)
        .map_err(DispatchError::backend_startup)?;

    // Resolve the target file within the workspace.
    let file_path = resolve_file(dependencies.workspace_root, &args.file)?;
    let file_content = std::fs::read_to_string(&file_path).map_err(|err| {
        DispatchError::invalid_arguments(format!("cannot read file '{}': {err}", args.file))
    })?;

    let mut plugin_args = std::collections::HashMap::new();
    plugin_args.insert(
        "refactoring".into(),
        serde_json::Value::String(args.refactoring.clone()),
    );
    // Forward any extra arguments beyond the known flags.
    for extra in &args.extra {
        let parts: Vec<&str> = extra.splitn(2, '=').collect();
        if parts.len() == 2 {
            plugin_args.insert(
                parts[0].to_owned(),
                serde_json::Value::String(parts[1].to_owned()),
            );
        }
    }

    let plugin_request = PluginRequest::with_arguments(
        &args.refactoring,
        vec![FilePayload::new(PathBuf::from(&args.file), file_content)],
        plugin_args,
    );

    match dependencies
        .runtime
        .execute(&args.provider, &plugin_request)
    {
        Ok(response) => {
            handle_plugin_response(response, writer, backends, dependencies.workspace_root)
        }
        Err(error) => {
            writer.write_stderr(format!(
                "act refactor failed: {error} (provider={}, refactoring={}, file={})\n",
                args.provider, args.refactoring, args.file
            ))?;
            Ok(DispatchResult::with_status(1))
        }
    }
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

struct RefactorArgs {
    provider: String,
    refactoring: String,
    file: String,
    extra: Vec<String>,
}

/// Accumulates parsed flag values during argument iteration.
#[derive(Default)]
struct RefactorArgsBuilder {
    provider: Option<String>,
    refactoring: Option<String>,
    file: Option<String>,
    extra: Vec<String>,
}

impl RefactorArgsBuilder {
    /// Finalizes the builder, requiring all mandatory fields.
    fn build(self) -> Result<RefactorArgs, DispatchError> {
        Ok(RefactorArgs {
            provider: self.provider.ok_or_else(|| {
                DispatchError::invalid_arguments("act refactor requires --provider <plugin-name>")
            })?,
            refactoring: self.refactoring.ok_or_else(|| {
                DispatchError::invalid_arguments("act refactor requires --refactoring <operation>")
            })?,
            file: self.file.ok_or_else(|| {
                DispatchError::invalid_arguments("act refactor requires --file <path>")
            })?,
            extra: self.extra,
        })
    }
}

fn parse_refactor_args(arguments: &[String]) -> Result<RefactorArgs, DispatchError> {
    let mut builder = RefactorArgsBuilder::default();
    let mut iter = arguments.iter();

    while let Some(arg) = iter.next() {
        apply_flag(arg, &mut iter, &mut builder)?;
    }

    builder.build()
}

/// Classifies a single argument token, consuming the next token as the
/// value when the argument is a recognised flag.
fn apply_flag<'a>(
    arg: &str,
    iter: &mut impl Iterator<Item = &'a String>,
    builder: &mut RefactorArgsBuilder,
) -> Result<(), DispatchError> {
    match arg {
        "--provider" => {
            builder.provider =
                Some(iter.next().cloned().ok_or_else(|| {
                    DispatchError::invalid_arguments("--provider requires a value")
                })?);
        }
        "--refactoring" => {
            builder.refactoring = Some(iter.next().cloned().ok_or_else(|| {
                DispatchError::invalid_arguments("--refactoring requires a value")
            })?);
        }
        "--file" => {
            builder.file = Some(
                iter.next()
                    .cloned()
                    .ok_or_else(|| DispatchError::invalid_arguments("--file requires a value"))?,
            );
        }
        other => builder.extra.push(other.to_owned()),
    }
    Ok(())
}

/// Returns `true` if any component of `path` is a parent-directory reference.
fn contains_parent_traversal(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// Resolves a file path relative to the workspace root.
fn resolve_file(workspace_root: &Path, file: &str) -> Result<std::path::PathBuf, DispatchError> {
    let path = std::path::Path::new(file);
    if path.is_absolute() {
        return Err(DispatchError::invalid_arguments(
            "absolute file paths are not allowed; use a path relative to the workspace root",
        ));
    }
    if contains_parent_traversal(path) {
        return Err(DispatchError::invalid_arguments(
            "path traversal is not allowed",
        ));
    }
    let resolved = workspace_root.join(path);
    if !resolved.starts_with(workspace_root) {
        return Err(DispatchError::invalid_arguments(
            "path traversal is not allowed",
        ));
    }
    Ok(resolved)
}

fn handle_plugin_response<W: Write>(
    response: PluginResponse,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    workspace_root: &Path,
) -> Result<DispatchResult, DispatchError> {
    if !response.is_success() {
        let diagnostics: Vec<String> = response
            .diagnostics()
            .iter()
            .map(|diag| diag.message().to_owned())
            .collect();
        let message = if diagnostics.is_empty() {
            String::from("plugin reported failure without diagnostics")
        } else {
            diagnostics.join("; ")
        };
        writer.write_stderr(format!("act refactor failed: {message}\n"))?;
        return Ok(DispatchResult::with_status(1));
    }

    match response.output() {
        PluginOutput::Diff { content } => {
            forward_diff_to_apply_patch(content, writer, backends, workspace_root)
        }
        PluginOutput::Analysis { .. } | PluginOutput::Empty => {
            writer.write_stderr(
                "act refactor failed: plugin succeeded but did not return diff output\n",
            )?;
            Ok(DispatchResult::with_status(1))
        }
    }
}

fn forward_diff_to_apply_patch<W: Write>(
    patch: &str,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    workspace_root: &Path,
) -> Result<DispatchResult, DispatchError> {
    let patch_request = CommandRequest {
        command: CommandDescriptor {
            domain: String::from("act"),
            operation: String::from("apply-patch"),
        },
        arguments: Vec::new(),
        patch: Some(patch.to_owned()),
    };
    apply_patch::handle(&patch_request, writer, backends, workspace_root)
}

#[cfg(test)]
mod behaviour;
#[cfg(test)]
mod tests;
