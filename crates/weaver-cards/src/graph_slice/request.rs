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

/// Error returned when a string does not match any known direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionParseError {
    name: String,
}

impl fmt::Display for DirectionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown direction: {}; expected one of: in, out, both",
            self.name
        )
    }
}

impl std::error::Error for DirectionParseError {}

impl FromStr for SliceDirection {
    type Err = DirectionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "in" => Ok(Self::In),
            "out" => Ok(Self::Out),
            "both" => Ok(Self::Both),
            _ => Err(DirectionParseError {
                name: String::from(s),
            }),
        }
    }
}

/// Edge type filter for graph-slice traversal.
///
/// Variants are ordered canonically: `call`, `import`, `config`.
/// When serialized, the canonical ordering is preserved.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
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

/// Error returned when a string does not match any known edge type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeTypeParseError {
    name: String,
}

impl fmt::Display for EdgeTypeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown edge type: {}; expected one of: call, import, config",
            self.name
        )
    }
}

impl std::error::Error for EdgeTypeParseError {}

impl FromStr for SliceEdgeType {
    type Err = EdgeTypeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "call" => Ok(Self::Call),
            "import" => Ok(Self::Import),
            "config" => Ok(Self::Config),
            _ => Err(EdgeTypeParseError {
                name: String::from(s),
            }),
        }
    }
}

impl SliceEdgeType {
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
    uri: String,
    /// Line number (1-indexed, user-facing).
    line: u32,
    /// Column number (1-indexed, user-facing).
    column: u32,
    /// Traversal depth limit.
    depth: u32,
    /// Traversal direction.
    direction: SliceDirection,
    /// Edge type filter (canonical order).
    edge_types: Vec<SliceEdgeType>,
    /// Minimum confidence threshold.
    min_confidence: f64,
    /// Budget constraints.
    budget: SliceBudget,
    /// Detail level for the entry card.
    entry_detail: DetailLevel,
    /// Detail level for non-entry cards.
    node_detail: DetailLevel,
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
        let mut uri: Option<String> = None;
        let mut position: Option<(u32, u32)> = None;
        let mut depth = DEFAULT_DEPTH;
        let mut direction = SliceDirection::default();
        let mut edge_types: Option<Vec<SliceEdgeType>> = None;
        let mut min_confidence = DEFAULT_MIN_CONFIDENCE;
        let mut budget = SliceBudget::default();
        let mut entry_detail = DetailLevel::Structure;
        let mut node_detail = DetailLevel::Minimal;

