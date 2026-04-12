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
    let file_path = resolve_file(workspace_root, &args.file)?;
    let file_content = std::fs::read_to_string(&file_path).map_err(|err| {
        DispatchError::invalid_arguments(format!("cannot read file '{}': {err}", args.file))
    })?;
    let mut plugin_args = build_plugin_args(args);
    let effective_operation = effective_operation(&mut plugin_args, args)?;
    let plugin_request = PluginRequest::with_arguments(
        &effective_operation,
        vec![FilePayload::new(PathBuf::from(&args.file), file_content)],
        plugin_args,
    );
    let capability = capability_from_operation(&effective_operation)?;
    Ok((plugin_request, capability, file_path))
}

fn build_plugin_args(args: &arguments::RefactorArgs) -> HashMap<String, serde_json::Value> {
    let mut plugin_args = HashMap::new();
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
    plugin_args
}

fn effective_operation(
    plugin_args: &mut HashMap<String, serde_json::Value>,
    args: &arguments::RefactorArgs,
) -> Result<String, DispatchError> {
    match args.refactoring.as_str() {
        "rename" => {
            apply_rename_symbol_mapping(plugin_args, &args.file)?;
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
    if !resolved.starts_with(workspace_root) {
        return Err(DispatchError::invalid_arguments(
            "path traversal is not allowed",
        ));
    }
    Ok(resolved)
}

fn apply_rename_symbol_mapping(
    plugin_args: &mut HashMap<String, serde_json::Value>,
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
            "act refactor does not support capability resolution for '{other}' (only \
             'rename-symbol' is currently implemented)"
        ))),
    }
}
