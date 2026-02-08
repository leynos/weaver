//! Search/replace matching helpers for apply-patch modifications.

use crate::dispatch::act::apply_patch::errors::ApplyPatchError;
use crate::dispatch::act::apply_patch::types::{
    FileContent, FilePath, LineEnding, SearchPattern, SearchReplaceBlock,
};

/// Applies SEARCH/REPLACE blocks to the provided file content in order.
///
/// Uses the patch path and block index to report errors when a block cannot be
/// matched, normalises replacement line endings to the dominant line ending in
/// the original content, and updates the match cursor after each replacement.
///
/// # Errors
///
/// Returns `ApplyPatchError::SearchBlockNotFound` when a search block does not
/// match the remaining content in sequence.
pub(crate) fn apply_search_replace(
    path: &FilePath,
    original: &FileContent,
    blocks: &[SearchReplaceBlock],
) -> Result<FileContent, ApplyPatchError> {
    let mut content = FileContent::new(original.as_str());
    let mut cursor = 0;
    let line_ending = dominant_line_ending(original.as_str());

    for (index, block) in blocks.iter().enumerate() {
        let (start, end) = find_exact(&content, cursor, &block.search)
            .or_else(|| find_fuzzy(&content, cursor, &block.search))
            .ok_or_else(|| ApplyPatchError::SearchBlockNotFound {
                path: path.clone(),
                block_index: index + 1,
            })?;

        let replacement = normalise_line_endings(block.replace.as_str(), line_ending);
        content.replace_range(start..end, &replacement);
        cursor = start + replacement.len();
    }

    Ok(content)
}

fn find_exact(
    content: &FileContent,
    cursor: usize,
    search: &SearchPattern,
) -> Option<(usize, usize)> {
    debug_assert!(
        cursor <= content.as_str().len(),
        "exact search cursor out of bounds"
    );
    let content = content.as_str();
    let search = search.as_str();
    content[cursor..].find(search).map(|offset| {
        let start = cursor + offset;
        let end = start + search.len();
        (start, end)
    })
}

fn find_fuzzy(
    content: &FileContent,
    cursor: usize,
    search: &SearchPattern,
) -> Option<(usize, usize)> {
    debug_assert!(
        cursor <= content.as_str().len(),
        "fuzzy search cursor out of bounds"
    );
    let normalized_content = NormalizedContent::new(content.as_str());
    let normalized_search = normalise_line_endings(search.as_str(), LineEnding::Lf);
    let trimmed_search = trim_fuzzy_whitespace(&normalized_search);
    if trimmed_search.is_empty() {
        return None;
    }

    let cursor_norm = normalized_content.orig_to_norm(cursor)?;
    let haystack = &normalized_content.normalized[cursor_norm..];
    let offset = haystack.find(trimmed_search)?;
    let start_norm = cursor_norm + offset;
    let end_norm = start_norm + trimmed_search.len();
    let start_orig = normalized_content.norm_to_orig(start_norm)?;
    let end_orig = normalized_content.norm_to_orig(end_norm)?;
    Some((start_orig, end_orig))
}

fn trim_fuzzy_whitespace(value: &str) -> &str {
    value.trim_matches(|ch| ch == ' ' || ch == '\t')
}

fn dominant_line_ending(content: &str) -> LineEnding {
    let mut crlf = 0;
    let mut lf = 0;
    let bytes = content.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'\r' && bytes.get(idx + 1) == Some(&b'\n') {
            crlf += 1;
            idx += 2;
            continue;
        }
        if bytes[idx] == b'\n' {
            lf += 1;
        }
        idx += 1;
    }

    // Tie-breaker: when counts are equal and CRLF is present, prefer CRLF.
    if crlf > 0 && crlf >= lf {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}

/// Normalises line endings in text to the specified line ending style.
///
/// Converts CRLF to LF when targeting `LineEnding::Lf` and converts bare LF to
/// CRLF when targeting `LineEnding::CrLf`. Other characters are preserved.
pub(crate) fn normalise_line_endings(input: &str, line_ending: LineEnding) -> String {
    match line_ending {
        LineEnding::Lf => input.replace("\r\n", "\n"),
        LineEnding::CrLf => normalise_line_endings_crlf(input),
    }
}

/// Processes a single character for CRLF normalisation.
/// Returns the string to append and whether to skip the next char.
fn process_char_for_crlf(ch: char, next_char: Option<char>) -> (&'static str, bool) {
    match (ch, next_char) {
        ('\r', Some('\n')) => ("\r\n", true),
        ('\r', _) => ("\r", false),
        ('\n', _) => ("\r\n", false),
        _ => ("", false),
    }
}

