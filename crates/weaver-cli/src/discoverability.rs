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
    /// Uses DOMAIN_OPERATIONS as the single source of truth.
    pub(crate) fn try_parse(s: &str) -> Option<Self> {
        let normalized = s.trim().to_ascii_lowercase();
        DOMAIN_OPERATIONS
            .iter()
            .find(|(domain, _, _)| *domain == normalized.as_str())
            .map(|(domain, _, _)| match *domain {
                "observe" => Self::Observe,
                "act" => Self::Act,
                "verify" => Self::Verify,
                _ => panic!("DOMAIN_OPERATIONS contains unknown domain: {domain}"),
            })
    }

    fn operations(self) -> &'static [&'static str] {
        DOMAIN_OPERATIONS
            .iter()
            .find(|(name, _, _)| *name == self.as_str())
            .map(|(_, _, ops)| *ops)
            .unwrap_or_else(|| panic!("missing DOMAIN_OPERATIONS entry for '{}'", self.as_str()))
    }

    fn catalogue_order() -> impl Iterator<Item = Self> {
        DOMAIN_OPERATIONS
            .iter()
            .map(|(domain, _, _)| known_domain_from_catalogue_entry(domain))
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
    match domain {
        "observe" => KnownDomain::Observe,
        "act" => KnownDomain::Act,
        "verify" => KnownDomain::Verify,
        _ => panic!("DOMAIN_OPERATIONS must contain valid KnownDomain entries: {domain}"),
    }
}

fn valid_domains_list() -> String {
    KnownDomain::catalogue_order()
        .map(KnownDomain::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn suggestion_for_unknown_domain(domain: &str) -> Option<KnownDomain> {
    let normalized_domain = domain.trim().to_ascii_lowercase();
    let mut best_match = None;
    let mut best_distance = usize::MAX;
    let mut tied = false;

    for candidate in KnownDomain::catalogue_order() {
        let Some(distance) = bounded_levenshtein(&normalized_domain, candidate.as_str(), 2) else {
            continue;
        };
        if distance < best_distance {
            best_distance = distance;
            best_match = Some(candidate);
            tied = false;
        } else if distance == best_distance {
            tied = true;
        }
    }

    if tied { None } else { best_match }
}

fn bounded_levenshtein(left: &str, right: &str, max_distance: usize) -> Option<usize> {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();

    if left_chars.len().abs_diff(right_chars.len()) > max_distance {
        return None;
    }

    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left_chars.iter().enumerate() {
        current[0] = left_index + 1;
        let mut row_min = current[0];

        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != right_char);
            current[right_index + 1] = usize::min(
                usize::min(previous[right_index + 1] + 1, current[right_index] + 1),
                previous[right_index] + substitution_cost,
            );
            row_min = row_min.min(current[right_index + 1]);
        }

        if row_min > max_distance {
            return None;
        }

        previous.clone_from_slice(&current);
    }

    let distance = previous[right_chars.len()];
    (distance <= max_distance).then_some(distance)
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

/// Writes contextual guidance for an unknown domain.
pub(crate) fn write_unknown_domain_guidance<W: Write>(
    writer: &mut W,
    localizer: &dyn Localizer,
    domain: &str,
) -> io::Result<bool> {
    if KnownDomain::try_parse(domain).is_some() {
        return Ok(false);
    }
    let mut args = LocalizationArgs::new();
    args.insert("domain", domain.into());
    let valid_domains = valid_domains_list();
    args.insert("domains", valid_domains.as_str().into());
    let error = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-unknown-domain-error",
        Some(&args),
        &format!("error: unknown domain '{domain}'"),
    ));
    let valid_domains_message = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-valid-domains",
        Some(&args),
        &format!("Valid domains: {valid_domains}"),
    ));

    writeln!(writer, "{error}")?;
    writeln!(writer)?;
    writeln!(writer, "{valid_domains_message}")?;

    if let Some(suggested_domain) = suggestion_for_unknown_domain(domain) {
        let suggested_domain = suggested_domain.as_str();
        args.insert("suggested_domain", suggested_domain.into());
        let suggestion = strip_bidi_isolates(localizer.message(
            "weaver-domain-guidance-did-you-mean-domain",
            Some(&args),
            &format!("Did you mean '{suggested_domain}'?"),
        ));
        writeln!(writer, "{suggestion}")?;
    }

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
}

#[cfg(test)]
mod tests {
    //! Unit tests for unknown-domain suggestion helpers.

    use super::{KnownDomain, bounded_levenshtein, suggestion_for_unknown_domain};
    use rstest::rstest;

    #[rstest]
    #[case("observe", "observe", Some(0))]
    #[case("obsrve", "observe", Some(1))]
    #[case("obsve", "observe", Some(2))]
    #[case("bogus", "observe", None)]
    fn bounded_levenshtein_respects_threshold(
        #[case] left: &str,
        #[case] right: &str,
        #[case] expected: Option<usize>,
    ) {
        assert_eq!(bounded_levenshtein(left, right, 2), expected);
    }

    #[test]
    fn suggestion_for_unknown_domain_returns_closest_match_within_threshold() {
        assert_eq!(
            suggestion_for_unknown_domain("obsrve"),
            Some(KnownDomain::Observe)
        );
    }

    #[test]
    fn suggestion_for_unknown_domain_rejects_distant_values() {
        assert_eq!(suggestion_for_unknown_domain("bogus"), None);
    }

    #[test]
    fn suggestion_for_unknown_domain_accepts_distance_two_match() {
        assert_eq!(
            suggestion_for_unknown_domain("obsve"),
            Some(KnownDomain::Observe)
        );
    }
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