        let mut iter = arguments.iter().peekable();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--uri" => {
                    let value = require_arg_value(&mut iter, "--uri")?;
                    uri = Some(String::from(value));
                }
                "--position" => {
                    let value = require_arg_value(&mut iter, "--position")?;
                    position = Some(parse_position(value)?);
                }
                "--depth" => {
                    let value = require_arg_value(&mut iter, "--depth")?;
                    depth = parse_u32(value, "--depth")?;
                }
                "--direction" => {
                    let value = require_arg_value(&mut iter, "--direction")?;
                    direction = parse_direction(value)?;
                }
                "--edge-types" => {
                    let value = require_arg_value(&mut iter, "--edge-types")?;
                    edge_types = Some(parse_edge_types(value)?);
                }
                "--min-confidence" => {
                    let value =
                        require_arg_value(&mut iter, "--min-confidence")?;
                    min_confidence = parse_confidence(value)?;
                }
                "--max-cards" => {
                    let value = require_arg_value(&mut iter, "--max-cards")?;
                    let max_cards = parse_u32(value, "--max-cards")?;
                    budget = SliceBudget::new(
                        max_cards,
                        budget.max_edges(),
                        budget.max_estimated_tokens(),
                    );
                }
                "--max-edges" => {
                    let value = require_arg_value(&mut iter, "--max-edges")?;
                    let max_edges = parse_u32(value, "--max-edges")?;
                    budget = SliceBudget::new(
                        budget.max_cards(),
                        max_edges,
                        budget.max_estimated_tokens(),
                    );
                }
                "--max-estimated-tokens" => {
                    let value =
                        require_arg_value(&mut iter, "--max-estimated-tokens")?;
                    let max_tokens = parse_u32(value, "--max-estimated-tokens")?;
                    budget = SliceBudget::new(
                        budget.max_cards(),
                        budget.max_edges(),
                        max_tokens,
                    );
                }
                "--entry-detail" => {
                    let value =
                        require_arg_value(&mut iter, "--entry-detail")?;
                    entry_detail = parse_detail(value, "--entry-detail")?;
                }
                "--node-detail" => {
                    let value =
                        require_arg_value(&mut iter, "--node-detail")?;
                    node_detail = parse_detail(value, "--node-detail")?;
                }
                other if other.starts_with("--") => {
                    skip_unknown_flag_value(&mut iter);
                }
                other => {
                    return Err(GraphSliceError::UnknownArgument {
                        argument: String::from(other),
                    });
                }
            }
        }

        let resolved_uri =
            uri.ok_or_else(|| GraphSliceError::MissingArgument {
                flag: String::from("--uri"),
            })?;
        let (line, column) =
            position.ok_or_else(|| GraphSliceError::MissingArgument {
                flag: String::from("--position"),
            })?;

        // Normalize edge types into canonical order.
        let mut resolved_types = edge_types.unwrap_or_else(|| {
            SliceEdgeType::all().to_vec()
        });
        resolved_types.sort();
        resolved_types.dedup();

        Ok(Self {
            uri: resolved_uri,
            line,
            column,
            depth,
            direction,
            edge_types: resolved_types,
            min_confidence,
            budget,
            entry_detail,
            node_detail,
        })
    }

    /// Returns the file URI.
    #[must_use]
    pub fn uri(&self) -> &str { &self.uri }

    /// Returns the 1-indexed line number.
    #[must_use]
    pub const fn line(&self) -> u32 { self.line }

    /// Returns the 1-indexed column number.
    #[must_use]
    pub const fn column(&self) -> u32 { self.column }

    /// Returns the traversal depth limit.
    #[must_use]
    pub const fn depth(&self) -> u32 { self.depth }

    /// Returns the traversal direction.
    #[must_use]
    pub const fn direction(&self) -> SliceDirection { self.direction }

    /// Returns the edge type filter (in canonical order).
    #[must_use]
    pub fn edge_types(&self) -> &[SliceEdgeType] { &self.edge_types }

    /// Returns the minimum confidence threshold.
    #[must_use]
    pub const fn min_confidence(&self) -> f64 { self.min_confidence }

    /// Returns the budget constraints.
    #[must_use]
    pub const fn budget(&self) -> &SliceBudget { &self.budget }

    /// Returns the entry card detail level.
    #[must_use]
    pub const fn entry_detail(&self) -> DetailLevel { self.entry_detail }

    /// Returns the non-entry node detail level.
    #[must_use]
    pub const fn node_detail(&self) -> DetailLevel { self.node_detail }
}

// -------------------------------------------------------------------------
// Parsing helpers
// -------------------------------------------------------------------------

fn require_arg_value<'a, I>(
    iter: &mut I,
    flag: &str,
) -> Result<&'a str, GraphSliceError>
where
    I: Iterator<Item = &'a String>,
{
    match iter.next().map(String::as_str) {
        Some(value) if value.starts_with('-') => {
            Err(GraphSliceError::InvalidValue {
                flag: String::from(flag),
                message: String::from("requires a value"),
            })
        }
        Some(value) => Ok(value),
        None => Err(GraphSliceError::InvalidValue {
            flag: String::from(flag),
            message: String::from("requires a value"),
        }),
    }
}

fn skip_unknown_flag_value<'a, I>(iter: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = &'a String>,
{
    let is_value =
        iter.peek().is_some_and(|next| !next.starts_with('-'));
    if is_value {
        iter.next();
    }
}

