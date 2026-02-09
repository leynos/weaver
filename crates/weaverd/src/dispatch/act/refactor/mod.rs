//! Handler for `act refactor`.
//!
//! Delegates refactoring operations to registered plugin processes via the
//! `weaver-plugins` crate. The plugin produces a unified diff which is fed
//! through the Double-Lock safety harness before any filesystem change is
//! committed.
//!
//! In this initial Phase 3.1.1 implementation the handler validates the
//! request arguments, resolves the target file, and builds a
//! [`PluginRequest`]. Full plugin execution and safety harness integration
//! will be wired in Phase 3.2 when the daemon runtime holds a
//! [`PluginRunner`] instance.

use std::io::Write;
use std::path::Path;

use tracing::debug;

use weaver_plugins::PluginRequest;
use weaver_plugins::protocol::FilePayload;

use crate::backends::{BackendKind, FusionBackends};
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::{DISPATCH_TARGET, DispatchResult};
use crate::semantic_provider::SemanticBackendProvider;

/// Handles `act refactor` requests.
///
/// Expects `--provider <plugin>` and `--refactoring <operation>` in the
/// request arguments, plus `--file <path>` identifying the target file.
///
/// The handler reads the file content, builds a [`PluginRequest`], and will
/// execute the plugin via a [`PluginRunner`] once the daemon runtime holds
/// a plugin registry (Phase 3.2).
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
    workspace_root: &Path,
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
    let file_path = resolve_file(workspace_root, &args.file)?;
    let file_content = std::fs::read_to_string(&file_path).map_err(|err| {
        DispatchError::invalid_arguments(format!("cannot read file '{}': {err}", args.file))
    })?;

    // Build the plugin request with in-band file content.
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

    let _plugin_request = PluginRequest::with_arguments(
        &args.refactoring,
        vec![FilePayload::new(file_path.clone(), file_content)],
        plugin_args,
    );

    // Phase 3.2 will wire in PluginRunner::execute() here.
    // For now return "not yet available" so the operation is routed correctly
    // but does not attempt execution without a runtime registry.
    writer.write_stderr(format!(
        "act refactor: plugin execution not yet available \
         (provider={}, refactoring={}, file={})\n",
        args.provider, args.refactoring, args.file
    ))?;
    Ok(DispatchResult::with_status(1))
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

fn parse_refactor_args(arguments: &[String]) -> Result<RefactorArgs, DispatchError> {
    let mut provider = None;
    let mut refactoring = None;
    let mut file = None;
    let mut extra = Vec::new();
    let mut iter = arguments.iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--provider" => provider = Some(parse_flag_value(&mut iter, "--provider")?),
            "--refactoring" => refactoring = Some(parse_flag_value(&mut iter, "--refactoring")?),
            "--file" => file = Some(parse_flag_value(&mut iter, "--file")?),
            other => extra.push(other.to_owned()),
        }
    }

    validate_required_args(provider, refactoring, file, extra)
}

/// Consumes the next element from the iterator as the value for a flag.
fn parse_flag_value<'a>(
    iter: &mut impl Iterator<Item = &'a String>,
    flag: &str,
) -> Result<String, DispatchError> {
    iter.next()
        .cloned()
        .ok_or_else(|| DispatchError::invalid_arguments(format!("{flag} requires a value")))
}

/// Validates that all required arguments are present and builds the result.
fn validate_required_args(
    provider: Option<String>,
    refactoring: Option<String>,
    file: Option<String>,
    extra: Vec<String>,
) -> Result<RefactorArgs, DispatchError> {
    Ok(RefactorArgs {
        provider: provider.ok_or_else(|| {
            DispatchError::invalid_arguments("act refactor requires --provider <plugin-name>")
        })?,
        refactoring: refactoring.ok_or_else(|| {
            DispatchError::invalid_arguments("act refactor requires --refactoring <operation>")
        })?,
        file: file.ok_or_else(|| {
            DispatchError::invalid_arguments("act refactor requires --file <path>")
        })?,
        extra,
    })
}

/// Resolves a file path relative to the workspace root.
fn resolve_file(workspace_root: &Path, file: &str) -> Result<std::path::PathBuf, DispatchError> {
    let path = std::path::Path::new(file);
    if path.is_absolute() {
        return Err(DispatchError::invalid_arguments(
            "absolute file paths are not allowed; use a path relative to the workspace root",
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
