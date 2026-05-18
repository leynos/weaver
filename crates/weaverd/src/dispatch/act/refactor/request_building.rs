//! Builds plugin requests for `act refactor`.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use url::Url;
use weaver_plugins::{PluginRequest, capability::CapabilityId, protocol::FilePayload};

use super::{
    arguments,
    metrics::PositionMetrics,
    positions::{LineCol, line_col_to_byte_offset},
    requirements::{
        capability_for_operation,
        effective_operation as supported_effective_operation,
    },
};
use crate::dispatch::errors::DispatchError;

struct ResolvedFile {
    path: PathBuf,
    relative_path: PathBuf,
}

struct CapabilityMappingContext<'a> {
    capability: CapabilityId,
    file_path: &'a Path,
    file_content: &'a str,
    position: Option<LineCol>,
    metrics: &'a dyn PositionMetrics,
}

/// Resolves the target file, reads its content, builds the [`PluginRequest`],
/// and maps the refactoring operation to the corresponding [`CapabilityId`].
pub(super) fn prepare_plugin_request(
    workspace_root: &Path,
    args: &arguments::RefactorArgs,
    metrics: &dyn PositionMetrics,
) -> Result<(PluginRequest, CapabilityId, PathBuf), DispatchError> {
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        DispatchError::invalid_arguments(format!(
            "cannot canonicalize workspace root '{}': {error}",
            workspace_root.display()
        ))
    })?;
    let resolved_file = resolve_file(&canonical_workspace, &args.file)?;
    let mut plugin_args = build_plugin_args(args)?;
    let effective_operation = supported_effective_operation(&args.refactoring)?;
    let capability = capability_for_operation(effective_operation)?;
    let file_content = load_file_contents(&resolved_file.path)?;
    apply_capability_argument_mapping(
        &mut plugin_args,
        CapabilityMappingContext {
            capability,
            file_path: &resolved_file.path,
            file_content: &file_content,
            position: args.position,
            metrics,
        },
    )?;
    plugin_args.insert(
        String::from("refactoring"),
        serde_json::Value::String(String::from(effective_operation)),
    );
    let plugin_request = PluginRequest::with_arguments(
        effective_operation,
        vec![FilePayload::new(resolved_file.relative_path, file_content)],
        plugin_args,
    );
    Ok((plugin_request, capability, resolved_file.path))
}

fn build_plugin_args(
    args: &arguments::RefactorArgs,
) -> Result<HashMap<String, serde_json::Value>, DispatchError> {
    let mut plugin_args = HashMap::new();
    for extra in &args.extra {
        let parts: Vec<&str> = extra.splitn(2, '=').collect();
        let key = parts
            .first()
            .copied()
            .ok_or_else(|| {
                DispatchError::invalid_arguments("refactor extra argument cannot be empty")
            })?
            .trim()
            .to_owned();
        if key.is_empty() {
            return Err(DispatchError::invalid_arguments(format!(
                "refactor extra argument has an empty key: '{extra}'"
            )));
        }
        if key == "refactoring" {
            return Err(DispatchError::invalid_arguments(
                "refactor extra argument must not override reserved key 'refactoring'",
            ));
        }
        if parts.len() == 2 {
            plugin_args.insert(key, serde_json::Value::String(parts[1].to_owned()));
        } else {
            // Bare extra arguments are interpreted as boolean flags.
            plugin_args.insert(key, serde_json::Value::Bool(true));
        }
    }
    Ok(plugin_args)
}

fn apply_capability_argument_mapping(
    plugin_args: &mut HashMap<String, serde_json::Value>,
    context: CapabilityMappingContext<'_>,
) -> Result<(), DispatchError> {
    if context.capability == CapabilityId::RenameSymbol {
        return apply_rename_symbol_mapping(plugin_args, context);
    }
    Ok(())
}

fn contains_parent_traversal(path: &Path) -> bool {
    path.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
}

fn resolve_file(workspace_root: &Path, file: &str) -> Result<ResolvedFile, DispatchError> {
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
    let relative_path = canonical_resolved
        .strip_prefix(workspace_root)
        .map_err(|_| {
            DispatchError::invalid_arguments("resolved file path escapes the workspace root")
        })?
        .to_path_buf();
    Ok(ResolvedFile {
        path: canonical_resolved,
        relative_path,
    })
}

