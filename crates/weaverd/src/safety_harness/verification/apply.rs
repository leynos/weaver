//! Text edit application utilities.
//!
//! This module provides functions to apply text edits to file content, handling
//! both LF and CRLF line endings correctly.

use std::path::Path;

use crate::safety_harness::edit::FileEdit;
use crate::safety_harness::error::SafetyHarnessError;

/// Applies text edits to the original content to produce modified content.
///
/// Edits are applied in reverse order (from end of file to start) to avoid
/// invalidating positions as text is inserted or deleted.
///
/// Handles both LF (`\n`) and CRLF (`\r\n`) line endings correctly by computing
/// byte offsets from the original content rather than assuming fixed newline lengths.
pub fn apply_edits(original: &str, file_edit: &FileEdit) -> Result<String, SafetyHarnessError> {
    let line_starts = compute_line_start_offsets(original);
    let mut result = original.to_string();

    // Sort edits in reverse order by position to avoid offset shifts
    let mut edits: Vec<_> = file_edit.edits().iter().collect();
    edits.sort_by(|a, b| {
        b.start_line()
            .cmp(&a.start_line())
            .then_with(|| b.start_column().cmp(&a.start_column()))
    });

    for edit in edits {
        let start_offset = line_column_to_offset(
            &line_starts,
            original,
            edit.start_line(),
            edit.start_column(),
        )
        .ok_or_else(|| edit_error(file_edit.path(), edit.start_line(), edit.start_column()))?;

        let end_offset =
            line_column_to_offset(&line_starts, original, edit.end_line(), edit.end_column())
                .ok_or_else(|| edit_error(file_edit.path(), edit.end_line(), edit.end_column()))?;

        result.replace_range(start_offset..end_offset, edit.new_text());
    }

    Ok(result)
}

/// Creates an edit application error for an invalid position.
fn edit_error(path: &Path, line: u32, column: u32) -> SafetyHarnessError {
    SafetyHarnessError::EditApplicationError {
        path: path.to_path_buf(),
        message: format!("invalid position: line {line}, column {column}"),
    }
}

/// Computes the byte offset of each line start in the original content.
///
/// Handles both LF (`\n`) and CRLF (`\r\n`) line endings by scanning for actual
/// newline positions rather than assuming fixed lengths.
fn compute_line_start_offsets(content: &str) -> Vec<usize> {
    let mut offsets = vec![0]; // Line 0 starts at byte 0
    for (idx, byte) in content.bytes().enumerate() {
        if byte == b'\n' {
            offsets.push(idx + 1); // Next line starts after the newline
        }
    }
    offsets
}

/// Converts a line and column pair to a byte offset in the original text.
///
/// Uses pre-computed line start offsets for correct handling of any newline style.
fn line_column_to_offset(
    line_starts: &[usize],
    content: &str,
    line: u32,
    column: u32,
) -> Option<usize> {
    let line_idx = line as usize;
    let col_offset = column as usize;

    // Get the byte offset where this line starts
    let line_start = *line_starts.get(line_idx)?;

    // Calculate line length to validate column
    let line_end = line_starts
        .get(line_idx + 1)
        .copied()
        .unwrap_or(content.len());

    // Calculate content length (excluding newline characters)
    let line_content_end = if line_end > 0 && content.as_bytes().get(line_end - 1) == Some(&b'\n') {
        if line_end > 1 && content.as_bytes().get(line_end - 2) == Some(&b'\r') {
            line_end - 2 // CRLF
        } else {
            line_end - 1 // LF
        }
    } else {
        line_end // Last line without trailing newline
    };

    let line_len = line_content_end.saturating_sub(line_start);

    // Allow column to be at most line_len (for end-of-line positions)
    if col_offset > line_len {
        return None;
    }

    line_start.checked_add(col_offset)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::safety_harness::edit::{FileEdit, Position, TextEdit};

    /// Helper for testing successful edit application scenarios.
    fn assert_edits_produce(original: &str, edits: Vec<TextEdit>, expected: &str) {
        let path = PathBuf::from("test.txt");
        let edit = FileEdit::with_edits(path, edits);
        let result = apply_edits(original, &edit).expect("edit should succeed");
        assert_eq!(result, expected);
    }

    #[test]
    fn apply_edits_inserts_text() {
        assert_edits_produce(
            "hello world",
            vec![TextEdit::insert_at(Position::new(0, 6), "beautiful ")],
            "hello beautiful world",
        );
    }

    #[test]
    fn apply_edits_deletes_text() {
        assert_edits_produce(
            "hello beautiful world",
            vec![TextEdit::delete_range(
                Position::new(0, 6),
                Position::new(0, 16),
            )],
            "hello world",
        );
    }

    #[test]
    fn apply_edits_replaces_text() {
        assert_edits_produce(
            "fn foo() {}",
            vec![TextEdit::from_positions(
                Position::new(0, 3),
                Position::new(0, 6),
                "bar".to_string(),
            )],
            "fn bar() {}",
        );
    }

    #[test]
    fn apply_edits_handles_multiple_edits() {
        assert_edits_produce(
            "aaa bbb ccc",
            vec![
                TextEdit::from_positions(
                    Position::new(0, 0),
                    Position::new(0, 3),
                    "AAA".to_string(),
                ),
                TextEdit::from_positions(
                    Position::new(0, 8),
                    Position::new(0, 11),
                    "CCC".to_string(),
                ),
            ],
            "AAA bbb CCC",
        );
    }

    #[test]
    fn apply_edits_handles_crlf_line_endings() {
        // CRLF line endings: each \r\n is 2 bytes
        // Replace "two" on line 1 (zero-indexed)
        assert_edits_produce(
            "line one\r\nline two\r\nline three",
            vec![TextEdit::from_positions(
                Position::new(1, 5),
                Position::new(1, 8),
                "TWO".to_string(),
            )],
            "line one\r\nline TWO\r\nline three",
        );
    }

    #[test]
    fn apply_edits_handles_mixed_multiline_with_lf() {
        // LF line endings
        // Replace "second" on line 1
        assert_edits_produce(
            "first\nsecond\nthird",
            vec![TextEdit::from_positions(
                Position::new(1, 0),
                Position::new(1, 6),
                "SECOND".to_string(),
            )],
            "first\nSECOND\nthird",
        );
    }

    #[test]
    fn apply_edits_rejects_past_eof_line() {
        // Single line without trailing newline
        let original = "hello";
        let path = PathBuf::from("test.txt");

        // Try to insert at line 1 (past EOF on a file with no trailing newline)
        let edit = FileEdit::with_edits(
            path,
            vec![TextEdit::insert_at(Position::new(1, 0), "world")],
        );
        let result = apply_edits(original, &edit);
        assert!(result.is_err(), "should reject past-EOF line");
    }

    #[test]
    fn apply_edits_allows_end_of_file_position() {
        // Single line without trailing newline
        // Insert at end of line 0 (column 5, after "hello")
        assert_edits_produce(
            "hello",
            vec![TextEdit::insert_at(Position::new(0, 5), " world")],
            "hello world",
        );
    }
}
