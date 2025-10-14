//! Logging configuration types for Weaver binaries.
//!
//! Defines the [`LogFormat`] enumeration used across the CLI and daemon along
//! with parsing helpers that integrate with Serde and `strum` derives.
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Supported logging output formats.
#[derive(
    Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, EnumString, Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum LogFormat {
    /// Structured JSON suitable for ingestion by logging stacks.
    #[default]
    Json,
    /// Human-readable single line output.
    Compact,
}

/// Errors encountered while parsing a [`LogFormat`] from text.
pub type LogFormatParseError = strum::ParseError;