fn load_file_contents(path: &Path) -> Result<String, DispatchError> {
    std::fs::read_to_string(path).map_err(|error| {
        DispatchError::invalid_arguments(format!("cannot read file '{}': {error}", path.display()))
    })
}

#[tracing::instrument(
    level = "debug",
    skip(plugin_args, context),
    fields(
        capability = ?CapabilityId::RenameSymbol,
        file_path = %context.file_path.display(),
        input_form = rename_symbol_input_form(plugin_args, context.position),
    )
)]
fn apply_rename_symbol_mapping(
    plugin_args: &mut HashMap<String, serde_json::Value>,
    context: CapabilityMappingContext<'_>,
) -> Result<(), DispatchError> {
    let file = context.file_path;
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
    if plugin_args.contains_key("position") {
        return Err(invalid_rename_arguments(
            file,
            "refactor rename must use '--position LINE:COL'; trailing 'position=' is reserved for \
             the internal plugin contract",
        ));
    }
    if let Some(position) = context.position {
        let offset = line_col_to_byte_offset(
            context.file_content,
            position.line,
            position.column,
            Some(file),
        )
        .inspect_err(|error| {
            context.metrics.increment_conversion_error();
            warn_position_conversion_error(file, position, error);
        })?;
        plugin_args.insert(
            String::from("position"),
            serde_json::Value::String(offset.to_string()),
        );
        return Ok(());
    }
    if let Some(offset_val) = plugin_args.remove("offset") {
        tracing::warn!(
            file_path = %file.display(),
            "using deprecated offset= for rename position"
        );
        if offset_val.is_string() || offset_val.is_number() {
            plugin_args.insert(String::from("position"), offset_val);
            return Ok(());
        }

        return Err(invalid_rename_arguments(
            file,
            "refactor rename deprecated offset= must be a numeric or string byte offset",
        ));
    }
    Err(invalid_rename_arguments(
        file,
        "refactor rename requires --position LINE:COL",
    ))
}

fn rename_symbol_input_form(
    plugin_args: &HashMap<String, serde_json::Value>,
    position: Option<LineCol>,
) -> &'static str {
    match (position.is_some(), plugin_args.contains_key("offset")) {
        (true, true) => "position_and_deprecated_offset",
        (true, false) => "--position",
        (false, true) => "deprecated_offset",
        (false, false) => "missing",
    }
}

fn invalid_rename_arguments(file: &Path, message: &str) -> DispatchError {
    DispatchError::invalid_arguments(format!("{message} for '{}'", file.display()))
}

fn warn_position_conversion_error(file: &Path, position: LineCol, error: &DispatchError) {
    tracing::warn!(
        line = position.line,
        column = position.column,
        file_path = %file.display(),
        error = %error,
        "position is out of range for the target file"
    );
}

#[cfg(test)]
mod tests {
    //! Unit tests for request-building internals.

    use serde_json::Value;

    use super::*;

    #[test]
    fn apply_rename_symbol_mapping_rejects_non_string_or_numeric_offset() {
        let mut plugin_args = HashMap::from([
            (String::from("offset"), Value::Bool(false)),
            (
                String::from("new_name"),
                Value::String(String::from("woven")),
            ),
        ]);
        let err = apply_rename_symbol_mapping(
            &mut plugin_args,
            CapabilityMappingContext {
                capability: CapabilityId::RenameSymbol,
                file_path: Path::new("/tmp"),
                file_content: "hello world",
                position: None,
                metrics: &crate::dispatch::act::refactor::metrics::NullPositionMetrics,
            },
        )
        .expect_err("offset must be rejected when not numeric");

        assert!(matches!(err, DispatchError::InvalidArguments { .. }));
        let invalid_arguments = match err {
            DispatchError::InvalidArguments { message } => message,
            _ => unreachable!(),
        };
        assert!(invalid_arguments.contains("must be a numeric or string byte offset"));
        assert!(invalid_arguments.contains("/tmp"));
    }
}
