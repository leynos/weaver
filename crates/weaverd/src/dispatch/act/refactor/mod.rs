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

use std::{io::Write, path::Path, sync::Arc};

use arguments::parse_refactor_args;
use manifests::{rope_manifest, rust_analyzer_manifest};
use plugin_paths::{
    ROPE_PLUGIN_PATH_ENV,
    RUST_ANALYZER_PLUGIN_PATH_ENV,
    resolve_rope_plugin_path,
    resolve_rust_analyzer_plugin_path,
};
use request_building::prepare_plugin_request;
use resolution::{CapabilityResolutionEnvelope, ResolutionRequest, resolve_provider};
use tracing::debug;
use weaver_plugins::{
    PluginError,
    PluginRegistry,
    PluginRequest,
    PluginResponse,
    capability::CapabilityId,
    process::SandboxExecutor,
    runner::PluginRunner,
};

use crate::{
    backends::{BackendKind, FusionBackends},
    dispatch::{
        errors::DispatchError,
        request::CommandRequest,
        response::ResponseWriter,
        router::{DISPATCH_TARGET, DispatchResult},
    },
    semantic_provider::SemanticBackendProvider,
};

mod arguments;
mod candidates;
mod manifests;
mod plugin_paths;
#[cfg(test)]
pub(super) mod refactor_helpers;
mod refusal;

mod request_building;
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
/// Parameters required for provider resolution.
struct ResolutionParams<'a> {
    runtime: &'a dyn RefactorPluginRuntime,
    capability: CapabilityId,
    file_path: &'a std::path::Path,
    provider_override: Option<&'a str>,
}
/// Resolves the provider for the refactor operation.
fn resolve_provider_with_fallback(
    params: ResolutionParams<'_>,
    args: &arguments::RefactorArgs,
    writer: &mut ResponseWriter<impl Write>,
) -> Result<Option<CapabilityResolutionEnvelope>, DispatchError> {
    match params.runtime.resolve(ResolutionRequest::new(
        params.capability,
        params.file_path,
        params.provider_override,
    )) {
        Ok(resolution) => Ok(Some(resolution)),
        Err(error) => {
            writer.write_stderr(format!(
                "act refactor failed: {error} (provider={}, refactoring={}, file={})\n",
                args.provider.as_deref().unwrap_or("<auto>"),
                args.refactoring,
                args.file
            ))?;
            Ok(None)
        }
    }
}

/// Parameters required for plugin execution.
struct ExecutionParams<'a> {
    runtime: &'a dyn RefactorPluginRuntime,
    selected_provider: &'a str,
    plugin_request: &'a PluginRequest,
}

/// Starts the semantic backend and handles the plugin response.
fn handle_successful_execution<W: Write>(
    response: PluginResponse,
    writer: &mut ResponseWriter<W>,
    context: &mut RefactorContext<'_>,
) -> Result<DispatchResult, DispatchError> {
    context
        .backends
        .ensure_started(BackendKind::Semantic)
        .map_err(DispatchError::backend_startup)?;
    handle_plugin_response(response, writer, context.backends, context.workspace_root)
}
/// Handles `act refactor` requests.
///
/// Expects `--refactoring <operation>` and `--file <path>` in the request
/// arguments. `--provider <plugin>` is optional and acts as an explicit
/// compatibility override when supplied.
///
/// The handler reads the file content, executes the plugin, and forwards
/// successful diff output through `act apply-patch` for Double-Lock
/// verification and atomic commit.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    mut context: RefactorContext<'_>,
) -> Result<DispatchResult, DispatchError> {
    let args = parse_refactor_args(&request.arguments)?;

    debug!(
        target: DISPATCH_TARGET,
        provider = args.provider.as_deref().unwrap_or("<auto>"),
        refactoring = args.refactoring,
        file = args.file,
        "handling act refactor"
    );

    let (plugin_request, capability, file_path) =
        prepare_plugin_request(context.workspace_root, &args)?;

    let resolution_params = ResolutionParams {
        runtime: context.runtime,
        capability,
        file_path: file_path.as_path(),
        provider_override: args.provider.as_deref(),
    };

    let Some(resolution) = resolve_provider_with_fallback(resolution_params, &args, writer)? else {
        return Ok(DispatchResult::with_status(1));
    };

    write_capability_resolution(writer, &resolution)?;
    let Some(selected_provider) = resolution.details().selected_provider() else {
        return Ok(DispatchResult::with_status(1));
    };

    let execution_params = ExecutionParams {
        runtime: context.runtime,
        selected_provider,
        plugin_request: &plugin_request,
    };

    execute_plugin_and_handle_response(execution_params, &args, writer, &mut context)
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
mod rollback_tests;
#[cfg(test)]
mod tests;

/// Executes the plugin and handles the response.
fn execute_plugin_and_handle_response<W: Write>(
    params: ExecutionParams<'_>,
    args: &arguments::RefactorArgs,
    writer: &mut ResponseWriter<W>,
    context: &mut RefactorContext<'_>,
) -> Result<DispatchResult, DispatchError> {
    let result = params
        .runtime
        .execute(params.selected_provider, params.plugin_request);

    if let Ok(response) = result {
        return handle_successful_execution(response, writer, context);
    }

    write_execution_error(&result.unwrap_err(), params.selected_provider, args, writer)?;
    Ok(DispatchResult::with_status(1))
}

/// Writes an error message for a failed plugin execution.
fn write_execution_error<W: Write>(
    error: &PluginError,
    selected_provider: &str,
    args: &arguments::RefactorArgs,
    writer: &mut ResponseWriter<W>,
) -> Result<(), DispatchError> {
    writer.write_stderr(format!(
        "act refactor failed: {error} (provider={}, refactoring={}, file={})\n",
        selected_provider, args.refactoring, args.file
    ))
}
