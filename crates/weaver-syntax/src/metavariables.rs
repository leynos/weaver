//! Metavariable parsing helpers shared across modules.
//!
//! Weaver patterns and rewrite templates use `$NAME` and `$$$NAME` metavariables.
//! This module centralises the name rules so parsing stays consistent.

pub(crate) const METAVAR_PLACEHOLDER_PREFIX: &str = "__WEAVER_METAVAR_";
pub(crate) const METAVAR_PLACEHOLDER_SUFFIX: &str = "__";

/// Returns whether `c` is a valid first character for a metavariable name.
///
/// Metavariable names must begin with an ASCII uppercase letter or `_`.
#[must_use]
pub(crate) const fn is_valid_metavar_start_char(c: char) -> bool {
    c.is_ascii_uppercase() || c == '_'
}

/// Returns whether `c` is a valid continuation character for a metavariable name.
///
/// After the first character, metavariable names may contain ASCII uppercase
/// letters, ASCII digits, or `_`.
#[must_use]
pub(crate) const fn is_valid_metavar_continuation_char(c: char) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'
}

/// Extracts a metavariable name from a character stream.
///
/// The stream is expected to be positioned at the first character after the `$`
/// prefix. Returns an empty string if the next character is not a valid start.
pub(crate) fn extract_metavar_name(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> String {
    let mut name = String::new();

    let Some((_, first_char)) = chars.peek().copied() else {
        return name;
    };

    if !is_valid_metavar_start_char(first_char) {
        return name;
    }

    name.push(first_char);
    chars.next();

    while let Some((_, c)) = chars.peek().copied() {
        if !is_valid_metavar_continuation_char(c) {
            break;
        }
        name.push(c);
        chars.next();
    }

    name
}

/// Builds the identifier used to represent a metavariable in a normalised pattern.
#[must_use]
pub(crate) fn placeholder_for_metavar(name: &str) -> String {
    format!("{METAVAR_PLACEHOLDER_PREFIX}{name}{METAVAR_PLACEHOLDER_SUFFIX}")
}

/// Extracts the metavariable name from a normalised placeholder identifier.
#[must_use]
pub(crate) fn metavar_name_from_placeholder(text: &str) -> Option<&str> {
    text.strip_prefix(METAVAR_PLACEHOLDER_PREFIX)
        .and_then(|rest| rest.strip_suffix(METAVAR_PLACEHOLDER_SUFFIX))
}
