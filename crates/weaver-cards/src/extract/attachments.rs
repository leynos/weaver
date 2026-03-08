//! Helpers for extracting leading comments and decorator bundles.

use weaver_syntax::SupportedLanguage;

use super::LeadingAttachments;

struct SourceLines<'a> {
    text: &'a str,
    ranges: Vec<(usize, usize)>,
}

impl<'a> SourceLines<'a> {
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

    fn line_index_for_byte(&self, byte: usize) -> Option<usize> {
        self.ranges
            .iter()
            .enumerate()
            .find_map(|(index, range)| (byte >= range.0 && byte <= range.1).then_some(index))
    }

    fn line_text(&self, index: usize) -> &str {
        self.ranges
            .get(index)
            .and_then(|(start, end)| self.text.get(*start..*end))
            .unwrap_or_default()
    }
}

pub(super) struct Decorator(String);

impl Decorator {
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
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
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

/// Builds the normalised decorator representation for the card payload.
pub(super) fn normalised_decorators(decorators: &[Decorator]) -> Vec<String> {
    decorators.iter().map(Decorator::normalise).collect()
}

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
            continue;
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

fn normalise_comment_line(line: &str, language: SupportedLanguage) -> Option<String> {
    match language {
        SupportedLanguage::Rust => rust_comment(line),
        SupportedLanguage::Python => python_comment(line),
        SupportedLanguage::TypeScript => ts_comment(line),
    }
}

fn rust_comment(line: &str) -> Option<String> {
    for prefix in ["///", "//!", "/**", "/*!", "//", "*", "*/"] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(rest.trim().to_owned());
        }
    }
    None
}

fn python_comment(line: &str) -> Option<String> {
    line.strip_prefix('#').map(|rest| rest.trim().to_owned())
}

fn ts_comment(line: &str) -> Option<String> {
    for prefix in ["/**", "/*", "*/", "*", "//"] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(rest.trim().to_owned());
        }
    }
    None
}
