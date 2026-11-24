//! Patch parser for the apply-patch command.

use crate::dispatch::act::apply_patch::{
    errors::ApplyPatchError,
    types::{DiffHeaderLine, FilePath, PatchOperation, PatchText},
};

mod chunk;
mod paths;
mod state;

use self::{
    chunk::ChunkParser,
    paths::parse_diff_paths,
    state::{CreateContentCapture, SearchReplaceParser},
};

/// Line processing context containing type and position information.
struct LineInfo {
    /// Classification of the current line.
    line_type: LineType,
    /// Byte offset where the line begins within the chunk.
    start: usize,
    /// Byte offset just past the line, including its newline.
    end: usize,
}

impl LineInfo {
    /// Bundles the classification and byte range of one source line.
    const fn new(line_type: LineType, start: usize, end: usize) -> Self {
        Self {
            line_type,
            start,
            end,
        }
    }
}

/// Parses a patch stream into the operations it describes.
///
/// # Errors
///
/// Returns [`ApplyPatchError::EmptyPatch`] or [`ApplyPatchError::BinaryPatch`]
/// when the input is empty or contains NUL bytes, and forwards parsing errors
/// from [`split_operations`] and [`parse_operation`].
pub(crate) fn parse_patch(patch: &PatchText) -> Result<Vec<PatchOperation>, ApplyPatchError> {
    let text = patch.as_str();
    if text.trim().is_empty() {
        return Err(ApplyPatchError::EmptyPatch);
    }
    if text.as_bytes().contains(&0) {
        return Err(ApplyPatchError::BinaryPatch);
    }

    let chunks = split_operations(text)?;
    chunks.into_iter().map(parse_operation).collect()
}

/// Splits `patch` into one chunk per `diff --git` header.
///
/// # Errors
///
/// Returns [`ApplyPatchError::MissingDiffHeader`] when no header line is found.
fn split_operations(patch: &str) -> Result<Vec<&str>, ApplyPatchError> {
    let offsets = header_offsets(patch);
    if offsets.is_empty() {
        return Err(ApplyPatchError::MissingDiffHeader);
    }
    Ok(slice_at_offsets(patch, &offsets))
}

/// Returns the byte offset of every `diff --git` header outside a SEARCH block.
///
/// A header inside a `<<<<<<< SEARCH` / `>>>>>>> REPLACE` block is literal
/// patch content, not a chunk boundary, so it must not be recorded.
fn header_offsets(patch: &str) -> Vec<usize> {
    let mut offsets = Vec::new();
    let mut offset = 0;
    let mut in_search_replace = false;
    for line in patch.split_inclusive('\n') {
        let trimmed = trim_line(line);
        match trimmed {
            "<<<<<<< SEARCH" => in_search_replace = true,
            ">>>>>>> REPLACE" => in_search_replace = false,
            _ => {}
        }
        if !in_search_replace && trimmed.starts_with("diff --git ") {
            offsets.push(offset);
        }
        offset += line.len();
    }
    offsets
}

/// Slices `patch` at consecutive `offsets`, ending at end of input.
fn slice_at_offsets<'a>(patch: &'a str, offsets: &[usize]) -> Vec<&'a str> {
    offsets
        .iter()
        .enumerate()
        .filter_map(|(index, start)| {
            let end = offsets.get(index + 1).copied().unwrap_or(patch.len());
            patch.get(*start..end)
        })
        .collect()
}

/// Parses a single `diff --git` chunk into one patch operation.
///
/// # Errors
///
/// Returns a parse error when the chunk header is invalid, a `SEARCH` block
/// cannot be matched, or the captured operation fails validation.
fn parse_operation(chunk: &str) -> Result<PatchOperation, ApplyPatchError> {
    let (path_text, start) = parse_header(chunk)?;
    let mut parser = ChunkParser::new(FilePath::new(path_text), start);
    let remainder = chunk.get(start..).unwrap_or_default();
    for line in remainder.split_inclusive('\n') {
        parser.advance(chunk, line)?;
    }
    parser.finish()
}

/// Applies mode transitions for create/delete markers.
fn detect_mode_transition(trimmed: &str, current: OperationMode) -> OperationMode {
    let mut mode = current;
    if trimmed.starts_with("new file mode ") {
        mode = mode.promote(OperationMode::Create);
    } else if trimmed.starts_with("deleted file mode ") {
        mode = mode.promote(OperationMode::Delete);
    }
    mode
}

/// Validates line types that may raise parse errors.
///
/// # Errors
///
/// Returns [`ApplyPatchError::InvalidDiffHeader`] when a `diff --git` header
/// line appears in a position where it cannot be parsed.
fn validate_line_type(line_type: LineType, trimmed: &str) -> Result<(), ApplyPatchError> {
    if line_type == LineType::DiffHeader {
        return Err(ApplyPatchError::InvalidDiffHeader {
            line: trimmed.to_owned(),
        });
    }
    Ok(())
}

