//! Handler for `act refactor`.
//!
//! Delegates refactoring operations to registered plugin processes via the
//! `weaver-plugins` crate. The plugin produces a unified diff which is fed
//! through the Double-Lock safety harness before any filesystem change is
//! committed.
//!
//! The handler validates arguments, resolves the target file, and builds a
//! [`PluginRequest`]. Diff output is forwarded to `act apply-patch` so
//! syntactic and semantic locks are reused without duplicating safety logic.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arguments::parse_refactor_args;
use tracing::debug;
use url::Url;

use weaver_plugins::capability::CapabilityId;
use weaver_plugins::process::SandboxExecutor;
use weaver_plugins::protocol::FilePayload;
use weaver_plugins::runner::PluginRunner;
use weaver_plugins::{PluginError, PluginRegistry, PluginRequest, PluginResponse};

use crate::backends::{BackendKind, FusionBackends};
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::{DISPATCH_TARGET, DispatchResult};
use crate::semantic_provider::SemanticBackendProvider;
use manifests::{rope_manifest, rust_analyzer_manifest};
use plugin_paths::{
    ROPE_PLUGIN_PATH_ENV, RUST_ANALYZER_PLUGIN_PATH_ENV, resolve_rope_plugin_path,
    resolve_rust_analyzer_plugin_path,
};
use resolution::{CapabilityResolutionEnvelope, ResolutionRequest, resolve_provider};

mod arguments;
mod candidates;
mod manifests;
mod plugin_paths;
mod refusal;
mod requirements;
mod resolution;
mod response_handling;

/// Runtime abstraction for executing refactor plugins.
pub(crate) trait RefactorPluginRuntime {
    /// Resolves a provider for the given capability request.
    fn resolve(
        &self,
        request: ResolutionRequest<'_>,
    ) -> Result<CapabilityResolutionEnvelope, PluginError>;

    /// Executes the named plugin with the provided request.
    fn execute(
        &self,
        provider: &str,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError>;
}

/// Sandbox-backed runtime that resolves plugins from a registry.
pub(crate) struct SandboxRefactorRuntime {
    registry: PluginRegistry,
    runner: PluginRunner<SandboxExecutor>,
}

impl SandboxRefactorRuntime {
    /// Builds the runtime from environment configuration.
    ///
    /// # Errors
    ///
    /// Returns an error description if plugin registration fails.
    pub fn from_environment() -> Result<Self, String> {
        let mut registry = PluginRegistry::new();
        let rope_executable = resolve_rope_plugin_path(std::env::var_os(ROPE_PLUGIN_PATH_ENV));
        registry
            .register(rope_manifest(rope_executable))
            .map_err(|error| format!("failed to initialize refactor runtime: {error}"))?;

        let rust_analyzer_executable =
            resolve_rust_analyzer_plugin_path(std::env::var_os(RUST_ANALYZER_PLUGIN_PATH_ENV));
        registry
            .register(rust_analyzer_manifest(rust_analyzer_executable))
            .map_err(|error| format!("failed to initialize refactor runtime: {error}"))?;

        let runner = PluginRunner::new(registry.clone(), SandboxExecutor);
        Ok(Self { registry, runner })
    }
}

impl RefactorPluginRuntime for SandboxRefactorRuntime {
    fn resolve(
        &self,
        request: ResolutionRequest<'_>,
    ) -> Result<CapabilityResolutionEnvelope, PluginError> {
        Ok(resolve_provider(&self.registry, request))
    }

    fn execute(
        &self,
        provider: &str,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        self.runner.execute(provider, request)
    }
}

/// Runtime that reports an initialization error on every execution attempt.
struct NoopRefactorRuntime {
    message: String,
}

impl RefactorPluginRuntime for NoopRefactorRuntime {
    fn resolve(
        &self,
        _request: ResolutionRequest<'_>,
    ) -> Result<CapabilityResolutionEnvelope, PluginError> {
        Err(PluginError::Manifest {
            message: self.message.clone(),
        })
    }

    fn execute(
        &self,
        _provider: &str,
        _request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        Err(PluginError::Manifest {
            message: self.message.clone(),
        })
    }
}

/// Constructs the default refactor plugin runtime for daemon dispatch.
#[must_use]
pub(crate) fn default_runtime() -> Arc<dyn RefactorPluginRuntime + Send + Sync> {
    match SandboxRefactorRuntime::from_environment() {
        Ok(runtime) => Arc::new(runtime),
        Err(message) => Arc::new(NoopRefactorRuntime { message }),
    }
}

/// Context for executing refactor operations.
pub(crate) struct RefactorContext<'a> {
    /// Mutable reference to the fusion backends for starting semantic services.
    pub backends: &'a mut FusionBackends<SemanticBackendProvider>,
    /// Root directory of the workspace being refactored.
    pub workspace_root: &'a Path,
    /// Runtime used to execute the refactor plugin process.
    pub runtime: &'a dyn RefactorPluginRuntime,
}

