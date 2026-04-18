//! Builds plugin requests for `act refactor`.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use url::Url;
use weaver_plugins::{PluginRequest, capability::CapabilityId, protocol::FilePayload};

use super::arguments;
use crate::dispatch::errors::DispatchError;

/// Resolves the target file, reads its content, builds the [`PluginRequest`],
/// and maps the refactoring operation to the corresponding [`CapabilityId`].
pub(super) fn prepare_plugin_request(
    workspace_root: &Path,
    args: &arguments::RefactorArgs,
) -> Result<(PluginRequest, CapabilityId, PathBuf), DispatchError> {
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        DispatchError::invalid_arguments(format!(
            "cannot canonicalize workspace root '{}': {error}",
            workspace_root.display()
        ))
    })?;
    let file_path = resolve_file(&canonical_workspace, &args.file)?;
    let relative_file_path = file_path.strip_prefix(&canonical_workspace).map_err(|_| {
        DispatchError::invalid_arguments("resolved file path escapes the workspace root")
    })?;
    let mut plugin_args = build_plugin_args(args)?;
    let effective_operation = effective_operation(&mut plugin_args, args, &file_path)?;
    let capability = capability_from_operation(&effective_operation)?;
    let file_content = std::fs::read_to_string(&file_path).map_err(|err| {
        DispatchError::invalid_arguments(format!("cannot read file '{}': {err}", args.file))
    })?;
    let plugin_request = PluginRequest::with_arguments(
        &effective_operation,
        vec![FilePayload::new(
            relative_file_path.to_path_buf(),
            file_content,
        )],
        plugin_args,
    );
    Ok((plugin_request, capability, file_path))
}

fn build_plugin_args(
    args: &arguments::RefactorArgs,
) -> Result<HashMap<String, serde_json::Value>, DispatchError> {
    let mut plugin_args = HashMap::new();
    plugin_args.insert(
        "refactoring".into(),
        serde_json::Value::String(args.refactoring.clone()),
    );
    for extra in &args.extra {
        let parts: Vec<&str> = extra.splitn(2, '=').collect();
        let key = parts.first().copied().ok_or_else(|| {
            DispatchError::invalid_arguments("refactor extra argument cannot be empty")
        })?;
        if key.trim().is_empty() {
            return Err(DispatchError::invalid_arguments(format!(
                "refactor extra argument has an empty key: '{extra}'"
            )));
        }
        if parts.len() == 2 {
            plugin_args.insert(
                key.to_owned(),
                serde_json::Value::String(parts[1].to_owned()),
            );
        } else if parts.len() == 1 {
            // Bare extra arguments are interpreted as boolean flags.
            plugin_args.insert(key.to_owned(), serde_json::Value::Bool(true));
        }
    }
    Ok(plugin_args)
}

fn effective_operation(
    plugin_args: &mut HashMap<String, serde_json::Value>,
    args: &arguments::RefactorArgs,
    file_path: &Path,
) -> Result<String, DispatchError> {
    match args.refactoring.as_str() {
        "rename" => {
            apply_rename_symbol_mapping(plugin_args, file_path)?;
            Ok(String::from("rename-symbol"))
        }
        _ => Ok(args.refactoring.clone()),
    }
}

fn contains_parent_traversal(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

fn resolve_file(workspace_root: &Path, file: &str) -> Result<PathBuf, DispatchError> {
    let path = Path::new(file);
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
    let canonical_resolved = resolved.canonicalize().map_err(|error| {
        DispatchError::invalid_arguments(format!("cannot resolve file '{}': {error}", file))
    })?;
    if !canonical_resolved.starts_with(workspace_root) {
        return Err(DispatchError::invalid_arguments(
            "path traversal is not allowed",
        ));
    }
    Ok(canonical_resolved)
}

fn apply_rename_symbol_mapping(
    plugin_args: &mut HashMap<String, serde_json::Value>,
    file: &Path,
) -> Result<(), DispatchError> {
    plugin_args.insert(
        String::from("uri"),
        serde_json::Value::String(
            Url::from_file_path(file)
                .map_err(|()| {
                    DispatchError::invalid_arguments(format!(
                        "cannot construct file URI for '{}'",
                        file.display()
                    ))
                })?
                .to_string(),
        ),
    );
    if let Some(offset_val) = plugin_args.remove("offset") {
        plugin_args.insert(String::from("position"), offset_val);
    }
    Ok(())
}

fn capability_from_operation(operation: &str) -> Result<CapabilityId, DispatchError> {
    // TODO: Extend this mapping when additional refactoring operations are added
    // (e.g., extract-method, inline-variable, move-function).
    match operation {
        "rename-symbol" => Ok(CapabilityId::RenameSymbol),
        other => Err(DispatchError::invalid_arguments(format!(
            "act refactor does not support capability resolution for '{other}' (only \
             'rename-symbol' is currently implemented)"
        ))),
    }
}
