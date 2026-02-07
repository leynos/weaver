//! Patch parser for the apply-patch command.

use crate::dispatch::act::apply_patch::errors::ApplyPatchError;
use crate::dispatch::act::apply_patch::types::{
    DiffHeaderLine, FilePath, PatchOperation, PatchText,
};

mod state;

use self::state::{CreateContentCapture, SearchReplaceParser};

/// Line processing context containing type and position information.
struct LineInfo {
    line_type: LineType,
    start: usize,
    end: usize,
}

impl LineInfo {
    fn new(line_type: LineType, start: usize, end: usize) -> Self {
        Self {
            line_type,
            start,
            end,
        }
    }
}

/// Mutable parser state for search/replace operations.
struct ParserState<'a> {
    mode: &'a mut OperationMode,
    search_replace: &'a mut SearchReplaceParser,
}

impl<'a> ParserState<'a> {
    fn new(mode: &'a mut OperationMode, search_replace: &'a mut SearchReplaceParser) -> Self {
        Self {
            mode,
            search_replace,
        }
    }
}

pub(crate) fn parse_patch(patch: &PatchText) -> Result<Vec<PatchOperation>, ApplyPatchError> {
    let patch = patch.as_str();
    if patch.trim().is_empty() {
        return Err(ApplyPatchError::EmptyPatch);
    }
    if patch.as_bytes().contains(&0) {
        return Err(ApplyPatchError::BinaryPatch);
    }

    let chunks = split_operations(patch)?;
    chunks.into_iter().map(parse_operation).collect()
}

fn split_operations(patch: &str) -> Result<Vec<&str>, ApplyPatchError> {
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

    if offsets.is_empty() {
        return Err(ApplyPatchError::MissingDiffHeader);
    }

    let mut chunks = Vec::new();
    for (index, start) in offsets.iter().enumerate() {
        let end = offsets.get(index + 1).copied().unwrap_or(patch.len());
        chunks.push(&patch[*start..end]);
    }

    Ok(chunks)
}

fn parse_operation(chunk: &str) -> Result<PatchOperation, ApplyPatchError> {
    let (path, mut offset) = parse_header(chunk)?;
    let path = FilePath::new(path);
    let mut mode = OperationMode::Unknown;
    let mut search_replace = SearchReplaceParser::new();
    let mut create_capture = CreateContentCapture::new();

    for line in chunk[offset..].split_inclusive('\n') {
        let line_start = offset;
        let line_end = offset + line.len();
        let trimmed = trim_line(line);
        let line_type = classify_line(trimmed);
        let line_info = LineInfo::new(line_type, line_start, line_end);

        if matches!(
            line_type,
            LineType::SearchMarker | LineType::Separator | LineType::ReplaceMarker
        ) {
            let state = ParserState::new(&mut mode, &mut search_replace);
            if handle_search_replace_marker(&line_info, state, chunk, &path)? {
                offset = line_info.end;
                continue;
            }
        }
        if search_replace.is_open() {
            offset = line_end;
            continue;
        }

        mode = detect_mode_transition(trimmed, mode);
        if matches!(
            line_type,
            LineType::HunkHeader | LineType::DiffHeader | LineType::CreateContent
        ) {
            validate_line_type(line_type, trimmed)?;
            handle_mode_specific_capture(mode, line_type, line, &mut create_capture);
        }

        offset = line_end;
    }

    search_replace.validate_complete(&path)?;
    construct_operation(mode, path, search_replace, create_capture)
}

/// Handles SEARCH/REPLACE marker lines without mutating the cursor offset.
// Required to pass explicit marker context without threading local structs.
fn handle_search_replace_marker(
    line_info: &LineInfo,
    state: ParserState<'_>,
    chunk: &str,
    path: &FilePath,
) -> Result<bool, ApplyPatchError> {
    match line_info.line_type {
        LineType::SearchMarker => {
            *state.mode = state.mode.promote(OperationMode::Modify);
            state.search_replace.handle_search_marker(line_info.end);
            Ok(true)
        }
        LineType::Separator => {
            state
                .search_replace
                .handle_separator(chunk, line_info.start);
            Ok(true)
        }
        LineType::ReplaceMarker => {
            state
                .search_replace
                .handle_replace_marker(chunk, line_info.start, path)?;
            Ok(true)
        }
        _ => Ok(false),
    }
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
fn validate_line_type(line_type: LineType, trimmed: &str) -> Result<(), ApplyPatchError> {
    if line_type == LineType::DiffHeader {
        return Err(ApplyPatchError::InvalidDiffHeader {
            line: trimmed.to_string(),
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
            line: trimmed.to_string(),
        });
    }

    let header = DiffHeaderLine::new(trimmed);
    let (_, b_path) = parse_diff_paths(header.as_str())?;
    let path = strip_b_prefix(&b_path);
    Ok((path, line_end))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineType {
    SearchMarker,
    Separator,
    ReplaceMarker,
    HunkHeader,
    DiffHeader,
    CreateContent,
    Other,
}

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

fn trim_line(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

fn parse_diff_paths(line: &str) -> Result<(String, String), ApplyPatchError> {
    let remainder =
        line.strip_prefix("diff --git ")
            .ok_or_else(|| ApplyPatchError::InvalidDiffHeader {
                line: line.to_string(),
            })?;
    let mut chars = remainder.chars().peekable();
    let mut tokens = Vec::new();

    while tokens.len() < 2 {
        let token = parse_next_token(&mut chars, line)?;
        let Some(token) = token else {
            break;
        };
        if !token.is_empty() {
            tokens.push(token);
        }
    }

    let [first, second] =
        <[String; 2]>::try_from(tokens).map_err(|_| ApplyPatchError::InvalidDiffHeader {
            line: line.to_string(),
        })?;
    Ok((first, second))
}

fn parse_next_token(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    line: &str,
) -> Result<Option<String>, ApplyPatchError> {
    consume_whitespace(chars);
    let next = match chars.peek().copied() {
        Some(next) => next,
        None => return Ok(None),
    };
    if next == '"' {
        chars.next();
        read_quoted_token(chars, line).map(Some)
    } else {
        Ok(Some(read_unquoted_token(chars)))
    }
}

fn consume_whitespace(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) {
        chars.next();
    }
}

fn read_quoted_token(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    line: &str,
) -> Result<String, ApplyPatchError> {
    let mut value = String::new();
    for ch in chars.by_ref() {
        if ch == '"' {
            return Ok(value);
        }
        value.push(ch);
    }
    Err(ApplyPatchError::InvalidDiffHeader {
        line: line.to_string(),
    })
}

fn read_unquoted_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut value = String::new();
    while let Some(ch) = chars.peek().copied() {
        if ch.is_whitespace() {
            break;
        }
        value.push(ch);
        chars.next();
    }
    value
}

fn strip_b_prefix(path: &str) -> String {
    path.strip_prefix("b/")
        .or_else(|| path.strip_prefix("b\\"))
        .unwrap_or(path)
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationMode {
    Unknown,
    Modify,
    Create,
    Delete,
}

impl OperationMode {
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
