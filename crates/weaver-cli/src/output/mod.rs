//! Human-readable output rendering for daemon responses.
//!
//! This module parses JSON payloads for location- and diagnostic-bearing
//! responses and renders them with source context for humans. JSON payloads
//! remain unchanged when JSON output is requested.

mod models;
mod render;
mod source;

use clap::ValueEnum;

use crate::output::models::{
    DiagnosticItem, DiagnosticsResponse, ReferenceResponse, VerificationFailure, parse_definitions,
    parse_verification_failures,
};
use crate::output::source::{
    SourceLocation, SourcePosition, extract_uri_argument, from_path_or_uri, from_uri,
};

/// Output format selection for domain command responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    /// Selects `human` for terminal output and `json` for redirected output.
    Auto,
    /// Always render human-readable output.
    Human,
    /// Always emit raw JSON payloads from the daemon.
    Json,
}

/// Output format after resolving `auto` based on TTY detection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedOutputFormat {
    /// Human-readable output with source context.
    Human,
    /// Raw JSON payloads.
    Json,
}

impl OutputFormat {
    /// Resolves the output format based on whether stdout is a terminal.
    #[must_use]
    pub fn resolve(self, stdout_is_terminal: bool) -> ResolvedOutputFormat {
        match self {
            Self::Auto => {
                if stdout_is_terminal {
                    ResolvedOutputFormat::Human
                } else {
                    ResolvedOutputFormat::Json
                }
            }
            Self::Human => ResolvedOutputFormat::Human,
            Self::Json => ResolvedOutputFormat::Json,
        }
    }
}

/// Context about the command whose output is being rendered.
#[derive(Clone, Debug)]
pub struct OutputContext {
    /// The command domain (for example `observe`).
    pub domain: String,
    /// The operation within the domain.
    pub operation: String,
    /// Raw CLI arguments supplied to the command.
    pub arguments: Vec<String>,
}

impl OutputContext {
    /// Creates a new output context from command metadata.
    #[must_use]
    pub fn new(
        domain: impl Into<String>,
        operation: impl Into<String>,
        arguments: Vec<String>,
    ) -> Self {
        Self {
            domain: domain.into(),
            operation: operation.into(),
            arguments,
        }
    }
}

/// Attempts to render human-readable output for known response payloads.
///
/// Returns `Some(rendered)` when the payload matches a known schema, otherwise
/// returns `None` to indicate the raw payload should be forwarded.
#[must_use]
pub fn render_human_output(context: &OutputContext, data: &str) -> Option<String> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return None;
    }

    let domain = context.domain.to_ascii_lowercase();
    let operation = context.operation.to_ascii_lowercase();

    match (domain.as_str(), operation.as_str()) {
        ("observe", "get-definition") => render_definitions(trimmed),
        ("observe", "find-references") => render_references(trimmed),
        ("verify", "diagnostics") => render_diagnostics(trimmed, context),
        ("act", _) => render_verification_failures(trimmed),
        _ => None,
    }
}

fn render_definitions(payload: &str) -> Option<String> {
    let definitions = parse_definitions(payload)?;
    if definitions.is_empty() {
        return Some(String::from("no definitions found\n"));
    }
    let locations: Vec<SourceLocation> = definitions
        .into_iter()
        .map(|definition| {
            from_uri(
                &definition.uri,
                Some(definition.line),
                Some(definition.column),
                "definition",
            )
        })
        .collect();
    Some(render::render_locations(&locations))
}

fn render_references(payload: &str) -> Option<String> {
    let response: ReferenceResponse = serde_json::from_str(payload).ok()?;
    if response.references.is_empty() {
        return Some(String::from("no references found\n"));
    }
    let locations: Vec<SourceLocation> = response
        .references
        .into_iter()
        .map(|reference| {
            from_uri(
                &reference.uri,
                Some(reference.line),
                Some(reference.column),
                "reference",
            )
        })
        .collect();
    Some(render::render_locations(&locations))
}

fn render_diagnostics(payload: &str, context: &OutputContext) -> Option<String> {
    let response: DiagnosticsResponse = serde_json::from_str(payload).ok()?;
    if response.diagnostics.is_empty() {
        return Some(String::from("no diagnostics reported\n"));
    }
    let fallback_uri = extract_uri_argument(&context.arguments);
    let locations: Vec<SourceLocation> = response
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic_to_location(diagnostic, fallback_uri.as_deref()))
        .collect();
    Some(render::render_locations(&locations))
}

fn render_verification_failures(payload: &str) -> Option<String> {
    let failures = parse_verification_failures(payload)?;
    if failures.is_empty() {
        return None;
    }
    let locations: Vec<SourceLocation> = failures
        .into_iter()
        .map(verification_failure_to_location)
        .collect();
    Some(render::render_locations(&locations))
}

fn diagnostic_to_location(
    diagnostic: DiagnosticItem,
    fallback_uri: Option<&str>,
) -> SourceLocation {
    let label = if diagnostic.message.is_empty() {
        String::from("diagnostic")
    } else {
        diagnostic.message
    };

    if let Some(uri) = diagnostic.uri.as_deref().or(fallback_uri) {
        from_uri(uri, Some(diagnostic.line), Some(diagnostic.column), label)
    } else {
        SourceLocation::unresolved(
            String::from("<unknown source>"),
            SourcePosition::new(Some(diagnostic.line), Some(diagnostic.column)),
            label,
            String::from("missing URI for diagnostic"),
        )
    }
}

fn verification_failure_to_location(failure: VerificationFailure) -> SourceLocation {
    let label = if let Some(phase) = failure.phase.as_deref() {
        format!("{phase}: {}", failure.message)
    } else {
        failure.message
    };

    match failure.location {
        Some(location) => from_path_or_uri(&location, failure.line, failure.column, label),
        None => SourceLocation::unresolved(
            String::from("<unknown source>"),
            SourcePosition::new(failure.line, failure.column),
            label,
            String::from("missing file path for verification failure"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_auto_output_format() {
        assert_eq!(
            OutputFormat::Auto.resolve(true),
            ResolvedOutputFormat::Human
        );
        assert_eq!(
            OutputFormat::Auto.resolve(false),
            ResolvedOutputFormat::Json
        );
    }
}
