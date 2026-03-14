//! Discoverability helpers for CLI domain guidance.
//!
//! This module centralizes the client-side domain catalogue used for top-level
//! help assertions and for contextual guidance when an operator supplies a
//! known domain without an operation.

use std::io::{self, Write};

/// Canonical domain-to-operation mapping for CLI discoverability features.
pub const DOMAIN_OPERATIONS: &[(&str, &[&str])] = &[
    (
        "observe",
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
        &[
            "rename-symbol",
            "apply-edits",
            "apply-patch",
            "apply-rewrite",
            "refactor",
        ],
    ),
    ("verify", &["diagnostics", "syntax"]),
];

/// Returns the canonical operation list for a known domain.
pub(crate) fn operations_for_domain(
    domain: &str,
) -> Option<(&'static str, &'static [&'static str])> {
    DOMAIN_OPERATIONS
        .iter()
        .copied()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(domain))
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

/// Returns true when a parsed CLI invocation qualifies for domain guidance.
pub(crate) fn should_emit_missing_operation_guidance(cli: &crate::Cli) -> bool {
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
    pub(in crate::discoverability) const OBSERVE_HEADING: (&str, &str) = (
        "weaver-after-help-observe-heading",
        "observe \u{2014} Query code structure and relationships",
    );
    pub(in crate::discoverability) const OBSERVE_OPS_1: (&str, &str) = (
        "weaver-after-help-observe-ops-1",
        "get-definition    find-references    grep",
    );
    pub(in crate::discoverability) const OBSERVE_OPS_2: (&str, &str) = (
        "weaver-after-help-observe-ops-2",
        "diagnostics       call-hierarchy    get-card",
    );
    pub(in crate::discoverability) const ACT_HEADING: (&str, &str) = (
        "weaver-after-help-act-heading",
        "act \u{2014} Perform code modifications",
    );
    pub(in crate::discoverability) const ACT_OPS_1: (&str, &str) = (
        "weaver-after-help-act-ops-1",
        "rename-symbol     apply-edits        apply-patch",
    );
    pub(in crate::discoverability) const ACT_OPS_2: (&str, &str) =
        ("weaver-after-help-act-ops-2", "apply-rewrite     refactor");
    pub(in crate::discoverability) const VERIFY_HEADING: (&str, &str) = (
        "weaver-after-help-verify-heading",
        "verify \u{2014} Validate code correctness",
    );
    pub(in crate::discoverability) const VERIFY_OPS: (&str, &str) =
        ("weaver-after-help-verify-ops", "diagnostics       syntax");

    /// Renders the after-help domains-and-operations catalogue.
    pub(crate) fn render_after_help(localizer: &dyn ortho_config::Localizer) -> String {
        let msg = |entry: &(&str, &str)| localizer.message(entry.0, None, entry.1);
        let header = msg(&HEADER);
        let obs_h = msg(&OBSERVE_HEADING);
        let obs_1 = msg(&OBSERVE_OPS_1);
        let obs_2 = msg(&OBSERVE_OPS_2);
        let act_h = msg(&ACT_HEADING);
        let act_1 = msg(&ACT_OPS_1);
        let act_2 = msg(&ACT_OPS_2);
        let ver_h = msg(&VERIFY_HEADING);
        let ver_o = msg(&VERIFY_OPS);
        format!(
            "{header}\n\n  {obs_h}\n    {obs_1}\n    {obs_2}\n\n\
             \x20 {act_h}\n    {act_1}\n    {act_2}\n\n\
             \x20 {ver_h}\n    {ver_o}"
        )
    }
}
