//! Progressive detail levels for symbol card extraction.
//!
//! Cards support partial answers quickly and deeper answers only when needed.
//! Each detail level is a superset of the previous one, and the default
//! (`Structure`) provides high utility without requiring a live LSP server.
//!
//! See `docs/jacquard-card-first-symbol-graph-design.md` §7 for the full
//! detail-level taxonomy and latency expectations.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Progressive detail level for symbol card extraction.
///
/// Each level is a superset of the previous one. `Minimal` returns only
/// identity information, while `Full` includes all available data including
/// dependency edges and fan metrics.
///
/// The default detail level is `Structure`, which provides high utility
/// without requiring a live LSP server.
///
/// # Example
///
/// ```
/// use weaver_cards::DetailLevel;
///
/// let level = DetailLevel::default();
/// assert_eq!(level, DetailLevel::Structure);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DetailLevel {
    /// Identity only: `SymbolRef` and `SymbolId`.
    Minimal,
    /// Adds signature (Tree-sitter extraction).
    Signature,
    /// Adds docstring, attachments, locals, branches, basic metrics.
    /// This is the default.
    #[default]
    Structure,
    /// Adds LSP hover/type information.
    Semantic,
    /// Adds dependency edges and fan-in/out metrics.
    Full,
}

/// Error returned when a string does not match any known detail level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailLevelParseError {
    /// The input string that could not be parsed.
    pub name: String,
}

impl fmt::Display for DetailLevelParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown detail level: {}; expected one of: minimal, signature, structure, semantic, full",
            self.name
        )
    }
}

impl std::error::Error for DetailLevelParseError {}

impl FromStr for DetailLevel {
    type Err = DetailLevelParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "minimal" => Ok(Self::Minimal),
            "signature" => Ok(Self::Signature),
            "structure" => Ok(Self::Structure),
            "semantic" => Ok(Self::Semantic),
            "full" => Ok(Self::Full),
            _ => Err(DetailLevelParseError {
                name: String::from(s),
            }),
        }
    }
}
