//! Programmatic regression test asserting that
//! `crates/weaverd/src/dispatch/act/refactor/refactor_helpers.rs`
//! contains no forbidden lint-suppression patterns.
//!
//! This guards against re-introduction of:
//! - File-wide blanket `#![allow(…)]` attributes.
//! - Item-level `#[allow(dead_code` suppressions.
//! - Anonymous const dead-code witnesses (`const _:`).

use rstest::rstest;

const HELPER_SRC: &str = include_str!("../src/dispatch/act/refactor/refactor_helpers.rs");

/// Returns `refactor_helpers.rs` without its in-file lint compliance tests.
fn refactor_helpers_source() -> &'static str {
    &HELPER_SRC[..HELPER_SRC.rfind("#[cfg(test)]").unwrap_or(HELPER_SRC.len())]
}

/// Returns whether `source` contains `pattern` after stripping whitespace.
fn contains_forbidden_pattern(source: &str, pattern: &str) -> bool {
    normalize_whitespace(source).contains(&normalize_whitespace(pattern))
}

/// Strips all Unicode whitespace characters from `value`.
fn normalize_whitespace(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

#[rstest]
#[case(
    "#![allow(",
    concat!(
        "refactor_helpers.rs contains a forbidden file-wide `#![allow(…)]` attribute. ",
        "File-wide blanket allows are banned by project lint policy."
    )
)]
#[case(
    "#[allow(dead_code",
    concat!(
        "refactor_helpers.rs contains a forbidden item-level `#[allow(dead_code…)]`. ",
        "Use `#[expect(dead_code, reason = \"…\")]` if suppression is genuinely required, ",
        "but prefer restructuring so that it is not."
    )
)]
#[case(
    "const_:",
    concat!(
        "refactor_helpers.rs contains an anonymous const dead-code witness (`const _: …`). ",
        "These witnesses were removed in issue `#89` and must not be re-introduced."
    )
)]
fn no_forbidden_lint_suppression_patterns(#[case] pattern: &str, #[case] message: &str) {
    assert!(
        !contains_forbidden_pattern(refactor_helpers_source(), pattern),
        "{message}"
    );
}
