//! Diff-header path and token parsing for the apply-patch parser.
//!
//! A `diff --git` header names two paths, either of which may be quoted and may
//! contain spaces. These helpers tokenize the header without allocating a
//! throwaway `Vec` per line and reject the malformed forms the parser must
//! report as [`ApplyPatchError::InvalidDiffHeader`].

use super::ApplyPatchError;

/// Reads the two paths from a `diff --git` header line.
///
/// # Errors
///
/// Returns [`ApplyPatchError::InvalidDiffHeader`] when the prefix is missing,
/// the line is unterminated, or fewer than two paths are present.
pub(super) fn parse_diff_paths(line: &str) -> Result<(String, String), ApplyPatchError> {
    let remainder =
        line.strip_prefix("diff --git ")
            .ok_or_else(|| ApplyPatchError::InvalidDiffHeader {
                line: line.to_owned(),
            })?;
    let mut chars = remainder.chars().peekable();
    let mut tokens = Vec::new();

    while tokens.len() < 2 {
        let Some(parsed_token) = parse_next_token(&mut chars, line)? else {
            break;
        };
        if !parsed_token.is_empty() {
            tokens.push(parsed_token);
        }
    }

    let [first, second] =
        <[String; 2]>::try_from(tokens).map_err(|_| ApplyPatchError::InvalidDiffHeader {
            line: line.to_owned(),
        })?;
    Ok((first, second))
}

/// Reads the next whitespace-delimited or quoted path token.
///
/// # Errors
///
/// Returns [`ApplyPatchError::InvalidDiffHeader`] when a quoted token is not
/// terminated.
fn parse_next_token(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    line: &str,
) -> Result<Option<String>, ApplyPatchError> {
    consume_whitespace(chars);
    let Some(next) = chars.peek().copied() else {
        return Ok(None);
    };
    if next == '"' {
        chars.next();
        read_quoted_token(chars, line).map(Some)
    } else {
        Ok(Some(read_unquoted_token(chars)))
    }
}

/// Advances past any whitespace at the front of `chars`.
fn consume_whitespace(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) {
        chars.next();
    }
}

/// Reads a double-quoted path token, consuming the closing quote.
///
/// # Errors
///
/// Returns [`ApplyPatchError::InvalidDiffHeader`] when the input ends before
/// the closing quote.
fn read_quoted_token(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    line: &str,
) -> Result<String, ApplyPatchError> {
    let mut value = String::new();
    for ch in chars.by_ref() {
        if ch == '"' {
            return Ok(value);
        }
        value.push(ch);
    }
    Err(ApplyPatchError::InvalidDiffHeader {
        line: line.to_owned(),
    })
}

/// Reads a whitespace-delimited path token from `chars`.
fn read_unquoted_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut value = String::new();
    while let Some(ch) = chars.peek().copied() {
        if ch.is_whitespace() {
            break;
        }
        value.push(ch);
        chars.next();
    }
    value
}
