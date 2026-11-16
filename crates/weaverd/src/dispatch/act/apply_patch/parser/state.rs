//! Parser state helpers for apply-patch operations.

use crate::dispatch::act::apply_patch::{
    errors::ApplyPatchError,
    types::{FileContent, FilePath, ReplacementText, SearchPattern, SearchReplaceBlock},
};

/// Accumulates `SEARCH`/`REPLACE` pairs while scanning one modify operation.
pub(super) struct SearchReplaceParser {
    /// Blocks captured so far, in source order.
    blocks: Vec<SearchReplaceBlock>,
    /// Byte offset where the open `SEARCH` block begins, if one is open.
    search_start: Option<usize>,
    /// Byte offset where the open `REPLACE` block begins, if one is open.
    replace_start: Option<usize>,
}

impl SearchReplaceParser {
    /// Creates a parser with no captured blocks and no open markers.
    pub(super) const fn new() -> Self {
        Self {
            blocks: Vec::new(),
            search_start: None,
            replace_start: None,
        }
    }

    /// Records the start offset of a new `SEARCH` block.
    pub(super) const fn handle_search_marker(&mut self, offset: usize) {
        self.search_start = Some(offset);
        self.replace_start = None;
    }

    /// Closes the open `SEARCH` block at a separator line.
    pub(super) fn handle_separator(&mut self, chunk: &str, line_start: usize) {
        if let Some(start) = self.search_start {
            let search = chunk.get(start..line_start).unwrap_or_default();
            self.replace_start = Some(line_end_from_chunk(chunk, line_start));
            self.blocks.push(SearchReplaceBlock {
                search: SearchPattern::new(search),
                replace: ReplacementText::new(""),
            });
        }
    }

    /// Completes the open block with the text before the `REPLACE` marker.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError::UnclosedSearchBlock`] when no separator line
    /// preceded the replace marker.
    pub(super) fn handle_replace_marker(
        &mut self,
        chunk: &str,
        line_start: usize,
        path: &FilePath,
    ) -> Result<(), ApplyPatchError> {
        let Some(start) = self.replace_start else {
            return Err(ApplyPatchError::UnclosedSearchBlock { path: path.clone() });
        };
        let replace = chunk.get(start..line_start).unwrap_or_default();
        if let Some(last) = self.blocks.last_mut() {
            last.replace = ReplacementText::new(replace);
        }
        self.search_start = None;
        self.replace_start = None;
        Ok(())
    }

    /// Checks that no marker block is still open at the end of the operation.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError::UnclosedSearchBlock`] when a `SEARCH` block
    /// never reached its separator, and
    /// [`ApplyPatchError::UnclosedReplaceBlock`] when a `REPLACE` block never
    /// reached its closing marker.
    pub(super) fn validate_complete(&self, path: &FilePath) -> Result<(), ApplyPatchError> {
        if self.search_start.is_some() && self.replace_start.is_none() {
            return Err(ApplyPatchError::UnclosedSearchBlock { path: path.clone() });
        }
        if self.replace_start.is_some() {
            return Err(ApplyPatchError::UnclosedReplaceBlock { path: path.clone() });
        }
        Ok(())
    }

    /// Reports whether a `SEARCH` or `REPLACE` block is still open.
    pub(super) const fn is_open(&self) -> bool {
        self.search_start.is_some() || self.replace_start.is_some()
    }

    /// Consumes the parser and returns the captured blocks.
    pub(super) fn into_blocks(self) -> Vec<SearchReplaceBlock> { self.blocks }
}

/// Captures added lines from a create hunk until the operation completes.
pub(super) struct CreateContentCapture {
    /// Accumulated file content, with the leading `+` already stripped.
    content: String,
    /// Whether added lines are currently being collected.
    capture_hunk: bool,
    /// Whether any hunk header has been seen at all.
    saw_hunk: bool,
}

impl CreateContentCapture {
    /// Creates a capture that is not yet collecting added lines.
    pub(super) const fn new() -> Self {
        Self {
            content: String::new(),
            capture_hunk: false,
            saw_hunk: false,
        }
    }

    /// Starts collecting added lines at a hunk header.
    pub(super) const fn handle_hunk_header(&mut self) {
        self.saw_hunk = true;
        self.capture_hunk = true;
    }

    /// Appends one added line to the captured content.
    pub(super) fn capture_line(&mut self, line: &str) {
        if !self.capture_hunk {
            return;
        }
        let (content, ending) = split_line_content(line);
        self.content.push_str(content);
        self.content.push_str(ending);
    }

    /// Validates that a hunk was seen and returns the captured content.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError::MissingHunk`] when no hunk header was seen.
    pub(super) fn validate_and_finish(
        self,
        path: &FilePath,
    ) -> Result<FileContent, ApplyPatchError> {
        if !self.saw_hunk {
            return Err(ApplyPatchError::MissingHunk { path: path.clone() });
        }
        Ok(FileContent::new(self.content))
    }
}

/// Returns the offset just past the newline that ends the line at `line_start`.
fn line_end_from_chunk(chunk: &str, line_start: usize) -> usize {
    chunk
        .get(line_start..)
        .and_then(|tail| tail.find('\n'))
        .map_or(chunk.len(), |offset| line_start + offset + 1)
}

/// Splits a diff line into its content and original line ending.
fn split_line_content(line: &str) -> (&str, &str) {
    line.strip_suffix("\r\n").map_or_else(
        || {
            line.strip_suffix('\n').map_or_else(
                || (strip_leading_plus(line), ""),
                |stripped| (strip_leading_plus(stripped), "\n"),
            )
        },
        |stripped| (strip_leading_plus(stripped), "\r\n"),
    )
}

/// Removes a leading `+` marker from an added diff line.
fn strip_leading_plus(line: &str) -> &str { line.strip_prefix('+').unwrap_or(line) }
