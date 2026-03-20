//! Discoverability helpers for CLI domain guidance.
//!
//! This module centralizes the client-side domain catalogue used for top-level
//! help assertions and for contextual guidance when an operator supplies a
//! known domain without an operation.

use std::io::{self, Write};

/// Canonical domain-to-operation mapping for CLI discoverability features.
pub const DOMAIN_OPERATIONS: &[(&str, &str, &[&str])] = &[
    (
        "observe",
        "Query code structure and relationships",
        &[
            "get-definition",
            "find-references",
            "grep",
            "diagnostics",
            "call-hierarchy",
            "get-card",
        ],
    ),
    (
        "act",
        "Perform code modifications",
        &[
            "rename-symbol",
            "apply-edits",
            "apply-patch",
            "apply-rewrite",
            "refactor",
        ],
    ),
    (
        "verify",
        "Validate code correctness",
        &["diagnostics", "syntax"],
    ),
];

/// Returns the canonical operation list for a known domain.
pub(crate) fn operations_for_domain(
    domain: &str,
) -> Option<(&'static str, &'static [&'static str])> {
    DOMAIN_OPERATIONS
        .iter()
        .find(|(candidate, _, _)| candidate.eq_ignore_ascii_case(domain))
        .map(|(candidate, _, operations)| (*candidate, *operations))
}

/// Returns the first domain plus its first operation from `DOMAIN_OPERATIONS`.
///
/// Returns `None` when the catalogue is empty or when the first domain has no
/// registered operations.
fn first_known_command() -> Option<(&'static str, &'static str)> {
    let (domain, _, operations) = DOMAIN_OPERATIONS.first().copied()?;
    Some((domain, operations.first().copied()?))
}

/// Writes contextual guidance for a known domain missing its operation.
///
/// Returns `Ok(true)` when guidance was emitted and `Ok(false)` when the
/// supplied domain is not part of the client-side catalogue.
pub(crate) fn write_missing_operation_guidance<W: Write>(
    writer: &mut W,
    domain: &str,
) -> io::Result<bool> {
    let Some((domain, operations)) = operations_for_domain(domain) else {
        return Ok(false);
    };
    let Some(hint_operation) = operations.first() else {
        return Ok(false);
    };

    writeln!(writer, "error: operation required for domain '{domain}'")?;
    writeln!(writer)?;
    writeln!(writer, "Available operations:")?;
    for operation in operations {
        writeln!(writer, "  {operation}")?;
    }
    writeln!(writer)?;
    writeln!(
        writer,
        "Run 'weaver {domain} {hint_operation} --help' for operation details.",
    )?;

    Ok(true)
}

/// Writes contextual guidance for an unknown domain missing its operation.
pub(crate) fn write_unknown_domain_guidance<W: Write>(
    writer: &mut W,
    domain: &str,
) -> io::Result<bool> {
    if operations_for_domain(domain).is_some() {
        return Ok(false);
    }
    let Some((hint_domain, hint_operation)) = first_known_command() else {
        return Ok(false);
    };

    writeln!(writer, "error: unknown domain '{domain}'")?;
    writeln!(writer)?;
    writeln!(writer, "Available operations:")?;
    for (known_domain, _, operations) in DOMAIN_OPERATIONS {
        for operation in *operations {
            writeln!(writer, "  {known_domain} {operation}")?;
        }
    }
    writeln!(writer)?;
    writeln!(
        writer,
        "Run 'weaver {hint_domain} {hint_operation} --help' for operation details.",
    )?;

    Ok(true)
}

/// Returns true when a parsed CLI invocation qualifies for preflight guidance.
pub(crate) fn should_emit_domain_guidance(cli: &crate::Cli) -> bool {
    cli.command.is_none()
        && !cli.capabilities
        && cli
            .domain
            .as_deref()
            .is_some_and(|domain| !domain.trim().is_empty())
        && cli
            .operation
            .as_deref()
            .is_none_or(|operation| operation.trim().is_empty())
}

#[cfg(test)]
pub(crate) mod fluent_entries {
    pub(in crate::discoverability) const HEADER: (&str, &str) =
        ("weaver-after-help-header", "Domains and operations:");

    fn domain_heading_entry(domain: &str) -> Option<(String, String)> {
        super::DOMAIN_OPERATIONS
            .iter()
            .find(|(candidate, _, _)| candidate.eq_ignore_ascii_case(domain))
            .map(|(candidate, description, _)| {
                (
                    format!("weaver-after-help-{candidate}-heading"),
                    format!("{candidate} \u{2014} {description}"),
                )
            })
    }

    fn localize_operation(
        localizer: &dyn ortho_config::Localizer,
        domain: &str,
        operation: &str,
    ) -> String {
        let message_id = format!("weaver-after-help-{domain}-{operation}");
        localizer.message(&message_id, None, operation)
    }

    fn format_operation_row(operations: &[String]) -> String {
        const SECOND_COLUMN_START: usize = 18;
        const THIRD_COLUMN_START: usize = 37;

        let mut row = String::new();
        if let Some(first) = operations.first() {
            row.push_str(first);
        }
        if let Some(second) = operations.get(1) {
            while row.len() < SECOND_COLUMN_START {
                row.push(' ');
            }
            row.push_str(second);
        }
        if let Some(third) = operations.get(2) {
            while row.len() < THIRD_COLUMN_START {
                row.push(' ');
            }
            row.push_str(third);
        }
        row
    }

    /// Renders the after-help domains-and-operations catalogue.
    pub(crate) fn render_after_help(localizer: &dyn ortho_config::Localizer) -> String {
        let header = localizer.message(HEADER.0, None, HEADER.1);
        let mut sections = Vec::new();
        for (domain, _, operations) in super::DOMAIN_OPERATIONS {
            let Some((heading_id, heading_fallback)) = domain_heading_entry(domain) else {
                eprintln!("warning: missing heading for domain: {domain}");
                debug_assert!(false, "missing heading for domain: {domain}");
                continue;
            };
            let heading = localizer.message(&heading_id, None, &heading_fallback);
            let rows = operations
                .chunks(3)
                .map(|chunk| {
                    let localized = chunk
                        .iter()
                        .map(|operation| localize_operation(localizer, domain, operation))
                        .collect::<Vec<_>>();
                    format!("    {}", format_operation_row(&localized))
                })
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("  {heading}\n{rows}"));
        }

        format!("{header}\n\n{}", sections.join("\n\n"))
    }
}
