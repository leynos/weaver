//! Types representing file edits and modifications.
//!
//! These types form the input to the Double-Lock safety harness. External tools
//! produce edits that are captured here before being validated and applied.

use std::path::{Path, PathBuf};

/// A position within a text file.
///
/// Uses zero-based line and column offsets. Column offsets count UTF-8 bytes,
/// matching the convention used by the Language Server Protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Line number (zero-based).
    pub line: u32,
    /// Column offset (zero-based, UTF-8 bytes).
    pub column: u32,
}

impl Position {
    /// Creates a new position.
    #[must_use]
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// A range within a text file, defined by start and end positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    /// Start of the range (inclusive).
    pub start: Position,
    /// End of the range (exclusive).
    pub end: Position,
}

impl TextRange {
    /// Creates a new range from start to end.
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Creates a zero-length range at the given position.
    #[must_use]
    pub const fn point(position: Position) -> Self {
        Self {
            start: position,
            end: position,
        }
    }
}

/// Newtype wrapper for replacement text in edits.
///
/// This type reduces primitive obsession by providing a dedicated type for
/// text that replaces a range in a file. It provides ergonomic conversions
/// from strings while making the domain intent explicit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementText(String);

impl ReplacementText {
    /// Creates a new replacement text from a string.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Creates an empty replacement text (for deletions).
    #[must_use]
    pub fn empty() -> Self {
        Self(String::new())
    }

    /// Returns the replacement text as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper and returns the inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for ReplacementText {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ReplacementText {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for ReplacementText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A single text replacement within a file.
///
/// Range values use zero-based line and column offsets. Column offsets count
/// UTF-8 bytes, matching the convention used by the Language Server Protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Range being replaced.
    range: TextRange,
    /// Replacement text.
    new_text: String,
}

impl TextEdit {
    /// Builds a text edit from a range and replacement text.
    ///
    /// This is the core constructor. All other constructors delegate to this.
    #[must_use]
    pub fn new(range: TextRange, new_text: impl Into<String>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }

    /// Builds a text edit from Position types and replacement text.
    ///
    /// This constructor uses the parameter object pattern, accepting [`Position`]
    /// objects instead of primitive coordinates.
    #[must_use]
    pub fn from_positions(start: Position, end: Position, new_text: impl Into<String>) -> Self {
        Self::new(TextRange::new(start, end), new_text)
    }

    /// Creates an insertion at the specified position.
    ///
    /// An insertion is a zero-length replacement (start == end) with non-empty text.
    #[must_use]
    pub fn insert_at(position: Position, new_text: impl Into<String>) -> Self {
        Self::new(TextRange::point(position), new_text)
    }

    /// Creates a deletion spanning the given range.
    ///
    /// A deletion is a replacement with empty text.
    #[must_use]
    pub fn delete_range(start: Position, end: Position) -> Self {
        Self::new(TextRange::new(start, end), String::new())
    }

    /// Starting line (zero-based).
    #[must_use]
    pub const fn start_line(&self) -> u32 {
        self.range.start.line
    }

    /// Starting column (zero-based, UTF-8 bytes).
    #[must_use]
    pub const fn start_column(&self) -> u32 {
        self.range.start.column
    }

    /// Ending line (zero-based).
    #[must_use]
    pub const fn end_line(&self) -> u32 {
        self.range.end.line
    }

    /// Ending column (zero-based, UTF-8 bytes).
    #[must_use]
    pub const fn end_column(&self) -> u32 {
        self.range.end.column
    }

    /// Replacement text.
    #[must_use]
    pub fn new_text(&self) -> &str {
        &self.new_text
    }
}

/// A collection of edits for a single file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEdit {
    /// Path to the file being edited.
    path: PathBuf,
    /// Edits to apply, sorted by position.
    edits: Vec<TextEdit>,
}

impl FileEdit {
    /// Creates a new file edit with no changes.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            edits: Vec::new(),
        }
    }

    /// Adds a text edit to this file.
    pub fn add_edit(&mut self, edit: TextEdit) {
        self.edits.push(edit);
    }

    /// Builds a file edit from an existing collection of edits.
    #[must_use]
    pub fn with_edits(path: PathBuf, edits: Vec<TextEdit>) -> Self {
        Self { path, edits }
    }

    /// Path to the file being edited.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Edits to apply.
    #[must_use]
    pub fn edits(&self) -> &[TextEdit] {
        &self.edits
    }

    /// Returns true when no edits are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_edit_insert_is_zero_length() {
        let edit = TextEdit::insert_at(Position::new(5, 10), "hello");
        assert_eq!(edit.start_line(), 5);
        assert_eq!(edit.start_column(), 10);
        assert_eq!(edit.end_line(), 5);
        assert_eq!(edit.end_column(), 10);
        assert_eq!(edit.new_text(), "hello");
    }

    #[test]
    fn text_edit_delete_has_empty_replacement() {
        let edit = TextEdit::delete_range(Position::new(1, 0), Position::new(3, 5));
        assert_eq!(edit.start_line(), 1);
        assert_eq!(edit.start_column(), 0);
        assert_eq!(edit.end_line(), 3);
        assert_eq!(edit.end_column(), 5);
        assert!(edit.new_text().is_empty());
    }

    #[test]
    fn file_edit_tracks_path_and_edits() {
        let path = PathBuf::from("/project/src/main.rs");
        let mut file_edit = FileEdit::new(path.clone());
        assert!(file_edit.is_empty());

        file_edit.add_edit(TextEdit::insert_at(Position::new(0, 0), "// header\n"));
        assert!(!file_edit.is_empty());
        assert_eq!(file_edit.path(), &path);
        assert_eq!(file_edit.edits().len(), 1);
    }
}
