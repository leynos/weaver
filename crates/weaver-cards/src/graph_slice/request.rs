//! Request types for the `observe graph-slice` operation.
//!
//! The [`GraphSliceRequest`] struct captures the parsed arguments from a
//! `graph-slice` command. It provides a [`parse`](GraphSliceRequest::parse)
//! constructor that accepts the raw argument vector from the daemon's
//! `CommandRequest` and normalizes all optional flags to their documented
//! defaults.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::DetailLevel;

use super::budget::SliceBudget;
use super::parse::RequestBuilder;

/// Default traversal depth.
pub const DEFAULT_DEPTH: u32 = 2;

/// Default minimum confidence threshold.
pub const DEFAULT_MIN_CONFIDENCE: f64 = 0.5;

/// Traversal direction for graph-slice exploration.
///
/// # Example
///
/// ```
/// use weaver_cards::SliceDirection;
///
/// let dir = SliceDirection::default();
/// assert_eq!(dir, SliceDirection::Both);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SliceDirection {
    /// Follow only incoming edges (callers, importers).
    In,
    /// Follow only outgoing edges (callees, imports).
    Out,
    /// Follow edges in both directions.
    #[default]
    Both,
}

/// Error returned when a string does not match a known variant of a slice enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceParseError {
    kind: &'static str,
    expected: &'static str,
    name: String,
}

impl fmt::Display for SliceParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown {}: {}; expected one of: {}",
            self.kind, self.name, self.expected
        )
    }
}

impl std::error::Error for SliceParseError {}

/// Maps a string token to an enum variant using a static lookup table.
///
/// `variants` is an ordered slice of `(token, variant)` pairs.
/// Returns `Err(SliceParseError)` if no token matches `s`.
pub(super) fn parse_variant<T: Copy>(
    s: &str,
    variants: &[(&str, T)],
    kind: &'static str,
    expected: &'static str,
) -> Result<T, SliceParseError> {
    variants
        .iter()
        .find(|(name, _)| *name == s)
        .map(|(_, v)| *v)
        .ok_or_else(|| SliceParseError {
            kind,
            expected,
            name: String::from(s),
        })
}

impl FromStr for SliceDirection {
    type Err = SliceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_variant(
            s,
            &[("in", Self::In), ("out", Self::Out), ("both", Self::Both)],
            "direction",
            "in, out, both",
        )
    }
}

/// Edge type filter for graph-slice traversal.
///
/// Variants are ordered canonically: `call`, `import`, `config`.
/// When serialized, the canonical ordering is preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SliceEdgeType {
    /// A function or method call relationship.
    Call,
    /// A module or package import dependency.
    Import,
    /// A configuration key or feature flag dependency.
    Config,
}

impl FromStr for SliceEdgeType {
    type Err = SliceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_variant(
            s,
            &[
                ("call", Self::Call),
                ("import", Self::Import),
                ("config", Self::Config),
            ],
            "edge type",
            "call, import, config",
        )
    }
}

impl SliceEdgeType {
    /// Returns the canonical rank of this edge type for deterministic ordering.
    ///
    /// The canonical order is: Call (0), Import (1), Config (2).
    #[must_use]
    pub const fn canonical_rank(self) -> u8 {
        match self {
            Self::Call => 0,
            Self::Import => 1,
            Self::Config => 2,
        }
    }

    /// Returns all edge types in canonical order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Call, Self::Import, Self::Config]
    }
}

/// Errors that can occur during `graph-slice` request parsing.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GraphSliceError {
    /// A required argument is missing.
    #[error("missing required argument: {flag}")]
    MissingArgument {
        /// The flag that was expected.
        flag: String,
    },
    /// An argument value is malformed.
    #[error("invalid argument value for {flag}: {message}")]
    InvalidValue {
        /// The flag whose value was invalid.
        flag: String,
        /// Description of the problem.
        message: String,
    },
    /// An unknown positional argument was provided.
    #[error("unknown argument: {argument}")]
    UnknownArgument {
        /// The unrecognised argument.
        argument: String,
    },
    /// An unknown flag was provided.
    #[error("unknown flag: {flag}")]
    UnknownFlag {
        /// The unrecognised flag.
        flag: String,
    },
}

