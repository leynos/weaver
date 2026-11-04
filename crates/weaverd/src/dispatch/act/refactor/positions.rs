//! Source position conversion helpers for `act refactor`.
//!
//! The CLI accepts human-facing `LINE:COL` positions, while current rename
//! actuators still consume UTF-8 byte offsets inside the shared plugin request.

use std::path::Path;

use crate::dispatch::errors::DispatchError;

/// A validated, one-indexed line and Unicode-character column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LineCol {
    /// One-indexed line number within the source text.
    pub line: u32,
    /// One-indexed Unicode-character column within that line.
    pub column: u32,
}

/// Parses a one-indexed `LINE:COL` value.
#[tracing::instrument(level = "debug", fields(position = %value))]
pub(super) fn parse_line_col(value: &str) -> Result<LineCol, DispatchError> {
    let (line_str, column_str) = value.split_once(':').ok_or_else(|| {
        DispatchError::invalid_arguments(format!("position must be LINE:COL, got: {value}"))
    })?;

    let line: u32 = line_str.parse().map_err(|_| {
        DispatchError::invalid_arguments(format!("invalid line number: {line_str}"))
    })?;
    let column: u32 = column_str.parse().map_err(|_| {
        DispatchError::invalid_arguments(format!("invalid column number: {column_str}"))
    })?;

    if line == 0 {
        return Err(DispatchError::invalid_arguments("line number must be >= 1"));
    }
    if column == 0 {
        return Err(DispatchError::invalid_arguments(
            "column number must be >= 1",
        ));
    }

    Ok(LineCol { line, column })
}

/// Converts a one-indexed line and Unicode-character column into a byte offset.
#[tracing::instrument(
    level = "debug",
    skip(content),
    fields(content_len = content.len(), line, column, file_path = tracing::field::Empty)
)]
pub(super) fn line_col_to_byte_offset(
    content: &str,
    line: u32,
    column: u32,
    file_path: Option<&Path>,
) -> Result<usize, DispatchError> {
    if let Some(target_path) = file_path {
        tracing::Span::current()
            .record("file_path", tracing::field::display(target_path.display()));
    }
    let Some((line_start, target_line)) = line_entry(content, line) else {
        return Err(position_out_of_range(line, column, file_path));
    };
    let visible_line = trim_line_ending(target_line);
    if column as usize > visible_line.chars().count().saturating_add(1) {
        return Err(position_out_of_range(line, column, file_path));
    }

    let column_offset = visible_line
        .char_indices()
        .nth((column - 1) as usize)
        .map_or(visible_line.len(), |(offset, _)| offset);
    let offset = line_start.saturating_add(column_offset);
    tracing::debug!("resolved position {line}:{column} to byte offset {offset}");
    Ok(offset)
}

/// Returns the byte offset and text of the requested one-indexed line.
///
/// Yields `None` when `source` has fewer lines than `target_line`.
fn line_entry(source: &str, target_line: u32) -> Option<(usize, &str)> {
    let mut start = 0usize;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        if index + 1 == target_line as usize {
            return Some((start, line));
        }
        start = start.saturating_add(line.len());
    }
    None
}

/// Removes a trailing `\n` or `\r\n` from a line slice without reallocating.
fn trim_line_ending(line: &str) -> &str {
    let without_newline = line.strip_suffix('\n').unwrap_or(line);
    without_newline
        .strip_suffix('\r')
        .unwrap_or(without_newline)
}

/// Builds the invalid-arguments error for a position outside the target file.
fn position_out_of_range(line: u32, column: u32, file_path: Option<&Path>) -> DispatchError {
    let file_context = file_path
        .map(|path| format!(" '{}'", path.display()))
        .unwrap_or_default();
    DispatchError::invalid_arguments(format!(
        "position {line}:{column} is out of range for the target file{file_context}"
    ))
}
