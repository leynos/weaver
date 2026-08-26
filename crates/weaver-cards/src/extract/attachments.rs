//! Helpers for extracting leading comments and decorator bundles.

use weaver_syntax::SupportedLanguage;

use super::LeadingAttachments;

/// A source text pre-indexed into per-line byte ranges, so repeated
/// backward scans over the leading comment block can look up lines by byte
/// offset without re-splitting the text on every call.
struct SourceLines<'a> {
    /// Full source text the byte ranges index into.
    text: &'a str,
    /// Half-open `(start, end)` byte range for each line, in source order,
    /// including the trailing `\n` where present.
    ranges: Vec<(usize, usize)>,
}

impl<'a> SourceLines<'a> {
    /// Splits `text` into indexed lines. A trailing partial line (or empty
    /// input) is still recorded as its own range so callers can look up the
    /// final line without special-casing files that lack a closing newline.
    fn new(text: &'a str) -> Self {
        let mut ranges = Vec::new();
        let mut start = 0;
        for line in text.split_inclusive('\n') {
            let end = start + line.len();
            ranges.push((start, end));
            start = end;
        }
        if text.is_empty() || !text.ends_with('\n') {
            ranges.push((start, text.len()));
        }

        Self { text, ranges }
    }

    /// Finds the index of the line containing `byte`, or `None` when `byte`
    /// is past the end of the indexed text (for example, an empty file).
    fn line_index_for_byte(&self, byte: usize) -> Option<usize> {
        self.ranges
            .iter()
            .enumerate()
            .find_map(|(index, range)| (byte >= range.0 && byte < range.1).then_some(index))
    }

    /// Returns the text of the line at `index`, or an empty string when
    /// `index` is out of range, so callers can index freely while scanning.
    fn line_text(&self, index: usize) -> &str {
        self.ranges
            .get(index)
            .and_then(|(start, end)| self.text.get(*start..*end))
            .unwrap_or_default()
    }
}

/// Internal wrapper around a raw decorator token captured during extraction.
///
/// The wrapped string is preserved exactly as collected from the language
/// extractor so callers can still emit the original decorator text in the card
/// payload. The only invariant relied upon here is that it represents one
/// decorator-like token from the parse tree rather than an arbitrary block of
/// source text.
pub(super) struct Decorator(String);

impl Decorator {
    /// Normalises the decorator name for attachment fingerprints and payloads.
    ///
    /// This returns an owned [`String`] with surrounding whitespace trimmed,
    /// any leading `@` removed, any argument suffix starting at the first `(`
    /// discarded, and the remaining name trimmed again.
    ///
    /// Examples: `@route("a  b") -> "route"`, `  @sealed  -> "sealed"`,
    /// `decorator -> "decorator"`.
    ///
    /// The implementation never panics; it uses `unwrap_or_default()` when the
    /// decorator text contains no split segment.
    pub(super) fn normalise(&self) -> String {
        self.0
            .trim()
            .trim_start_matches('@')
            .split('(')
            .next()
            .unwrap_or_default()
            .trim()
            .to_owned()
    }
}

impl From<&String> for Decorator {
    fn from(value: &String) -> Self { Self(value.clone()) }
}

/// Collects the leading comment block and decorator metadata for a symbol.
pub(super) fn collect_leading_attachments(
    source: &str,
    language: SupportedLanguage,
    anchor_byte: usize,
    decorators: &[Decorator],
) -> LeadingAttachments {
    let lines = SourceLines::new(source);
    LeadingAttachments {
        doc_comments: scan_comment_block(&lines, language, anchor_byte),
        decorators: decorators
            .iter()
            .map(|decorator| decorator.0.clone())
            .collect(),
    }
}

/// Builds the normalized decorator representation for the card payload.
pub(super) fn normalised_decorators(decorators: &[Decorator]) -> Vec<String> {
    decorators.iter().map(Decorator::normalise).collect()
}

/// Scans backwards from the symbol's anchor line to collect a contiguous
/// leading comment block, stopping at the first blank line or non-comment
/// line encountered. The result is in source order (oldest comment first),
/// since the backward scan collects newest-first and reverses at the end.
fn scan_comment_block(
    lines: &SourceLines<'_>,
    language: SupportedLanguage,
    anchor_byte: usize,
) -> Vec<String> {
    let Some(anchor_line) = lines.line_index_for_byte(anchor_byte) else {
        return Vec::new();
    };

    let mut comments = Vec::new();
    let mut line_index = anchor_line;
    while line_index > 0 {
        line_index -= 1;
        let line = lines.line_text(line_index);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(comment) = normalise_comment_line(trimmed, language) {
            comments.push(comment);
            continue;
        }
        break;
    }

    comments.reverse();
    comments
}

/// Dispatches comment-marker stripping to the syntax appropriate for
/// `language`, so the same scan loop works across Rust, Python, and
/// TypeScript comment conventions.
fn normalise_comment_line(line: &str, language: SupportedLanguage) -> Option<String> {
    match language {
        SupportedLanguage::Rust => rust_comment(line),
        SupportedLanguage::Python => python_comment(line),
        SupportedLanguage::TypeScript => ts_comment(line),
    }
}

/// Strips a Rust line- or block-comment marker (`///`, `//!`, `/**`, `/*!`,
/// `//`, or a block continuation/closer) from `line`. Returns `None` when
/// `line` carries no recognized comment marker, signalling the end of the
/// leading comment block to the caller.
fn rust_comment(line: &str) -> Option<String> {
    for prefix in ["///", "//!", "/**", "/*!", "//", "*/", "*"] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return trim_comment_body(rest);
        }
    }
    None
}

/// Strips a Python `#` comment marker from `line`. Returns `None` when
/// `line` has no `#` prefix, ending the leading comment scan.
fn python_comment(line: &str) -> Option<String> {
    line.strip_prefix('#').map(|rest| rest.trim().to_owned())
}

/// Strips a TypeScript/JSDoc line- or block-comment marker (`/**`, `/*`,
/// `*/`, a block continuation `*`, or `//`) from `line`. Returns `None` when
/// `line` carries no recognized comment marker.
fn ts_comment(line: &str) -> Option<String> {
    for prefix in ["/**", "/*", "*/", "*", "//"] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return trim_comment_body(rest);
        }
    }
    None
}

/// Trims whitespace and a trailing block-comment closer (`*/`) from a
/// comment marker's remainder, returning `None` when nothing but whitespace
/// and the closer are left, so purely decorative lines (like a lone `*/`)
/// are excluded from the collected doc comments.
fn trim_comment_body(rest: &str) -> Option<String> {
    let trimmed = rest.trim();
    let without_closer = trimmed
        .strip_suffix("*/")
        .map_or(trimmed, str::trim_end)
        .trim();
    (!without_closer.is_empty()).then(|| String::from(without_closer))
}

#[cfg(test)]
mod tests {
    //! Tests for comment extraction from Rust and TypeScript source files.

    use super::{rust_comment, ts_comment};

    #[test]
    fn rust_block_comment_strips_closer() {
        assert_eq!(rust_comment("/** hello */").as_deref(), Some("hello"));
    }

    #[test]
    fn rust_standalone_block_closer_is_ignored() {
        assert_eq!(rust_comment("*/"), None);
    }

    #[test]
    fn ts_block_comment_strips_closer() {
        assert_eq!(ts_comment("/** hello */").as_deref(), Some("hello"));
    }

    #[test]
    fn ts_standalone_block_closer_is_ignored() {
        assert_eq!(ts_comment("*/"), None);
    }
}