fn normalise_line_endings_crlf(input: &str) -> String {
    // Phase 1: Calculate required capacity
    let mut extra = 0;
    let mut prev_cr = false;
    for byte in input.as_bytes() {
        let is_lf = *byte == b'\n';
        if is_lf && !prev_cr {
            extra += 1;
        }
        prev_cr = *byte == b'\r';
    }

    // Phase 2: Build normalised output
    let mut output = String::with_capacity(input.len() + extra);
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        let next = chars.peek().copied();
        let (line_ending, skip_next) = process_char_for_crlf(ch, next);
        if !line_ending.is_empty() {
            output.push_str(line_ending);
            if skip_next {
                chars.next();
            }
        } else {
            output.push(ch);
        }
    }
    output
}

struct NormalizedContent {
    normalized: String,
    norm_to_orig: Vec<usize>,
    orig_to_norm: Vec<usize>,
}

impl NormalizedContent {
    fn new(input: &str) -> Self {
        let mut normalized = String::with_capacity(input.len());
        let mut norm_to_orig = Vec::new();
        let mut orig_to_norm = vec![0; input.len() + 1];

        let mut norm_index = 0;
        let mut iter = input.char_indices().peekable();
        while let Some((idx, ch)) = iter.next() {
            let ch_len = ch.len_utf8();
            norm_to_orig.push(idx);
            for entry in orig_to_norm.iter_mut().skip(idx).take(ch_len) {
                *entry = norm_index;
            }

            if ch == '\r'
                && let Some(&(next_idx, '\n')) = iter.peek()
            {
                map_crlf_indices(&mut orig_to_norm, next_idx, norm_index);
                normalized.push('\n');
                iter.next();
                norm_index += 1;
                continue;
            }

            normalized.push(ch);
            norm_index += 1;
        }
        orig_to_norm[input.len()] = norm_index;
        norm_to_orig.push(input.len());

        Self {
            normalized,
            norm_to_orig,
            orig_to_norm,
        }
    }

    fn norm_to_orig(&self, index: usize) -> Option<usize> {
        self.norm_to_orig.get(index).copied()
    }

    fn orig_to_norm(&self, index: usize) -> Option<usize> {
        self.orig_to_norm.get(index).copied()
    }
}

fn map_crlf_indices(orig_to_norm: &mut [usize], next_idx: usize, norm_index: usize) {
    let next_len = '\n'.len_utf8();
    for entry in orig_to_norm.iter_mut().skip(next_idx).take(next_len) {
        *entry = norm_index;
    }
}

#[cfg(test)]
mod tests {
    //! Tests for apply-patch matcher helpers.

    use rstest::fixture;
    use rstest::rstest;

    use super::*;
    use crate::dispatch::act::apply_patch::types::{
        FileContent, FilePath, ReplacementText, SearchPattern, SearchReplaceBlock,
    };

    #[fixture]
    fn path() -> FilePath {
        FilePath::new("file.txt")
    }

    #[rstest]
    #[case::exact_match(
        "alpha\nbeta\ngamma\n",
        vec![SearchReplaceBlock {
            search: SearchPattern::new("beta\n"),
            replace: ReplacementText::new("delta\n"),
        }],
        "alpha\ndelta\ngamma\n",
    )]
    #[case::fuzzy_line_endings(
        "alpha\r\nbeta\r\ngamma\r\n",
        vec![SearchReplaceBlock {
            search: SearchPattern::new("beta\n"),
            replace: ReplacementText::new("delta\n"),
        }],
        "alpha\r\ndelta\r\ngamma\r\n",
    )]
    #[case::cursor_ordered(
        "one two one two",
        vec![
            SearchReplaceBlock {
                search: SearchPattern::new("one"),
                replace: ReplacementText::new("ONE"),
            },
            SearchReplaceBlock {
                search: SearchPattern::new("one"),
                replace: ReplacementText::new("UNO"),
            },
        ],
        "ONE two UNO two",
    )]
    fn apply_search_replace_succeeds(
        path: FilePath,
        #[case] original: &str,
        #[case] blocks: Vec<SearchReplaceBlock>,
        #[case] expected: &str,
    ) {
        let original = FileContent::new(original);
        let result = apply_search_replace(&path, &original, &blocks).expect("apply");
        assert_eq!(result.as_str(), expected);
    }

    #[rstest]
    fn apply_search_replace_rejects_missing_block(path: FilePath) {
        let blocks = vec![SearchReplaceBlock {
            search: SearchPattern::new("missing"),
            replace: ReplacementText::new("new"),
        }];
        let original = FileContent::new("content");
        let error = apply_search_replace(&path, &original, &blocks).expect_err("error");
        match error {
            ApplyPatchError::SearchBlockNotFound { path, block_index } => {
                assert_eq!(path.as_str(), "file.txt");
                assert_eq!(block_index, 1);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
