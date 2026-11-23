//! Per-chunk line scanning for the apply-patch parser.

use super::{
    LineInfo,
    LineType,
    OperationMode,
    ParserState,
    classify_line,
    detect_mode_transition,
    handle_mode_specific_capture,
    handle_search_replace_marker,
    state::{CreateContentCapture, SearchReplaceParser},
    trim_line,
    validate_line_type,
};
use crate::dispatch::act::apply_patch::{errors::ApplyPatchError, types::FilePath};

/// Mutable state accumulated while scanning the lines of one chunk.
pub(super) struct ChunkScan {
    /// Operation mode inferred so far.
    pub(super) mode: OperationMode,
    /// Tracks SEARCH/REPLACE blocks.
    pub(super) search_replace: SearchReplaceParser,
    /// Collects the content of a created file.
    pub(super) create_capture: CreateContentCapture,
}

/// The chunk text and file path a line is scanned against.
pub(super) struct ChunkContext<'a> {
    /// Full text of the chunk.
    pub(super) chunk: &'a str,
    /// File path named by the chunk header.
    pub(super) path: &'a FilePath,
}

impl ChunkScan {
    /// Creates an empty scan with the mode still undetermined.
    pub(super) const fn new() -> Self {
        Self {
            mode: OperationMode::Unknown,
            search_replace: SearchReplaceParser::new(),
            create_capture: CreateContentCapture::new(),
        }
    }

    /// Consumes one line that starts at byte `line_start` of `chunk`.
    ///
    /// # Errors
    ///
    /// Returns a parse error when a marker cannot be recorded or the line
    /// type is invalid in its position.
    pub(super) fn consume_line(
        &mut self,
        line: &str,
        line_start: usize,
        context: &ChunkContext<'_>,
    ) -> Result<(), ApplyPatchError> {
        let trimmed = trim_line(line);
        let line_type = classify_line(trimmed);
        let line_info = LineInfo::new(line_type, line_start, line_start + line.len());

        if self.consume_marker(&line_info, context)? || self.search_replace.is_open() {
            return Ok(());
        }
        self.consume_content(line_type, trimmed, line)
    }

    /// Records a SEARCH/REPLACE marker line; returns whether the line was one.
    fn consume_marker(
        &mut self,
        line_info: &LineInfo,
        context: &ChunkContext<'_>,
    ) -> Result<bool, ApplyPatchError> {
        if !matches!(
            line_info.line_type,
            LineType::SearchMarker | LineType::Separator | LineType::ReplaceMarker
        ) {
            return Ok(false);
        }
        let mut state = ParserState::new(&mut self.mode, &mut self.search_replace);
        handle_search_replace_marker(line_info, &mut state, context.chunk, context.path)
    }

    /// Applies mode transitions and captures content for a non-marker line.
    fn consume_content(
        &mut self,
        line_type: LineType,
        trimmed: &str,
        line: &str,
    ) -> Result<(), ApplyPatchError> {
        self.mode = detect_mode_transition(trimmed, self.mode);
        if matches!(
            line_type,
            LineType::HunkHeader | LineType::DiffHeader | LineType::CreateContent
        ) {
            validate_line_type(line_type, trimmed)?;
            handle_mode_specific_capture(self.mode, line_type, line, &mut self.create_capture);
        }
        Ok(())
    }
}