/// Resolves the target file, reads its content, builds the [`PluginRequest`],
/// and maps the refactoring operation to the corresponding [`CapabilityId`].
fn prepare_plugin_request(
    workspace_root: &Path,
    args: &arguments::RefactorArgs,
) -> Result<(PluginRequest, CapabilityId, PathBuf), DispatchError> {
    let file_path = resolve_file(workspace_root, &args.file)?;
    let file_content = std::fs::read_to_string(&file_path).map_err(|err| {
        DispatchError::invalid_arguments(format!("cannot read file '{}': {err}", args.file))
    })?;

    let mut plugin_args = std::collections::HashMap::new();
    plugin_args.insert(
        "refactoring".into(),
        serde_json::Value::String(args.refactoring.clone()),
    );
    for extra in &args.extra {
        let parts: Vec<&str> = extra.splitn(2, '=').collect();
        if parts.len() == 2 {
            plugin_args.insert(
                parts[0].to_owned(),
                serde_json::Value::String(parts[1].to_owned()),
            );
        }
    }

    let effective_operation = match args.refactoring.as_str() {
        "rename" => {
            apply_rename_symbol_mapping(&mut plugin_args, &args.file)?;
            String::from("rename-symbol")
        }
        _ => args.refactoring.clone(),
    };

    let plugin_request = PluginRequest::with_arguments(
        &effective_operation,
        vec![FilePayload::new(PathBuf::from(&args.file), file_content)],
        plugin_args,
    );

    let capability = capability_from_operation(&effective_operation)?;
    Ok((plugin_request, capability, file_path))
}

/// Handles `act refactor` requests.
///
/// Expects `--provider <plugin>`, `--refactoring <operation>`, and
/// `--file <path>` in the request arguments.
///
/// The handler reads the file content, executes the plugin, and forwards
/// successful diff output through `act apply-patch` for Double-Lock
/// verification and atomic commit.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    context: RefactorContext<'_>,
) -> Result<DispatchResult, DispatchError> {
    let args = parse_refactor_args(&request.arguments)?;

    debug!(
        target: DISPATCH_TARGET,
        provider = args.provider,
        refactoring = args.refactoring,
        file = args.file,
        "handling act refactor"
    );

    let (plugin_request, capability, file_path) =
        prepare_plugin_request(context.workspace_root, &args)?;
    let resolution = match context.runtime.resolve(ResolutionRequest::new(
        capability,
        file_path.as_path(),
        Some(&args.provider),
    )) {
        Ok(resolution) => resolution,
        Err(error) => {
            writer.write_stderr(format!(
                "act refactor failed: {error} (provider={}, refactoring={}, file={})\n",
                args.provider, args.refactoring, args.file
            ))?;
            return Ok(DispatchResult::with_status(1));
        }
    };

    write_capability_resolution(writer, &resolution)?;
    let Some(selected_provider) = resolution.details().selected_provider() else {
        return Ok(DispatchResult::with_status(1));
    };

    match context.runtime.execute(selected_provider, &plugin_request) {
        Ok(response) => {
            // Ensure semantic backend is started before forwarding to apply-patch
            context
                .backends
                .ensure_started(BackendKind::Semantic)
                .map_err(DispatchError::backend_startup)?;
            handle_plugin_response(response, writer, context.backends, context.workspace_root)
        }
        Err(error) => {
            writer.write_stderr(format!(
                "act refactor failed: {error} (provider={}, refactoring={}, file={})\n",
                selected_provider, args.refactoring, args.file
            ))?;
            Ok(DispatchResult::with_status(1))
        }
    }
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

/// Rewrites `plugin_args` to conform with the `rename-symbol` contract:
/// injects `uri` from `file` and renames `offset` to `position`.
fn apply_rename_symbol_mapping(
    plugin_args: &mut std::collections::HashMap<String, serde_json::Value>,
    file: &str,
) -> Result<(), DispatchError> {
    plugin_args.insert(
        String::from("uri"),
        serde_json::Value::String(to_file_uri(file).map_err(|error| {
            DispatchError::invalid_arguments(format!(
                "cannot construct file URI for '{file}': {error}"
            ))
        })?),
    );
    if let Some(offset_val) = plugin_args.remove("offset") {
        plugin_args.insert(String::from("position"), offset_val);
    }
    Ok(())
}

fn to_file_uri(path: &str) -> Result<String, url::ParseError> {
    let mut url = Url::parse("file:///")?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| url::ParseError::RelativeUrlWithoutBase)?;
        segments.extend(path.split('/'));
    }
    Ok(url.to_string())
}

fn capability_from_operation(operation: &str) -> Result<CapabilityId, DispatchError> {
    // TODO: Extend this mapping when additional refactoring operations are added
    // (e.g., extract-method, inline-variable, move-function).
    match operation {
        "rename-symbol" => Ok(CapabilityId::RenameSymbol),
        other => Err(DispatchError::invalid_arguments(format!(
            "act refactor does not support capability resolution for '{other}' (only 'rename-symbol' is currently implemented)"
        ))),
    }
}

fn write_capability_resolution<W: Write>(
    writer: &mut ResponseWriter<W>,
    resolution: &CapabilityResolutionEnvelope,
) -> Result<(), DispatchError> {
    let json = serde_json::to_string(resolution)?;
    writer.write_stderr(format!("{json}\n"))
}

use response_handling::handle_plugin_response;
#[cfg(test)]
mod behaviour;
#[cfg(test)]
mod contract_tests;
#[cfg(test)]
mod resolution_tests;
#[cfg(test)]
mod tests;
