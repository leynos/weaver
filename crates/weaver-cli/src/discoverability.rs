//! Discoverability helpers for CLI domain guidance.
//!
//! This module centralizes the client-side domain catalogue used for top-level
//! help assertions and for contextual guidance when an operator supplies a
//! known domain without an operation.

use std::io::{self, Write};

use ortho_config::{LocalizationArgs, Localizer};

use crate::actionable_guidance::{ActionableGuidance, write_actionable_guidance};

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
            .find(|(domain, ..)| *domain == normalized.as_str())
            .map(|(domain, ..)| match *domain {
                "observe" => Self::Observe,
                "act" => Self::Act,
                "verify" => Self::Verify,
                _ => panic!("DOMAIN_OPERATIONS contains unknown domain: {domain}"),
            })
    }

    fn operations(self) -> Option<&'static [&'static str]> {
        DOMAIN_OPERATIONS
            .iter()
            .find(|(name, ..)| *name == self.as_str())
            .map(|(_, _, ops)| *ops)
    }

    fn catalogue_order() -> impl Iterator<Item = Self> {
        DOMAIN_OPERATIONS
            .iter()
            .map(|(domain, ..)| known_domain_from_catalogue_entry(domain))
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
            "graph-slice",
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
pub(crate) fn operations_for_domain(domain: KnownDomain) -> Option<&'static [&'static str]> {
    domain.operations()
}

fn strip_bidi_isolates(text: String) -> String { text.replace(['\u{2068}', '\u{2069}'], "") }

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

pub(crate) fn suggestion_for_unknown_domain(domain: &str) -> Option<KnownDomain> {
    let normalized_domain: Vec<char> = domain.trim().to_ascii_lowercase().chars().collect();
    let mut best_match = None;
    let mut best_distance = usize::MAX;
    let mut tied = false;

    for candidate in KnownDomain::catalogue_order() {
        let candidate_chars: Vec<char> = candidate.as_str().chars().collect();
        let Some(distance) = bounded_levenshtein(&normalized_domain, &candidate_chars, 2) else {
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

pub(crate) fn bounded_levenshtein(
    left: &[char],
    right: &[char],
    max_distance: usize,
) -> Option<usize> {
    if left.len().abs_diff(right.len()) > max_distance {
        return None;
    }

    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_char) in left.iter().enumerate() {
        current[0] = left_index + 1;
        let mut row_min = current[0];

        for (right_index, right_char) in right.iter().enumerate() {
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

    let distance = previous[right.len()];
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
    let Some(operations) = operations_for_domain(domain) else {
        return Ok(false);
    };
    let Some(hint_operation) = operations.first() else {
        return Ok(false);
    };
    let domain_name = domain.as_str();
    let mut args = LocalizationArgs::new();
    args.insert("domain", domain_name.into());
    args.insert("hint_operation", (*hint_operation).into());

    let problem = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-missing-operation-error",
        Some(&args),
        &format!("operation required for domain '{domain_name}'"),
    ));

    let available_operations = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-available-operations",
        None,
        "Available operations:",
    ));

    let mut alternatives = vec![available_operations];
    for operation in operations {
        alternatives.push(format!("  {operation}"));
    }

    let next_command = format!("weaver {domain_name} {hint_operation} --help");

    let guidance = ActionableGuidance::new(problem, alternatives, next_command);
    write_actionable_guidance(writer, &guidance)?;

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

    let problem = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-unknown-domain-error",
        Some(&args),
        &format!("unknown domain '{domain}'"),
    ));

    let valid_domains_message = strip_bidi_isolates(localizer.message(
        "weaver-domain-guidance-valid-domains",
        Some(&args),
        &format!("Valid domains: {valid_domains}"),
    ));

    let mut alternatives = vec![valid_domains_message];

    // Include "Did you mean" in alternatives if there's a suggestion
    let next_command = if let Some(suggested_domain) = suggestion_for_unknown_domain(domain) {
        let suggested_domain_str = suggested_domain.as_str();
        args.insert("suggested_domain", suggested_domain_str.into());
        let suggestion = strip_bidi_isolates(localizer.message(
            "weaver-domain-guidance-did-you-mean-domain",
            Some(&args),
            &format!("Did you mean '{suggested_domain_str}'?"),
        ));
        alternatives.push(suggestion);
        let Some(hint_op) = suggested_domain
            .operations()
            .and_then(|operations| operations.first().copied())
        else {
            return Ok(false);
        };
        format!("weaver {suggested_domain_str} {hint_op} --help")
    } else {
        "weaver --help".to_string()
    };

    let guidance = ActionableGuidance::new(problem, alternatives, next_command);
    write_actionable_guidance(writer, &guidance)?;

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
pub(crate) mod fluent_entries;
