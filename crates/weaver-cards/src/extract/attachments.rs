//! Helpers for extracting leading comments and decorator bundles.

use weaver_syntax::SupportedLanguage;

use super::LeadingAttachments;

/// Collects the leading comment block and decorator metadata for a symbol.
pub(super) fn collect_leading_attachments(
    source: &str,
    language: SupportedLanguage,
    anchor_byte: usize,
    decorators: &[String],
) -> LeadingAttachments {
    LeadingAttachments {
        doc_comments: scan_comment_block(source, language, anchor_byte),
        decorators: decorators.to_vec(),
    }
}

/// Builds the normalised decorator representation for the card payload.
pub(super) fn normalised_decorators(decorators: &[String]) -> Vec<String> {
    decorators
        .iter()
        .map(|decorator| {
            decorator
                .trim()
                .trim_start_matches('@')
                .split('(')
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned()
        })
        .collect()
}

fn scan_comment_block(
    source: &str,
    language: SupportedLanguage,
    anchor_byte: usize,
) -> Vec<String> {
    let line_ranges = line_ranges(source);
    let Some(anchor_line) = line_index_for_byte(&line_ranges, anchor_byte) else {
        return Vec::new();
    };

    let mut comments = Vec::new();
    let mut line_index = anchor_line;
    while line_index > 0 {
        line_index -= 1;
        let line = line_text(source, &line_ranges, line_index);
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

fn line_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for line in source.split_inclusive('\n') {
        let end = start + line.len();
        ranges.push((start, end));
        start = end;
    }
    if source.is_empty() || !source.ends_with('\n') {
        ranges.push((start, source.len()));
    }
    ranges
}

fn line_index_for_byte(ranges: &[(usize, usize)], byte: usize) -> Option<usize> {
    ranges
        .iter()
        .enumerate()
        .find_map(|(index, range)| (byte >= range.0 && byte <= range.1).then_some(index))
}

fn line_text<'a>(source: &'a str, ranges: &[(usize, usize)], index: usize) -> &'a str {
    ranges
        .get(index)
        .and_then(|(start, end)| source.get(*start..*end))
        .unwrap_or_default()
}
