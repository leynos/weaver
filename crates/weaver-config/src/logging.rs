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
