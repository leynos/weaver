//! Programmatic regression test asserting that
//! `crates/weaverd/src/dispatch/act/refactor/refactor_helpers.rs`
//! contains no forbidden lint-suppression patterns.
//!
//! This guards against re-introduction of:
//! - File-wide blanket `#![allow(…)]` attributes.
//! - Item-level `#[allow(dead_code` suppressions.
//! - Anonymous const dead-code witnesses (`const _:`).

const HELPER_SRC: &str = include_str!("../src/dispatch/act/refactor/refactor_helpers.rs");

fn refactor_helpers_source() -> &'static str {
    HELPER_SRC.split("#[cfg(test)]").next().unwrap_or("")
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

#[test]
fn no_file_wide_blanket_allow() {
    assert!(
        !contains_forbidden_pattern(refactor_helpers_source(), "#![allow("),
        concat!(
            "refactor_helpers.rs contains a forbidden file-wide `#![allow(…)]` attribute. ",
            "File-wide blanket allows are banned by project lint policy."
        )
    );
}

#[test]
fn refactor_helpers_avoids_dead_code_suppression_patterns() {
    let compact = normalize_whitespace(refactor_helpers_source());
    let file_wide_allow = ["#![", "allow("].concat();
    let item_dead_code_allow = ["#[", "allow(dead_code"].concat();
    let dead_code_witness = "const_:";

    assert!(
        !compact.contains(&file_wide_allow),
        "refactor_helpers.rs contains forbidden pattern `{file_wide_allow}`; file-wide blanket \
         lint allows are banned by project policy.",
    );
    assert!(
        !compact.contains(&item_dead_code_allow),
        "refactor_helpers.rs contains forbidden pattern `{item_dead_code_allow}`; item-level \
         dead-code allows without a reason are banned by project policy.",
    );
    assert!(
        !compact.contains(dead_code_witness),
        "refactor_helpers.rs contains forbidden pattern `{dead_code_witness}`; anonymous const \
         witnesses must not be used to mask dead-code lints.",
    );
}

#[test]
fn no_item_level_dead_code_allow() {
    assert!(
        !contains_forbidden_pattern(refactor_helpers_source(), "#[allow(dead_code"),
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
        !contains_forbidden_pattern(refactor_helpers_source(), "const _:"),
        concat!(
            "refactor_helpers.rs contains an anonymous const dead-code witness (`const _: …`). ",
            "These witnesses were removed in issue `#89` and must not be re-introduced."
        )
    );
}