/// Parsed request for the `observe graph-slice` operation.
///
/// All optional fields carry explicit defaults. When serialized, the
/// normalized request is fully populated so that downstream consumers
/// can observe which defaults were applied.
///
/// # Example
///
/// ```
/// use weaver_cards::{GraphSliceRequest, SliceDirection};
///
/// let args = vec![
///     String::from("--uri"), String::from("file:///src/main.rs"),
///     String::from("--position"), String::from("10:5"),
/// ];
/// let request = GraphSliceRequest::parse(&args).expect("valid request");
/// assert_eq!(request.depth(), 2);
/// assert_eq!(request.direction(), SliceDirection::Both);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphSliceRequest {
    /// File URI (e.g. `file:///src/main.rs`).
    pub(super) uri: String,
    /// Line number (1-indexed, user-facing).
    pub(super) line: u32,
    /// Column number (1-indexed, user-facing).
    pub(super) column: u32,
    /// Traversal depth limit.
    pub(super) depth: u32,
    /// Traversal direction.
    pub(super) direction: SliceDirection,
    /// Edge type filter (canonical order).
    pub(super) edge_types: Vec<SliceEdgeType>,
    /// Minimum confidence threshold.
    pub(super) min_confidence: f64,
    /// Budget constraints.
    pub(super) budget: SliceBudget,
    /// Detail level for the entry card.
    pub(super) entry_detail: DetailLevel,
    /// Detail level for non-entry cards.
    pub(super) node_detail: DetailLevel,
}

impl GraphSliceRequest {
    /// Parses a `graph-slice` request from a CLI argument list.
    ///
    /// Expects `--uri <URI> --position <LINE:COL>` format with optional
    /// traversal, budget, and detail flags. Arguments can appear in any
    /// order. `--uri` and `--position` are required.
    ///
    /// # Errors
    ///
    /// Returns [`GraphSliceError`] if required flags are missing, values
    /// are malformed, or a non-flag positional token is encountered.
    pub fn parse(arguments: &[String]) -> Result<Self, GraphSliceError> {
        let mut builder = RequestBuilder::default();

        let mut iter = arguments.iter().peekable();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                flag if flag.starts_with("--") => {
                    builder.apply_flag(flag, &mut iter)?;
                }
                other => {
                    return Err(GraphSliceError::UnknownArgument {
                        argument: String::from(other),
                    });
                }
            }
        }

        builder.build()
    }

    /// Returns the file URI.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the 1-indexed line number.
    #[must_use]
    pub const fn line(&self) -> u32 {
        self.line
    }

    /// Returns the 1-indexed column number.
    #[must_use]
    pub const fn column(&self) -> u32 {
        self.column
    }

    /// Returns the traversal depth limit.
    #[must_use]
    pub const fn depth(&self) -> u32 {
        self.depth
    }

    /// Returns the traversal direction.
    #[must_use]
    pub const fn direction(&self) -> SliceDirection {
        self.direction
    }

    /// Returns the edge type filter (in canonical order).
    #[must_use]
    pub fn edge_types(&self) -> &[SliceEdgeType] {
        &self.edge_types
    }

    /// Returns the minimum confidence threshold.
    #[must_use]
    pub const fn min_confidence(&self) -> f64 {
        self.min_confidence
    }

    /// Returns the budget constraints.
    #[must_use]
    pub const fn budget(&self) -> &SliceBudget {
        &self.budget
    }

    /// Returns the entry card detail level.
    #[must_use]
    pub const fn entry_detail(&self) -> DetailLevel {
        self.entry_detail
    }

    /// Returns the non-entry node detail level.
    #[must_use]
    pub const fn node_detail(&self) -> DetailLevel {
        self.node_detail
    }
}
