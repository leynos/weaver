//! Search/replace matching helpers for apply-patch modifications.

use crate::dispatch::act::apply_patch::errors::ApplyPatchError;
use crate::dispatch::act::apply_patch::types::{
    FileContent, FilePath, LineEnding, SearchPattern, SearchReplaceBlock,
};

pub(crate) fn apply_search_replace(
    path: &FilePath,
    original: &FileContent,
    blocks: &[SearchReplaceBlock],
) -> Result<FileContent, ApplyPatchError> {
    let mut content = FileContent::new(original.as_str());
    let mut cursor = 0;
    let line_ending = dominant_line_ending(original.as_str());

    for (index, block) in blocks.iter().enumerate() {
        let (start, end) = if let Some(found) = find_exact(&content, cursor, &block.search) {
            found
        } else if let Some(found) = find_fuzzy(&content, cursor, &block.search) {
            found
        } else {
            return Err(ApplyPatchError::SearchBlockNotFound {
                path: path.clone(),
                block_index: index + 1,
            });
        };

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

    if crlf > 0 && crlf >= lf {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}

pub(crate) fn normalise_line_endings(input: &str, line_ending: LineEnding) -> String {
    match line_ending {
        LineEnding::Lf => input.replace("\r\n", "\n"),
        LineEnding::CrLf => input.replace("\r\n", "\n").replace('\n', "\r\n"),
    }
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
        let mut idx = 0;
        while idx < input.len() {
            let ch = input[idx..].chars().next().expect("valid utf8 char");
            let ch_len = ch.len_utf8();
            norm_to_orig.push(idx);
            for entry in orig_to_norm.iter_mut().skip(idx).take(ch_len) {
                *entry = norm_index;
            }

            if ch == '\r' && input.as_bytes().get(idx + ch_len) == Some(&b'\n') {
                normalized.push('\n');
                orig_to_norm[idx + ch_len] = norm_index;
                idx += ch_len + 1;
            } else {
                normalized.push(ch);
                idx += ch_len;
            }
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

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::dispatch::act::apply_patch::types::{
        FileContent, FilePath, SearchPattern, SearchReplaceBlock,
    };

    #[rstest]
    #[case::exact_match(
        "alpha\nbeta\ngamma\n",
        vec![SearchReplaceBlock {
            search: SearchPattern::new("beta\n"),
            replace: "delta\n".to_string(),
        }],
        "alpha\ndelta\ngamma\n",
    )]
    #[case::fuzzy_line_endings(
        "alpha\r\nbeta\r\ngamma\r\n",
        vec![SearchReplaceBlock {
            search: SearchPattern::new("beta\n"),
            replace: "delta\n".to_string(),
        }],
        "alpha\r\ndelta\r\ngamma\r\n",
    )]
    #[case::cursor_ordered(
        "one two one two",
        vec![
            SearchReplaceBlock {
                search: SearchPattern::new("one"),
                replace: "ONE".to_string(),
            },
            SearchReplaceBlock {
                search: SearchPattern::new("one"),
                replace: "UNO".to_string(),
            },
        ],
        "ONE two UNO two",
    )]
    fn apply_search_replace_succeeds(
        #[case] original: &str,
        #[case] blocks: Vec<SearchReplaceBlock>,
        #[case] expected: &str,
    ) {
        let path = FilePath::new("file.txt");
        let original = FileContent::new(original);
        let result = apply_search_replace(&path, &original, &blocks).expect("apply");
        assert_eq!(result.as_str(), expected);
    }

    #[test]
    fn apply_search_replace_rejects_missing_block() {
        let blocks = vec![SearchReplaceBlock {
            search: SearchPattern::new("missing"),
            replace: "new".to_string(),
        }];
        let path = FilePath::new("file.txt");
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
