//! Shared position conversion helpers.
//!
//! Tree-sitter positions are zero-based. For user-facing messages, we prefer
//! one-based line and column numbers.

/// Converts a Tree-sitter position (0-based) to one-based display coordinates.
#[must_use]
pub(crate) fn point_to_one_based(pos: tree_sitter::Point) -> (u32, u32) {
    // Line/column numbers will realistically never exceed u32::MAX.
    let line = u32::try_from(pos.row.saturating_add(1)).unwrap_or(u32::MAX);
    let column = u32::try_from(pos.column.saturating_add(1)).unwrap_or(u32::MAX);
    (line, column)
}

