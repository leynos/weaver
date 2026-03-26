//! Fixture catalogue for `observe get-card` snapshot coverage.

mod python;
mod rust;

/// One source fixture plus the cursor position used for `get-card`.
#[derive(Debug, Clone, Copy)]
pub struct CardFixtureCase {
    /// Snapshot-friendly fixture identifier.
    pub name: &'static str,
    /// File name written to the temporary workspace.
    pub file_name: &'static str,
    /// Source text written to disk.
    pub source: &'static str,
    /// One-based line requested from `observe get-card`.
    pub line: u32,
    /// One-based column requested from `observe get-card`.
    pub column: u32,
}

pub use python::PYTHON_CASES;
pub use rust::RUST_CASES;
