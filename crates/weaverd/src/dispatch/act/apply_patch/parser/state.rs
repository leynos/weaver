//! Parser state helpers for apply-patch operations.

use crate::dispatch::act::apply_patch::errors::ApplyPatchError;
use crate::dispatch::act::apply_patch::types::{FilePath, SearchPattern, SearchReplaceBlock};

pub(super) struct SearchReplaceParser {
    blocks: Vec<SearchReplaceBlock>,
    search_start: Option<usize>,
    replace_start: Option<usize>,
}

impl SearchReplaceParser {
    pub(super) fn new() -> Self {
        Self {
            blocks: Vec::new(),
            search_start: None,
            replace_start: None,
        }
    }

    pub(super) fn handle_search_marker(&mut self, offset: usize) {
        self.search_start = Some(offset);
        self.replace_start = None;
    }

    pub(super) fn handle_separator(&mut self, chunk: &str, line_start: usize) {
        if let Some(start) = self.search_start {
            let search = &chunk[start..line_start];
            self.replace_start = Some(line_end_from_chunk(chunk, line_start));
            self.blocks.push(SearchReplaceBlock {
                search: SearchPattern::new(search),
                replace: String::new(),
            });
        }
    }

    pub(super) fn handle_replace_marker(
        &mut self,
        chunk: &str,
        line_start: usize,
        path: &FilePath,
    ) -> Result<(), ApplyPatchError> {
        let Some(start) = self.replace_start else {
            return Err(ApplyPatchError::UnclosedSearchBlock { path: path.clone() });
        };
        let replace = &chunk[start..line_start];
        if let Some(last) = self.blocks.last_mut() {
            last.replace = replace.to_string();
        }
        self.search_start = None;
        self.replace_start = None;
        Ok(())
    }

    pub(super) fn validate_complete(&self, path: &FilePath) -> Result<(), ApplyPatchError> {
        if self.search_start.is_some() && self.replace_start.is_none() {
            return Err(ApplyPatchError::UnclosedSearchBlock { path: path.clone() });
        }
        if self.replace_start.is_some() {
            return Err(ApplyPatchError::UnclosedReplaceBlock { path: path.clone() });
        }
        Ok(())
    }

    pub(super) const fn is_open(&self) -> bool {
        self.search_start.is_some() || self.replace_start.is_some()
    }

    pub(super) fn into_blocks(self) -> Vec<SearchReplaceBlock> {
        self.blocks
    }
}

pub(super) struct CreateContentCapture {
    content: String,
    capture_hunk: bool,
    saw_hunk: bool,
}

impl CreateContentCapture {
    pub(super) fn new() -> Self {
        Self {
            content: String::new(),
            capture_hunk: false,
            saw_hunk: false,
        }
    }

    pub(super) fn handle_hunk_header(&mut self) {
        self.saw_hunk = true;
        self.capture_hunk = true;
    }

    pub(super) fn capture_line(&mut self, line: &str) {
        if !self.capture_hunk {
            return;
        }
        let (content, ending) = split_line_content(line);
        self.content.push_str(content);
        self.content.push_str(ending);
    }

    pub(super) fn validate_and_finish(self, path: &FilePath) -> Result<String, ApplyPatchError> {
        if !self.saw_hunk {
            return Err(ApplyPatchError::MissingHunk { path: path.clone() });
        }
        Ok(self.content)
    }
}

fn line_end_from_chunk(chunk: &str, line_start: usize) -> usize {
    chunk[line_start..]
        .find('\n')
        .map(|offset| line_start + offset + 1)
        .unwrap_or(chunk.len())
}

fn split_line_content(line: &str) -> (&str, &str) {
    if let Some(stripped) = line.strip_suffix("\r\n") {
        (strip_leading_plus(stripped), "\r\n")
    } else if let Some(stripped) = line.strip_suffix('\n') {
        (strip_leading_plus(stripped), "\n")
    } else {
        (strip_leading_plus(line), "")
    }
}

fn strip_leading_plus(line: &str) -> &str {
    line.strip_prefix('+').unwrap_or(line)
}