/// Captures create-mode hunk metadata and added lines.
fn handle_mode_specific_capture(
    mode: OperationMode,
    line_type: LineType,
    line: &str,
    create_capture: &mut CreateContentCapture,
) {
    match (mode, line_type) {
        (OperationMode::Create, LineType::HunkHeader) => create_capture.handle_hunk_header(),
        (OperationMode::Create, LineType::CreateContent) => create_capture.capture_line(line),
        _ => {}
    }
}

/// Parses the leading `diff --git` header and returns its path and end offset.
///
/// # Errors
///
/// Returns [`ApplyPatchError::MissingDiffHeader`] when the chunk is empty and
/// [`ApplyPatchError::InvalidDiffHeader`] when the header or its paths are
/// malformed.
fn parse_header(chunk: &str) -> Result<(String, usize), ApplyPatchError> {
    if chunk.is_empty() {
        return Err(ApplyPatchError::MissingDiffHeader);
    }

    let mut lines = chunk.split_inclusive('\n');
    let Some(line) = lines.next() else {
        return Err(ApplyPatchError::MissingDiffHeader);
    };
    let line_end = line.len();
    let trimmed = trim_line(line);
    if !trimmed.starts_with("diff --git ") {
        return Err(ApplyPatchError::InvalidDiffHeader {
            line: trimmed.to_owned(),
        });
    }

    let header = DiffHeaderLine::new(trimmed);
    let (_, b_path) = parse_diff_paths(header.as_str())?;
    let path = strip_b_prefix(&b_path);
    Ok((path, line_end))
}

/// Classification of a trimmed patch line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineType {
    /// The `<<<<<<< SEARCH` marker opening a search block.
    SearchMarker,
    /// The `=======` separator between search and replace text.
    Separator,
    /// The `>>>>>>> REPLACE` marker closing a replace block.
    ReplaceMarker,
    /// A `@@` hunk header line.
    HunkHeader,
    /// A `diff --git` header line.
    DiffHeader,
    /// A line beginning with `+` inside a create hunk.
    CreateContent,
    /// Any other line that carries no parsing meaning.
    Other,
}

impl LineType {
    /// Reports whether the line carries mode-specific metadata or content.
    const fn is_structured(self) -> bool {
        matches!(
            self,
            Self::HunkHeader | Self::DiffHeader | Self::CreateContent
        )
    }
}

/// Classifies a trimmed patch line by its marker prefix.
fn classify_line(trimmed: &str) -> LineType {
    if trimmed == "<<<<<<< SEARCH" {
        LineType::SearchMarker
    } else if trimmed == "=======" {
        LineType::Separator
    } else if trimmed == ">>>>>>> REPLACE" {
        LineType::ReplaceMarker
    } else if trimmed.starts_with("@@") {
        LineType::HunkHeader
    } else if trimmed.starts_with("diff --git ") {
        LineType::DiffHeader
    } else if trimmed.starts_with('+') {
        LineType::CreateContent
    } else {
        LineType::Other
    }
}

/// Converts the accumulated parser state into a single patch operation.
///
/// # Errors
///
/// Returns [`ApplyPatchError::MissingSearchReplace`] when a modify operation
/// captured no blocks, and forwards create validation errors. Returns
/// [`ApplyPatchError::MissingDiffHeader`] when the mode stayed unknown.
fn construct_operation(
    mode: OperationMode,
    path: FilePath,
    search_replace: SearchReplaceParser,
    create_capture: CreateContentCapture,
) -> Result<PatchOperation, ApplyPatchError> {
    match mode {
        OperationMode::Modify => {
            let blocks = search_replace.into_blocks();
            if blocks.is_empty() {
                return Err(ApplyPatchError::MissingSearchReplace { path });
            }
            Ok(PatchOperation::Modify { path, blocks })
        }
        OperationMode::Create => {
            let content = create_capture.validate_and_finish(&path)?;
            Ok(PatchOperation::Create { path, content })
        }
        OperationMode::Delete => Ok(PatchOperation::Delete { path }),
        OperationMode::Unknown => Err(ApplyPatchError::MissingDiffHeader),
    }
}

/// Strips trailing CR and LF bytes from `line`.
fn trim_line(line: &str) -> &str { line.trim_end_matches(['\n', '\r']) }

/// Removes a leading `b/` or `b\` prefix from a diff path.
fn strip_b_prefix(path: &str) -> String {
    path.strip_prefix("b/")
        .or_else(|| path.strip_prefix("b\\"))
        .unwrap_or(path)
        .to_owned()
}

/// Mode inferred from the diff markers seen so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationMode {
    /// No mode-bearing marker has been seen yet.
    Unknown,
    /// The chunk modifies an existing file.
    Modify,
    /// The chunk creates a new file.
    Create,
    /// The chunk deletes an existing file.
    Delete,
}

impl OperationMode {
    /// Promotes the current mode to `next`, keeping an established mode.
    const fn promote(self, next: Self) -> Self {
        match (self, next) {
            (Self::Unknown, other) => other,
            (Self::Modify, Self::Modify) => Self::Modify,
            (Self::Create, Self::Create) => Self::Create,
            (Self::Delete, Self::Delete) => Self::Delete,
            (current, _) => current,
        }
    }
}

#[cfg(test)]
mod tests;
