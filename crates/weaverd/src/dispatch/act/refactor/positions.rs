//! Source position conversion helpers for `act refactor`.
//!
//! The CLI accepts human-facing `LINE:COL` positions, while current rename
//! actuators still consume UTF-8 byte offsets inside the shared plugin request.

use crate::dispatch::errors::DispatchError;

/// Parses a one-indexed `LINE:COL` value.
pub(super) fn parse_line_col(value: &str) -> Result<(u32, u32), DispatchError> {
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

    Ok((line, column))
}

/// Converts a one-indexed line and Unicode-character column into a byte offset.
pub(super) fn line_col_to_byte_offset(
    content: &str,
    line: u32,
    column: u32,
) -> Result<usize, DispatchError> {
    let Some((line_start, target_line)) = line_entry(content, line) else {
        return Err(position_out_of_range(line, column));
    };
    let visible_line = trim_line_ending(target_line);
    if column as usize > visible_line.chars().count().saturating_add(1) {
        return Err(position_out_of_range(line, column));
    }

    let column_offset = visible_line
        .char_indices()
        .nth((column - 1) as usize)
        .map_or(visible_line.len(), |(offset, _)| offset);
    Ok(line_start.saturating_add(column_offset))
}

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

fn trim_line_ending(line: &str) -> &str {
    let without_newline = line.strip_suffix('\n').unwrap_or(line);
    without_newline
        .strip_suffix('\r')
        .unwrap_or(without_newline)
}

fn position_out_of_range(line: u32, column: u32) -> DispatchError {
    DispatchError::invalid_arguments(format!(
        "position {line}:{column} is out of range for the target file"
    ))
}

#[cfg(test)]
mod tests {
    //! Unit tests for refactor source position conversion.

    use rstest::rstest;

    use super::{line_col_to_byte_offset, parse_line_col};
    use crate::dispatch::errors::DispatchError;

    fn invalid_arguments_message(error: DispatchError) -> String {
        match error {
            DispatchError::InvalidArguments { message } => message,
            other => panic!("expected invalid arguments error, got: {other:?}"),
        }
    }

    #[rstest]
    #[case::start("1:1", (1, 1))]
    #[case::middle("12:34", (12, 34))]
    fn parse_line_col_accepts_valid_values(#[case] value: &str, #[case] expected: (u32, u32)) {
        assert_eq!(parse_line_col(value).expect("position parses"), expected);
    }

    #[rstest]
    #[case::missing_colon("1", "LINE:COL")]
    #[case::non_numeric_line("x:1", "invalid line number")]
    #[case::non_numeric_column("1:x", "invalid column number")]
    #[case::zero_line("0:1", "line number must be >= 1")]
    #[case::zero_column("1:0", "column number must be >= 1")]
    fn parse_line_col_rejects_invalid_values(#[case] value: &str, #[case] expected_message: &str) {
        let message =
            invalid_arguments_message(parse_line_col(value).expect_err("position should fail"));

        assert!(message.contains(expected_message), "{message}");
    }

    #[rstest]
    #[case::ascii_start("hello\n", 1, 1, 0)]
    #[case::ascii_middle("hello\n", 1, 5, 4)]
    #[case::ascii_end("hello\n", 1, 6, 5)]
    #[case::second_line("hello\nworld\n", 2, 2, 7)]
    #[case::multibyte_middle("héllo\n", 1, 3, 3)]
    #[case::multibyte_end("héllo\n", 1, 6, 6)]
    #[case::empty_line("hello\n\nworld\n", 2, 1, 6)]
    #[case::crlf_line("hello\r\nworld\r\n", 2, 1, 7)]
    fn line_col_to_byte_offset_accepts_valid_positions(
        #[case] content: &str,
        #[case] line: u32,
        #[case] column: u32,
        #[case] expected: usize,
    ) {
        assert_eq!(
            line_col_to_byte_offset(content, line, column).expect("position converts"),
            expected
        );
    }

    #[rstest]
    #[case::line_past_end("hello\n", 2, 1)]
    #[case::column_past_end("hello\n", 1, 7)]
    #[case::empty_line_column_past_end("hello\n\nworld\n", 2, 2)]
    fn line_col_to_byte_offset_rejects_out_of_range_positions(
        #[case] content: &str,
        #[case] line: u32,
        #[case] column: u32,
    ) {
        let message = invalid_arguments_message(
            line_col_to_byte_offset(content, line, column).expect_err("position should fail"),
        );

        assert!(message.contains("out of range"), "{message}");
    }
}
