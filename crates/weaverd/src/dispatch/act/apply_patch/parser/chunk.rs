//! Line-by-line state for parsing one `diff --git` chunk.
//!
//! Keeping the accumulators in one struct lets each line be handled by a
//! short method, so the parsing decisions stay flat instead of nesting
//! conditionals inside the read loop.

use super::{
    ApplyPatchError,
    CreateContentCapture,
    FilePath,
    LineInfo,
    LineType,
    OperationMode,
    PatchOperation,
    SearchReplaceParser,
    classify_line,
    construct_operation,
    detect_mode_transition,
    handle_mode_specific_capture,
    trim_line,
    validate_line_type,
};

/// Accumulated state for one `diff --git` chunk read line by line.
pub(super) struct ChunkParser {
    /// Path of the operation being parsed.
    path: FilePath,
    /// Offset of the line the parser will read next.
    offset: usize,
    /// Operation mode inferred so far.
    mode: OperationMode,
    /// Accumulator for `SEARCH`/`REPLACE` blocks.
    search_replace: SearchReplaceParser,
    /// Accumulator for create-mode content.
    create_capture: CreateContentCapture,
}

impl ChunkParser {
    /// Seeds the accumulators for a chunk whose header ends at `start`.
    pub(super) const fn new(path: FilePath, start: usize) -> Self {
        Self {
            path,
            offset: start,
            mode: OperationMode::Unknown,
            search_replace: SearchReplaceParser::new(),
            create_capture: CreateContentCapture::new(),
        }
    }

    /// Reads one line, then reports whether it continued a search block.
    ///
    /// # Errors
    ///
    /// Forwards marker, diff-header and create-content validation errors.
    pub(super) fn advance(&mut self, chunk: &str, line: &str) -> Result<(), ApplyPatchError> {
        let trimmed = trim_line(line);
        let line_info = LineInfo::new(
            classify_line(trimmed),
            self.offset,
            self.offset + line.len(),
        );
        self.offset = line_info.end;
        if self.record_marker(&line_info, chunk)? || self.search_replace.is_open() {
            return Ok(());
        }
        self.mode = detect_mode_transition(trimmed, self.mode);
        self.capture_structured(&line_info, trimmed, line)
    }

    /// Records a SEARCH/REPLACE marker line, reporting whether it was one.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError::UnclosedReplaceBlock`] when a replace marker
    /// cannot be recorded.
    fn record_marker(
        &mut self,
        line_info: &LineInfo,
        chunk: &str,
    ) -> Result<bool, ApplyPatchError> {
        match line_info.line_type {
            LineType::SearchMarker => {
                self.mode = self.mode.promote(OperationMode::Modify);
                self.search_replace.handle_search_marker(line_info.end);
                Ok(true)
            }
            LineType::Separator => {
                self.search_replace.handle_separator(chunk, line_info.start);
                Ok(true)
            }
            LineType::ReplaceMarker => {
                self.search_replace
                    .handle_replace_marker(chunk, line_info.start, &self.path)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Validates and captures hunk metadata and create-mode content.
    ///
    /// # Errors
    ///
    /// Returns [`ApplyPatchError::InvalidDiffHeader`] when a diff header line
    /// appears in a position where it cannot be parsed.
    fn capture_structured(
        &mut self,
        line_info: &LineInfo,
        trimmed: &str,
        line: &str,
    ) -> Result<(), ApplyPatchError> {
        if !line_info.line_type.is_structured() {
            return Ok(());
        }
        validate_line_type(line_info.line_type, trimmed)?;
        handle_mode_specific_capture(
            self.mode,
            line_info.line_type,
            line,
            &mut self.create_capture,
        );
        Ok(())
    }

    /// Validates the accumulated state and builds the finished operation.
    ///
    /// # Errors
    ///
    /// Forwards block-completeness and create-content validation errors.
    pub(super) fn finish(self) -> Result<PatchOperation, ApplyPatchError> {
        self.search_replace.validate_complete(&self.path)?;
        construct_operation(
            self.mode,
            self.path,
            self.search_replace,
            self.create_capture,
        )
    }
}