fn parse_position(value: &str) -> Result<(u32, u32), GraphSliceError> {
    let (line_str, col_str) =
        value
            .split_once(':')
            .ok_or_else(|| GraphSliceError::InvalidValue {
                flag: String::from("--position"),
                message: format!("expected LINE:COL, got: {value}"),
            })?;

    let line: u32 =
        line_str
            .parse()
            .map_err(|_| GraphSliceError::InvalidValue {
                flag: String::from("--position"),
                message: format!("invalid line number: {line_str}"),
            })?;
    let column: u32 =
        col_str
            .parse()
            .map_err(|_| GraphSliceError::InvalidValue {
                flag: String::from("--position"),
                message: format!("invalid column number: {col_str}"),
            })?;

    if line == 0 {
        return Err(GraphSliceError::InvalidValue {
            flag: String::from("--position"),
            message: String::from("line number must be >= 1"),
        });
    }
    if column == 0 {
        return Err(GraphSliceError::InvalidValue {
            flag: String::from("--position"),
            message: String::from("column number must be >= 1"),
        });
    }

    Ok((line, column))
}

fn parse_u32(value: &str, flag: &str) -> Result<u32, GraphSliceError> {
    value.parse().map_err(|_| GraphSliceError::InvalidValue {
        flag: String::from(flag),
        message: format!("expected a positive integer, got: {value}"),
    })
}

fn parse_direction(value: &str) -> Result<SliceDirection, GraphSliceError> {
    value
        .parse()
        .map_err(|e: DirectionParseError| GraphSliceError::InvalidValue {
            flag: String::from("--direction"),
            message: e.to_string(),
        })
}

fn parse_edge_types(
    value: &str,
) -> Result<Vec<SliceEdgeType>, GraphSliceError> {
    value
        .split(',')
        .map(|s| {
            s.trim().parse().map_err(
                |e: EdgeTypeParseError| GraphSliceError::InvalidValue {
                    flag: String::from("--edge-types"),
                    message: e.to_string(),
                },
            )
        })
        .collect()
}

fn parse_confidence(value: &str) -> Result<f64, GraphSliceError> {
    let confidence: f64 =
        value
            .parse()
            .map_err(|_| GraphSliceError::InvalidValue {
                flag: String::from("--min-confidence"),
                message: format!(
                    "expected a number between 0.0 and 1.0, got: {value}"
                ),
            })?;
    if !(0.0..=1.0).contains(&confidence) {
        return Err(GraphSliceError::InvalidValue {
            flag: String::from("--min-confidence"),
            message: format!(
                "expected a number between 0.0 and 1.0, got: {value}"
            ),
        });
    }
    Ok(confidence)
}

