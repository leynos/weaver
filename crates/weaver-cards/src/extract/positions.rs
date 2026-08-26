//! Position and coordinate helpers for source text.

use super::CardExtractionError;

/// Converts a `usize` to `u32`, saturating at [`u32::MAX`] for files larger
/// than 4 GiB, an edge case that is unreachable in normal operation.
pub(super) fn usize_to_u32(value: usize) -> u32 { u32::try_from(value).unwrap_or(u32::MAX) }

/// Converts a one-based line/column request into a byte offset into `source`.
///
/// Columns are counted in `char`s, not bytes, so multi-byte characters before
/// the target column are accounted for correctly. A column one past the last
/// character on a line is accepted, since that is where an inserted symbol
/// would begin.
///
/// # Errors
///
/// Returns [`CardExtractionError::PositionOutOfRange`] when `line` or
/// `column` is zero, when `line` exceeds the number of lines in `source`, or
/// when `column` is beyond the end of the requested line.
pub(super) fn position_to_byte(
    source: &str,
    line: u32,
    column: u32,
) -> Result<usize, CardExtractionError> {
    if line == 0 || column == 0 {
        return Err(CardExtractionError::PositionOutOfRange { line, column });
    }

    let Some((line_start, target_line)) = line_entry(source, line) else {
        return Err(CardExtractionError::PositionOutOfRange { line, column });
    };
    let visible_line = trim_line_ending(target_line);
    if column as usize > visible_line.chars().count().saturating_add(1) {
        return Err(CardExtractionError::PositionOutOfRange { line, column });
    }

    let column_offset = visible_line
        .char_indices()
        .nth((column - 1) as usize)
        .map_or(visible_line.len(), |(offset, _)| offset);
    Ok(line_start.saturating_add(column_offset))
}

/// Locates the one-based `target_line` in `source`, returning its starting
/// byte offset and its slice including the trailing line terminator, if any.
/// Returns `None` when `target_line` exceeds the number of lines present.
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

/// Strips a trailing `\r\n` or `\n` line terminator, so column counts match
/// visible characters rather than including the terminator itself.
fn trim_line_ending(line: &str) -> &str {
    let without_newline = line.strip_suffix('\n').unwrap_or(line);
    without_newline
        .strip_suffix('\r')
        .unwrap_or(without_newline)
}
