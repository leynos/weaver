//! Source span and position types for locating code regions.

use serde::{Deserialize, Serialize};

/// A line and column position within a source file.
///
/// Both fields are zero-indexed to match Tree-sitter conventions.
///
/// # Example
///
/// ```
/// use sempai_core::LineCol;
///
/// let pos = LineCol::new(10, 4);
/// assert_eq!(pos.line(), 10);
/// assert_eq!(pos.column(), 4);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineCol {
    /// Zero-indexed line number.
    pub line: u32,
    /// Zero-indexed column number (byte offset within the line).
    pub column: u32,
}

impl LineCol {
    /// Creates a new line/column position.
    #[must_use]
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }

    /// Returns the zero-indexed line number.
    #[must_use]
    pub const fn line(&self) -> u32 {
        self.line
    }

    /// Returns the zero-indexed column number.
    #[must_use]
    pub const fn column(&self) -> u32 {
        self.column
    }
}

/// A byte and line/column span in a UTF-8 source.
///
/// The byte range is half-open: `start_byte` is inclusive and `end_byte` is
/// exclusive.  The `start` and `end` positions provide human-readable
/// line/column equivalents.
///
/// # Example
///
/// ```
/// use sempai_core::{LineCol, Span};
///
/// let span = Span::new(10, 42, LineCol::new(2, 0), LineCol::new(4, 0));
/// assert_eq!(span.start_byte(), 10);
/// assert_eq!(span.end_byte(), 42);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start_byte: u32,
    /// End byte offset (exclusive).
    pub end_byte: u32,
    /// Start position as line and column.
    pub start: LineCol,
    /// End position as line and column.
    pub end: LineCol,
}

impl Span {
    /// Creates a new span from byte offsets and line/column positions.
    #[must_use]
    pub const fn new(start_byte: u32, end_byte: u32, start: LineCol, end: LineCol) -> Self {
        Self {
            start_byte,
            end_byte,
            start,
            end,
        }
    }

    /// Returns the inclusive start byte offset.
    #[must_use]
    pub const fn start_byte(&self) -> u32 {
        self.start_byte
    }

    /// Returns the exclusive end byte offset.
    #[must_use]
    pub const fn end_byte(&self) -> u32 {
        self.end_byte
    }

    /// Returns the start line/column position.
    #[must_use]
    pub const fn start(&self) -> &LineCol {
        &self.start
    }

    /// Returns the end line/column position.
    #[must_use]
    pub const fn end(&self) -> &LineCol {
        &self.end
    }
}