fn parse_detail(
    value: &str,
    flag: &str,
) -> Result<DetailLevel, GraphSliceError> {
    value.parse().map_err(
        |e: crate::DetailLevelParseError| GraphSliceError::InvalidValue {
            flag: String::from(flag),
            message: e.to_string(),
        },
    )
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| String::from(*s)).collect()
    }

    #[test]
    fn parses_minimal_arguments() {
        let arguments = args(&[
            "--uri",
            "file:///src/main.rs",
            "--position",
            "10:5",
        ]);
        let request =
            GraphSliceRequest::parse(&arguments).expect("should parse");

        assert_eq!(request.uri(), "file:///src/main.rs");
        assert_eq!(request.line(), 10);
        assert_eq!(request.column(), 5);
        assert_eq!(request.depth(), DEFAULT_DEPTH);
        assert_eq!(request.direction(), SliceDirection::Both);
        assert_eq!(request.edge_types(), SliceEdgeType::all());
        assert!((request.min_confidence() - DEFAULT_MIN_CONFIDENCE).abs() < f64::EPSILON);
        assert_eq!(request.budget(), &SliceBudget::default());
        assert_eq!(request.entry_detail(), DetailLevel::Structure);
        assert_eq!(request.node_detail(), DetailLevel::Minimal);
    }

    #[test]
    fn parses_all_flags() {
        let arguments = args(&[
            "--uri",
            "file:///src/lib.rs",
            "--position",
            "42:17",
            "--depth",
            "3",
            "--direction",
            "out",
            "--edge-types",
            "call,import",
            "--min-confidence",
            "0.8",
            "--max-cards",
            "10",
            "--max-edges",
            "50",
            "--max-estimated-tokens",
            "2000",
            "--entry-detail",
            "semantic",
            "--node-detail",
            "signature",
        ]);
        let request =
            GraphSliceRequest::parse(&arguments).expect("should parse");

        assert_eq!(request.uri(), "file:///src/lib.rs");
        assert_eq!(request.line(), 42);
        assert_eq!(request.column(), 17);
        assert_eq!(request.depth(), 3);
        assert_eq!(request.direction(), SliceDirection::Out);
        assert_eq!(
            request.edge_types(),
            &[SliceEdgeType::Call, SliceEdgeType::Import]
        );
        assert!((request.min_confidence() - 0.8).abs() < f64::EPSILON);
        assert_eq!(request.budget().max_cards(), 10);
        assert_eq!(request.budget().max_edges(), 50);
        assert_eq!(request.budget().max_estimated_tokens(), 2000);
        assert_eq!(request.entry_detail(), DetailLevel::Semantic);
        assert_eq!(request.node_detail(), DetailLevel::Signature);
    }

    #[test]
    fn normalizes_duplicate_edge_types() {
        let arguments = args(&[
            "--uri",
            "file:///src/main.rs",
            "--position",
            "1:1",
            "--edge-types",
            "import,call,import",
        ]);
        let request =
            GraphSliceRequest::parse(&arguments).expect("should parse");
        assert_eq!(
            request.edge_types(),
            &[SliceEdgeType::Call, SliceEdgeType::Import]
        );
    }

    #[test]
    fn normalizes_edge_types_to_canonical_order() {
        let arguments = args(&[
            "--uri",
            "file:///src/main.rs",
            "--position",
            "1:1",
            "--edge-types",
            "config,call,import",
        ]);
        let request =
            GraphSliceRequest::parse(&arguments).expect("should parse");
        assert_eq!(
            request.edge_types(),
            &[
                SliceEdgeType::Call,
                SliceEdgeType::Import,
                SliceEdgeType::Config
            ]
        );
    }

    #[rstest]
    #[case::missing_uri(&["--position", "10:5"], "--uri")]
    #[case::missing_position(&["--uri", "file:///main.rs"], "--position")]
    #[case::bad_position(
        &["--uri", "file:///main.rs", "--position", "10"],
        "LINE:COL"
    )]
    #[case::zero_line(
        &["--uri", "file:///main.rs", "--position", "0:5"],
        "line"
    )]
    #[case::zero_column(
        &["--uri", "file:///main.rs", "--position", "1:0"],
        "column"
    )]
    #[case::bad_depth(
        &["--uri", "file:///main.rs", "--position", "1:1", "--depth", "abc"],
        "positive integer"
    )]
    #[case::bad_direction(
        &["--uri", "file:///main.rs", "--position", "1:1", "--direction", "left"],
        "unknown direction"
    )]
    #[case::bad_edge_type(
        &["--uri", "file:///main.rs", "--position", "1:1", "--edge-types", "call,unknown"],
        "unknown edge type"
    )]
    #[case::confidence_too_high(
        &["--uri", "file:///main.rs", "--position", "1:1", "--min-confidence", "1.5"],
        "between 0.0 and 1.0"
    )]
    #[case::confidence_not_a_number(
        &["--uri", "file:///main.rs", "--position", "1:1", "--min-confidence", "abc"],
        "between 0.0 and 1.0"
    )]
    #[case::positional_token(
        &["--uri", "file:///main.rs", "--position", "1:1", "stray"],
        "stray"
    )]
    fn rejects_invalid_arguments(
        #[case] arg_list: &[&str],
        #[case] expected_substring: &str,
    ) {
        let arguments = args(arg_list);
        let error = GraphSliceRequest::parse(&arguments)
            .expect_err("should fail");
        let message = error.to_string();
        assert!(
            message.contains(expected_substring),
            "expected error to contain {expected_substring:?}, got: {message}"
        );
    }

    #[test]
    fn skips_unknown_flags() {
        let arguments = args(&[
            "--uri",
            "file:///main.rs",
            "--position",
            "1:1",
            "--bogus",
            "whatever",
            "--experimental",
        ]);
        let request =
            GraphSliceRequest::parse(&arguments).expect("should parse");
        assert_eq!(request.uri(), "file:///main.rs");
    }
}
