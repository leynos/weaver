//! Programmatic regression test asserting that
//! `crates/weaverd/src/dispatch/act/refactor/refactor_helpers.rs`
//! contains no forbidden lint-suppression patterns.
//!
//! This guards against re-introduction of:
//! - File-wide blanket `#![allow(…)]` attributes.
//! - Item-level `#[allow(dead_code` suppressions.
//! - Anonymous const dead-code witnesses (`const _:`).

const HELPER_SRC: &str = include_str!("../src/dispatch/act/refactor/refactor_helpers.rs");

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

#[test]
fn no_file_wide_blanket_allow() {
    assert!(
        !contains_forbidden_pattern(HELPER_SRC, "#![allow("),
        concat!(
            "refactor_helpers.rs contains a forbidden file-wide `#![allow(…)]` attribute. ",
            "File-wide blanket allows are banned by project lint policy."
        )
    );
}

#[test]
fn no_item_level_dead_code_allow() {
    assert!(
        !contains_forbidden_pattern(HELPER_SRC, "#[allow(dead_code"),
        concat!(
            "refactor_helpers.rs contains a forbidden item-level `#[allow(dead_code…)]`. ",
            "Use `#[expect(dead_code, reason = \"…\")]` if suppression is genuinely required, ",
            "but prefer restructuring so that it is not."
        )
    );
}

#[test]
fn no_anonymous_const_witness() {
    assert!(
        !contains_forbidden_pattern(HELPER_SRC, "const _:"),
        concat!(
            "refactor_helpers.rs contains an anonymous const dead-code witness (`const _: …`). ",
            "These witnesses were removed in issue `#89` and must not be re-introduced."
        )
    );
}
