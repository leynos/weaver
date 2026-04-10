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

fn bounded_levenshtein(left: &[char], right: &[char], max_distance: usize) -> Option<usize> {
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
    let operations = operations_for_domain(domain);
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
        let Some(hint_op) = suggested_domain.operations().first().copied() else {
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
mod tests {
    //! Unit tests for unknown-domain suggestion helpers.

    use super::{
        KnownDomain, bounded_levenshtein, suggestion_for_unknown_domain,
        write_missing_operation_guidance, write_unknown_domain_guidance,
    };
    use crate::localizer::WEAVER_EN_US;
    use ortho_config::FluentLocalizer;
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
        let left_chars: Vec<char> = left.chars().collect();
        let right_chars: Vec<char> = right.chars().collect();
        assert_eq!(bounded_levenshtein(&left_chars, &right_chars, 2), expected);
    }

    #[rstest]
    #[case("obsrve", Some(KnownDomain::Observe))]
    #[case("bogus", None)]
    #[case("obsve", Some(KnownDomain::Observe))]
    fn suggestion_for_unknown_domain_cases(
        #[case] input: &str,
        #[case] expected: Option<KnownDomain>,
    ) {
        assert_eq!(suggestion_for_unknown_domain(input), expected);
    }

    fn fluent_localizer() -> FluentLocalizer {
        FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
            .expect("embedded Fluent catalogue must parse")
    }

    fn assert_single_error_prefix(output: &str) {
        assert!(
            output.contains("error: "),
            "output should contain an error prefix"
        );
        assert!(
            !output.contains("error: error:"),
            "output should not contain a duplicated error prefix: {output}"
        );
    }

    fn assert_three_part_guidance(
        output: &str,
        error_text: &str,
        alternatives_text: &str,
        next_command: &str,
    ) {
        assert_single_error_prefix(output);
        let error_pos = output.find(error_text).expect("expected error text");
        let alternatives_pos = output
            .find(alternatives_text)
            .expect("expected alternatives text");
        let next_command_line = format!("Next command:\n  {next_command}");
        let next_command_pos = output
            .find(&next_command_line)
            .expect("expected exact Next command block");

        assert!(
            error_pos < alternatives_pos,
            "error line must precede alternatives block: {output}"
        );
        assert!(
            alternatives_pos < next_command_pos,
            "alternatives block must precede Next command: {output}"
        );
    }

    #[test]
    fn fluent_missing_operation_guidance_has_single_error_prefix() {
        let localizer = fluent_localizer();
        let mut output = Vec::new();

        let emitted =
            write_missing_operation_guidance(&mut output, &localizer, KnownDomain::Observe)
                .expect("guidance write must succeed");

        assert!(emitted, "known domain should emit guidance");
        let output = String::from_utf8(output).expect("guidance must be valid UTF-8");
        assert_three_part_guidance(
            &output,
            "error: operation required for domain 'observe'",
            "Available operations:\n  get-definition",
            "weaver observe get-definition --help",
        );
    }

    #[test]
    fn fluent_unknown_domain_guidance_has_single_error_prefix() {
        let localizer = fluent_localizer();
        let mut output = Vec::new();

        let emitted = write_unknown_domain_guidance(&mut output, &localizer, "unknown-domain")
            .expect("guidance write must succeed");

        assert!(emitted, "unknown domain should emit guidance");
        let output = String::from_utf8(output).expect("guidance must be valid UTF-8");
        assert_three_part_guidance(
            &output,
            "error: unknown domain 'unknown-domain'",
            "Valid domains: observe, act, verify",
            "weaver --help",
        );
    }

    #[test]
    fn fluent_unknown_domain_guidance_uses_unique_suggestion_when_available() {
        let localizer = fluent_localizer();
        let mut output = Vec::new();

        let emitted = write_unknown_domain_guidance(&mut output, &localizer, "obsrve")
            .expect("guidance write must succeed");

        assert!(emitted, "unknown domain should emit guidance");
        let output = String::from_utf8(output).expect("guidance must be valid UTF-8");
        assert!(
            output.contains("Did you mean 'observe'?"),
            "unique suggestion should be rendered: {output}"
        );
        assert_three_part_guidance(
            &output,
            "error: unknown domain 'obsrve'",
            "Did you mean 'observe'?",
            "weaver observe get-definition --help",
        );
    }
}

#[cfg(test)]
pub(crate) mod fluent_entries;
