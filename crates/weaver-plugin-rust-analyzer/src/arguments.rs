//! Argument parsing for `rename-symbol` plugin requests.
//!
//! Validates and extracts the `uri`, `position`, and `new_name` fields from a
//! rename-symbol plugin request.

use std::collections::HashMap;

/// Validated rename-symbol arguments extracted from a plugin request.
pub(crate) struct RenameSymbolArgs {
    uri: String,
    offset: usize,
    new_name: String,
}

impl RenameSymbolArgs {
    /// Returns the request URI.
    pub(crate) fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the byte offset parsed from the `position` field.
    pub(crate) const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the new symbol name.
    pub(crate) fn new_name(&self) -> &str {
        &self.new_name
    }
}

/// Parses and validates rename-symbol arguments from the request map.
///
/// # Errors
///
/// Returns a human-readable error message if any required field is missing,
/// has the wrong type, or is empty.
pub(crate) fn parse_rename_symbol_arguments(
    arguments: &HashMap<String, serde_json::Value>,
) -> Result<RenameSymbolArgs, String> {
    let uri = parse_uri(arguments)?;
    let offset = parse_position(arguments)?;
    let new_name = parse_new_name(arguments)?;
    Ok(RenameSymbolArgs {
        uri,
        offset,
        new_name,
    })
}

fn parse_uri(arguments: &HashMap<String, serde_json::Value>) -> Result<String, String> {
    let uri_value = arguments
        .get("uri")
        .ok_or_else(|| String::from("rename-symbol operation requires 'uri' argument"))?;
    let uri = uri_value
        .as_str()
        .ok_or_else(|| String::from("uri argument must be a string"))?;
    if uri.trim().is_empty() {
        return Err(String::from("uri argument must not be empty"));
    }
    Ok(String::from(uri))
}

fn parse_position(arguments: &HashMap<String, serde_json::Value>) -> Result<usize, String> {
    let position_value = arguments
        .get("position")
        .ok_or_else(|| String::from("rename-symbol operation requires 'position' argument"))?;
    let position_string = json_value_to_string(position_value)
        .ok_or_else(|| String::from("position argument must be a string or number"))?;
    position_string
        .parse::<usize>()
        .map_err(|error| format!("position must be a non-negative integer: {error}"))
}

fn parse_new_name(arguments: &HashMap<String, serde_json::Value>) -> Result<String, String> {
    let new_name_value = arguments
        .get("new_name")
        .ok_or_else(|| String::from("rename-symbol operation requires 'new_name' argument"))?;
    let new_name = new_name_value
        .as_str()
        .ok_or_else(|| String::from("new_name argument must be a string"))?;
    if new_name.trim().is_empty() {
        return Err(String::from("new_name argument must not be empty"));
    }
    Ok(String::from(new_name))
}

fn json_value_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.to_owned()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}
