//! Source position conversion helpers for `act refactor`.
//!
//! The CLI accepts human-facing `LINE:COL` positions, while current rename
//! actuators still consume UTF-8 byte offsets inside the shared plugin request.

use std::path::Path;

use crate::dispatch::errors::DispatchError;

/// A validated, one-indexed line and Unicode-character column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LineCol {
    pub line: u32,
    pub column: u32,
}

/// Parses a one-indexed `LINE:COL` value.
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
#[tracing::instrument(level = "debug", skip(content), fields(content_len = content.len()))]
pub(super) fn line_col_to_byte_offset(
    content: &str,
    line: u32,
    column: u32,
    file_path: Option<&Path>,
) -> Result<usize, DispatchError> {
    let Some((line_start, target_line)) = line_entry(content, line) else {
        return Err(conversion_out_of_range(line, column, file_path));
    };
    let visible_line = trim_line_ending(target_line);
    if column as usize > visible_line.chars().count().saturating_add(1) {
        return Err(conversion_out_of_range(line, column, file_path));
    }

    let column_offset = visible_line
        .char_indices()
        .nth((column - 1) as usize)
        .map_or(visible_line.len(), |(offset, _)| offset);
    let offset = line_start.saturating_add(column_offset);
    tracing::debug!("resolved position {line}:{column} to byte offset {offset}");
    Ok(offset)
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

fn conversion_out_of_range(line: u32, column: u32, file_path: Option<&Path>) -> DispatchError {
    position_out_of_range(line, column, file_path)
}

fn position_out_of_range(line: u32, column: u32, file_path: Option<&Path>) -> DispatchError {
    let file_context = file_path
        .map(|path| format!(" '{}'", path.display()))
        .unwrap_or_default();
    DispatchError::invalid_arguments(format!(
        "position {line}:{column} is out of range for the target file{file_context}"
    ))
}

#[cfg(test)]
mod tests {
    //! Unit tests for refactor source position conversion.

    use rstest::rstest;

    use super::{LineCol, line_col_to_byte_offset, parse_line_col};
    use crate::dispatch::errors::DispatchError;

    fn invalid_arguments_message(error: DispatchError) -> String {
        match error {
            DispatchError::InvalidArguments { message } => message,
            other => panic!("expected invalid arguments error, got: {other:?}"),
        }
    }

    #[rstest]
    #[case::start("1:1", LineCol { line: 1, column: 1 })]
    #[case::middle("12:34", LineCol { line: 12, column: 34 })]
    fn parse_line_col_accepts_valid_values(#[case] value: &str, #[case] expected: LineCol) {
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
            line_col_to_byte_offset(content, line, column, None).expect("position converts"),
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
            line_col_to_byte_offset(content, line, column, None).expect_err("position should fail"),
        );

        assert!(message.contains("out of range"), "{message}");
    }

    mod counter_tests {
        //! Counter tests for command-side position metrics.

        use std::cell::Cell;

        use super::super::{DispatchError, LineCol, line_col_to_byte_offset, parse_line_col};
        use crate::dispatch::act::refactor::metrics::PositionMetrics;

        #[derive(Default)]
        struct CountingPositionMetrics {
            parse_errors: Cell<u64>,
            conversion_errors: Cell<u64>,
        }

        impl PositionMetrics for CountingPositionMetrics {
            fn increment_parse_error(&self) {
                self.parse_errors
                    .set(self.parse_errors.get().saturating_add(1));
            }

            fn increment_conversion_error(&self) {
                self.conversion_errors
                    .set(self.conversion_errors.get().saturating_add(1));
            }
        }

        fn parse_with_metrics(
            value: &str,
            metrics: &dyn PositionMetrics,
        ) -> Result<LineCol, DispatchError> {
            parse_line_col(value).inspect_err(|_error| {
                metrics.increment_parse_error();
            })
        }

        fn convert_with_metrics(
            content: &str,
            line: u32,
            column: u32,
            metrics: &dyn PositionMetrics,
        ) -> Result<usize, DispatchError> {
            line_col_to_byte_offset(content, line, column, None).inspect_err(|_error| {
                metrics.increment_conversion_error();
            })
        }

        #[test]
        fn parse_success_does_not_increment_parse_counter() {
            let metrics = CountingPositionMetrics::default();

            let _ = parse_with_metrics("1:1", &metrics).expect("position parses");

            assert_eq!(metrics.parse_errors.get(), 0);
        }

        #[test]
        fn parse_failure_increments_parse_counter_once() {
            let metrics = CountingPositionMetrics::default();

            let _ = parse_with_metrics("1", &metrics).expect_err("position should fail");

            assert_eq!(metrics.parse_errors.get(), 1);
        }

        #[test]
        fn conversion_success_does_not_increment_conversion_counter() {
            let metrics = CountingPositionMetrics::default();

            let _ = convert_with_metrics("hello\n", 1, 1, &metrics).expect("position converts");

            assert_eq!(metrics.conversion_errors.get(), 0);
        }

        #[test]
        fn conversion_failure_increments_conversion_counter_once() {
            let metrics = CountingPositionMetrics::default();

            let _ =
                convert_with_metrics("hello\n", 2, 1, &metrics).expect_err("position should fail");

            assert_eq!(metrics.conversion_errors.get(), 1);
        }
    }

    mod property_tests {
        //! Property tests for source position parsing and conversion.

        use proptest::{collection::vec, prelude::*};

        use super::{LineCol, line_col_to_byte_offset, parse_line_col};

        fn ascii_line_strategy() -> impl Strategy<Value = String> {
            vec(0x20u8..=0x7eu8, 0..=128)
                .prop_map(|bytes| bytes.into_iter().map(char::from).collect())
        }

        fn ascii_lines_strategy() -> impl Strategy<Value = Vec<String>> {
            vec(ascii_line_strategy(), 1..=16)
        }

        fn newline_terminated_content_strategy() -> impl Strategy<Value = String> {
            ascii_lines_strategy().prop_map(|lines| {
                let mut content = lines.join("\n");
                content.push('\n');
                content
            })
        }

        fn unicode_line_strategy() -> impl Strategy<Value = String> {
            vec(any::<char>(), 0..=128).prop_map(|chars| chars.into_iter().collect())
        }

        fn unicode_content_strategy() -> impl Strategy<Value = String> {
            vec(unicode_line_strategy(), 1..=8).prop_map(|lines| {
                let mut content = lines.join("\n");
                content.push('\n');
                content
            })
        }

        fn line_start(content: &str, line: u32) -> usize {
            content
                .split_inclusive('\n')
                .take(line.saturating_sub(1) as usize)
                .map(str::len)
                .sum()
        }

        proptest! {
            #[test]
            fn line_col_to_byte_offset_returns_char_boundary_for_valid_ascii_positions(
                content in newline_terminated_content_strategy(),
                line_index in 0usize..16,
                column_index in 0usize..129,
            ) {
                let lines: Vec<&str> = content.split_inclusive('\n').collect();
                let actual_line_index = line_index % lines.len();
                let visible_line = lines[actual_line_index].trim_end_matches('\n');
                let column_count = visible_line.chars().count() + 1;
                let line = (actual_line_index + 1) as u32;
                let column = (column_index % column_count + 1) as u32;

                let offset = line_col_to_byte_offset(&content, line, column, None)
                    .expect("generated position is valid");

                prop_assert!(content.is_char_boundary(offset));
            }

            #[test]
            fn line_col_to_byte_offset_is_monotonic_across_ascii_columns(
                line in ascii_line_strategy(),
            ) {
                let content = format!("{line}\n");
                let mut previous = 0usize;

                for column in 1..=(line.len() + 1) {
                    let offset = line_col_to_byte_offset(&content, 1, column as u32, None)
                        .expect("column is valid for generated line");
                    prop_assert!(offset >= previous);
                    previous = offset;
                }
            }

            #[test]
            fn line_col_to_byte_offset_rejects_lines_past_terminated_content(
                content in newline_terminated_content_strategy(),
                extra_line in 1u32..=1024,
                column in 1u32..=1024,
            ) {
                let line_count = content.split_inclusive('\n').count() as u32;
                let line = line_count.saturating_add(extra_line);

                prop_assert!(line_col_to_byte_offset(&content, line, column, None).is_err());
            }

            #[test]
            fn line_col_to_byte_offset_has_crlf_lf_parity_relative_to_line_start(
                lines in ascii_lines_strategy(),
                line_index in 0usize..16,
                column_index in 0usize..129,
            ) {
                let actual_line_index = line_index % lines.len();
                let line = (actual_line_index + 1) as u32;
                let visible_line = &lines[actual_line_index];
                let column = (column_index % (visible_line.len() + 1) + 1) as u32;
                let lf_content = format!("{}\n", lines.join("\n"));
                let crlf_content = format!("{}\r\n", lines.join("\r\n"));

                let lf_offset = line_col_to_byte_offset(&lf_content, line, column, None)
                    .expect("LF position is valid");
                let crlf_offset = line_col_to_byte_offset(&crlf_content, line, column, None)
                    .expect("CRLF position is valid");

                prop_assert_eq!(
                    lf_offset - line_start(&lf_content, line),
                    crlf_offset - line_start(&crlf_content, line),
                );
            }

            #[test]
            fn line_col_to_byte_offset_never_panics_for_unicode_content(
                content in unicode_content_strategy(),
                line in any::<u32>(),
                column in any::<u32>(),
            ) {
                let _ = line_col_to_byte_offset(&content, line, column, None);
            }

            #[test]
            fn parse_line_col_round_trips_positive_values(
                line in 1u32..=9999,
                column in 1u32..=9999,
            ) {
                let value = format!("{line}:{column}");

                let parsed = parse_line_col(&value).expect("generated position parses");
                prop_assert_eq!(parsed, LineCol { line, column });
            }
        }
    }
}
