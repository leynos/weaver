//! Discoverability helpers for CLI domain guidance.
//!
//! This module centralizes the client-side domain catalogue used for top-level
//! help assertions and for contextual guidance when an operator supplies a
//! known domain without an operation.

use std::io::{self, Write};

use ortho_config::{LocalizationArgs, Localizer};

/// A validated, known CLI domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KnownDomain {
    Observe,
    Act,
    Verify,
}

impl KnownDomain {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::Act => "act",
            Self::Verify => "verify",
        }
    }

    /// Resolves a raw string to a known domain, case-insensitively.
    pub(crate) fn try_parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "observe" => Some(Self::Observe),
            "act" => Some(Self::Act),
            "verify" => Some(Self::Verify),
            _ => None,
        }
    }

    fn operations(self) -> &'static [&'static str] {
        DOMAIN_OPERATIONS
            .iter()
            .find(|(name, _, _)| *name == self.as_str())
            .map(|(_, _, ops)| *ops)
            .unwrap_or_else(|| panic!("missing DOMAIN_OPERATIONS entry for '{}'", self.as_str()))
    }
}

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
pub(crate) fn operations_for_domain(domain: KnownDomain) -> &'static [&'static str] {
    domain.operations()
}

fn strip_bidi_isolates(text: String) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn known_domain_from_catalogue_entry(domain: &str) -> KnownDomain {
    KnownDomain::try_parse(domain).unwrap_or_else(|| {
        panic!("DOMAIN_OPERATIONS must contain valid KnownDomain entries: {domain}")
    })
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
/// supplied [`KnownDomain`] has no registered operations, so no guidance could
/// be emitted.
pub(crate) fn write_missing_operation_guidance<W: Write>(
    writer: &mut W,
    localizer: &dyn Localizer,
    domain: KnownDomain,
) -> io::Result<bool> {
    let operations = operations_for_domain(domain);
    let Some(hint_operation) = operations.first() else {
        return Ok(false);
    };
    let domain_name = domain.as_str();
    let mut args = LocalizationArgs::new();
    args.insert("domain", domain_name.into());
    args.insert("hint_operation", (*hint_operation).into());
    let error = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-missing-operation-error",
        Some(&args),
        &format!("error: operation required for domain '{domain_name}'"),
    ));
    let available_operations = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-available-operations",
        None,
        "Available operations:",
    ));
    let hint = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-help-hint",
        Some(&args),
        &format!("Run 'weaver {domain_name} {hint_operation} --help' for operation details."),
    ));

    writeln!(writer, "{error}")?;
    writeln!(writer)?;
    writeln!(writer, "{available_operations}")?;
    for operation in operations {
        writeln!(writer, "  {operation}")?;
    }
    writeln!(writer)?;
    writeln!(writer, "{hint}")?;

    Ok(true)
}

/// Writes contextual guidance for an unknown domain missing its operation.
pub(crate) fn write_unknown_domain_guidance<W: Write>(
    writer: &mut W,
    localizer: &dyn Localizer,
    domain: &str,
) -> io::Result<bool> {
    if KnownDomain::try_parse(domain).is_some() {
        return Ok(false);
    }
    let Some((hint_domain, hint_operation)) = first_known_command() else {
        return Ok(false);
    };
    let mut args = LocalizationArgs::new();
    args.insert("domain", domain.into());
    args.insert("hint_domain", hint_domain.into());
    args.insert("hint_operation", hint_operation.into());
    let error = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-unknown-domain-error",
        Some(&args),
        &format!("error: unknown domain '{domain}'"),
    ));
    let available_operations = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-available-operations",
        None,
        "Available operations:",
    ));
    let hint = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-help-hint-unknown-domain",
        Some(&args),
        &format!("Run 'weaver {hint_domain} {hint_operation} --help' for operation details."),
    ));

    writeln!(writer, "{error}")?;
    writeln!(writer)?;
    writeln!(writer, "{available_operations}")?;
    for (known_domain, _, operations) in DOMAIN_OPERATIONS {
        let known_domain_enum = known_domain_from_catalogue_entry(known_domain);
        for operation in *operations {
            debug_assert_eq!(known_domain_enum.as_str(), *known_domain);
            writeln!(writer, "  {known_domain} {operation}")?;
        }
    }
    writeln!(writer)?;
    writeln!(writer, "{hint}")?;

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
    //! Test-only after-help catalogue builders used to assert localized help
    //! output without widening the production discoverability surface.

    pub(in crate::discoverability) const HEADER: (&str, &str) =
        ("weaver-after-help-header", "Domains and operations:");

    fn domain_heading_entry(domain: super::KnownDomain) -> (String, String) {
        let description = super::DOMAIN_OPERATIONS
            .iter()
            .find(|(candidate, _, _)| *candidate == domain.as_str())
            .map(|(_, description, _)| *description)
            .unwrap_or_default();

        (
            format!("weaver-after-help-{}-heading", domain.as_str()),
            format!("{} \u{2014} {description}", domain.as_str()),
        )
    }

    fn pad_to(s: &mut String, width: usize) {
        if s.len() < width {
            s.extend(std::iter::repeat_n(' ', width - s.len()));
        }
    }

    fn format_operation_row(operations: &[String]) -> String {
        const SECOND_COLUMN_START: usize = 18;
        const THIRD_COLUMN_START: usize = 37;

        let mut row = String::new();
        if let Some(first) = operations.first() {
            row.push_str(first);
        }
        if let Some(second) = operations.get(1) {
            pad_to(&mut row, SECOND_COLUMN_START);
            row.push_str(second);
        }
        if let Some(third) = operations.get(2) {
            pad_to(&mut row, THIRD_COLUMN_START);
            row.push_str(third);
        }
        row
    }

    /// Renders the after-help domains-and-operations catalogue.
    pub(crate) fn render_after_help(localizer: &dyn ortho_config::Localizer) -> String {
        let header = localizer.message(HEADER.0, None, HEADER.1);
        let mut sections = Vec::new();
        for (domain_str, _, operations) in super::DOMAIN_OPERATIONS {
            let domain = super::known_domain_from_catalogue_entry(domain_str);
            let (heading_id, heading_fallback) = domain_heading_entry(domain);
            let heading = localizer.message(&heading_id, None, &heading_fallback);
            let rows = operations
                .chunks(3)
                .map(|chunk| {
                    let operations = chunk.iter().map(ToString::to_string).collect::<Vec<_>>();
                    format!("    {}", format_operation_row(&operations))
                })
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("  {heading}\n{rows}"));
        }

        format!("{header}\n\n{}", sections.join("\n\n"))
    }
}
