//! Patch parser for the apply-patch command.

use crate::dispatch::act::apply_patch::errors::ApplyPatchError;
use crate::dispatch::act::apply_patch::types::{
    DiffHeaderLine, FilePath, PatchOperation, PatchText, SearchPattern, SearchReplaceBlock,
};

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
    for line in patch.split_inclusive('\n') {
        let trimmed = trim_line(line);
        if trimmed.starts_with("diff --git ") {
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
        let state = ParserState::new(&mut mode, &mut search_replace);

        if process_search_replace_marker(&line_info, state, chunk, &path)? {
            offset = line_info.end;
            continue;
        }

        mode = detect_operation_mode(mode, trimmed);
        process_secondary_line(line_type, mode, &mut create_capture, line)?;

        offset = line_end;
    }

    search_replace.validate_complete(&path)?;
    construct_operation(mode, path, search_replace, create_capture)
}

/// Processes SEARCH/REPLACE marker lines and updates parser state.
// Required to keep parse_operation shallow while carrying all marker context.
fn process_search_replace_marker(
    line_info: &LineInfo,
    state: ParserState<'_>,
    chunk: &str,
    path: &FilePath,
) -> Result<bool, ApplyPatchError> {
    handle_search_replace_marker(line_info, state, chunk, path)
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

/// Detects mode transitions based on diff metadata lines.
fn detect_operation_mode(current: OperationMode, trimmed: &str) -> OperationMode {
    detect_mode_transition(trimmed, current)
}

/// Applies mode transitions for create/delete markers.
fn detect_mode_transition(trimmed: &str, current: OperationMode) -> OperationMode {
    let mut mode = current;
    if trimmed.starts_with("new file mode ") {
        mode = mode.promote(OperationMode::Create);
    }
    if trimmed.starts_with("deleted file mode ") {
        mode = mode.promote(OperationMode::Delete);
    }
    mode
}

/// Processes hunk headers, diff headers, and create content lines.
// Required to keep line context explicit while delegating to small helpers.
fn process_secondary_line(
    line_type: LineType,
    mode: OperationMode,
    create_capture: &mut CreateContentCapture,
    line: &str,
) -> Result<(), ApplyPatchError> {
    match line_type {
        LineType::HunkHeader | LineType::DiffHeader | LineType::CreateContent => {
            let trimmed = trim_line(line);
            validate_line_type(line_type, trimmed)?;
            handle_mode_specific_capture(mode, line_type, line, create_capture);
            Ok(())
        }
        _ => Ok(()),
    }
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
    let path = strip_prefix(b_path).into_string();
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

struct SearchReplaceParser {
    blocks: Vec<SearchReplaceBlock>,
    search_start: Option<usize>,
    replace_start: Option<usize>,
}

impl SearchReplaceParser {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            search_start: None,
            replace_start: None,
        }
    }

    fn handle_search_marker(&mut self, offset: usize) {
        self.search_start = Some(offset);
        self.replace_start = None;
    }

    fn handle_separator(&mut self, chunk: &str, line_start: usize) {
        if let Some(start) = self.search_start {
            let search = &chunk[start..line_start];
            self.replace_start = Some(line_end_from_chunk(chunk, line_start));
            self.blocks.push(SearchReplaceBlock {
                search: SearchPattern::new(search),
                replace: String::new(),
            });
        }
    }

    fn handle_replace_marker(
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

    fn validate_complete(&self, path: &FilePath) -> Result<(), ApplyPatchError> {
        if self.search_start.is_some() && self.replace_start.is_none() {
            return Err(ApplyPatchError::UnclosedSearchBlock { path: path.clone() });
        }
        if self.replace_start.is_some() {
            return Err(ApplyPatchError::UnclosedReplaceBlock { path: path.clone() });
        }
        Ok(())
    }

    fn into_blocks(self) -> Vec<SearchReplaceBlock> {
        self.blocks
    }
}

struct CreateContentCapture {
    content: String,
    in_hunk: bool,
    capture_hunk: bool,
    saw_hunk: bool,
}

impl CreateContentCapture {
    fn new() -> Self {
        Self {
            content: String::new(),
            in_hunk: false,
            capture_hunk: false,
            saw_hunk: false,
        }
    }

    fn handle_hunk_header(&mut self) {
        self.saw_hunk = true;
        if !self.in_hunk {
            self.in_hunk = true;
            self.capture_hunk = true;
        } else {
            self.capture_hunk = false;
        }
    }

    fn capture_line(&mut self, line: &str) {
        if !self.capture_hunk {
            return;
        }
        let (content, ending) = split_line_content(line);
        self.content.push_str(content);
        self.content.push_str(ending);
    }

    fn validate_and_finish(self, path: &FilePath) -> Result<String, ApplyPatchError> {
        if !self.saw_hunk {
            return Err(ApplyPatchError::MissingHunk { path: path.clone() });
        }
        Ok(self.content)
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

fn line_end_from_chunk(chunk: &str, line_start: usize) -> usize {
    chunk[line_start..]
        .find('\n')
        .map(|offset| line_start + offset + 1)
        .unwrap_or(chunk.len())
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
        let token = parse_next_token(&mut chars);
        let Some(token) = token else { break };
        if !token.is_empty() {
            tokens.push(token);
        }
    }

    if tokens.len() != 2 {
        return Err(ApplyPatchError::InvalidDiffHeader {
            line: line.to_string(),
        });
    }

    Ok((tokens[0].clone(), tokens[1].clone()))
}

fn parse_next_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
    consume_whitespace(chars);
    let next = chars.peek().copied()?;
    if next == '"' {
        chars.next();
        Some(read_quoted_token(chars))
    } else {
        Some(read_unquoted_token(chars))
    }
}

fn consume_whitespace(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while matches!(chars.peek(), Some(ch) if ch.is_whitespace()) {
        chars.next();
    }
}

fn read_quoted_token(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    chars
        .by_ref()
        .take_while(|ch| *ch != '"')
        .collect::<String>()
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

fn strip_prefix(path: String) -> FilePath {
    FilePath::new(
        path.strip_prefix("b/")
            .or_else(|| path.strip_prefix("b\\"))
            .unwrap_or(path.as_str())
            .to_string(),
    )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationMode {
    Unknown,
    Modify,
    Create,
    Delete,
}

impl OperationMode {
    fn promote(self, next: Self) -> Self {
        match (self, next) {
            (Self::Unknown, other) => other,
            (current, other) if current == other => current,
            (current, _) => current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::act::apply_patch::types::PatchText;

    #[test]
    fn parses_modify_operation() {
        let patch = concat!(
            "diff --git a/src/lib.rs b/src/lib.rs\n",
            "<<<<<<< SEARCH\n",
            "fn main() {}\n",
            "=======\n",
            "fn main() { println!(\"hi\"); }\n",
            ">>>>>>> REPLACE\n",
        );
        let ops = parse_patch(&PatchText::from(patch)).expect("parse patch");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            PatchOperation::Modify { path, blocks } => {
                assert_eq!(path.as_str(), "src/lib.rs");
                assert_eq!(blocks.len(), 1);
            }
            other => panic!("unexpected operation: {other:?}"),
        }
    }

    #[test]
    fn parses_create_operation() {
        let patch = concat!(
            "diff --git a/src/new.rs b/src/new.rs\n",
            "new file mode 100644\n",
            "--- /dev/null\n",
            "+++ b/src/new.rs\n",
            "@@ -0,0 +1,2 @@\n",
            "+fn hello() {}\n",
            "+fn world() {}\n",
        );
        let ops = parse_patch(&PatchText::from(patch)).expect("parse patch");
        match &ops[0] {
            PatchOperation::Create { content, .. } => {
                assert!(content.contains("fn hello()"));
                assert!(content.contains("fn world()"));
            }
            other => panic!("unexpected operation: {other:?}"),
        }
    }

    #[test]
    fn create_operation_keeps_plus_prefixed_content() {
        let patch = concat!(
            "diff --git a/src/new.rs b/src/new.rs\n",
            "new file mode 100644\n",
            "--- /dev/null\n",
            "+++ b/src/new.rs\n",
            "@@ -0,0 +1,1 @@\n",
            "++++hello\n",
        );
        let ops = parse_patch(&PatchText::from(patch)).expect("parse patch");
        match &ops[0] {
            PatchOperation::Create { content, .. } => {
                assert!(content.contains("+++hello"));
            }
            other => panic!("unexpected operation: {other:?}"),
        }
    }

    #[test]
    fn parses_delete_operation() {
        let patch = concat!(
            "diff --git a/src/remove.rs b/src/remove.rs\n",
            "deleted file mode 100644\n",
        );
        let ops = parse_patch(&PatchText::from(patch)).expect("parse patch");
        match &ops[0] {
            PatchOperation::Delete { path } => {
                assert_eq!(path.as_str(), "src/remove.rs");
            }
            other => panic!("unexpected operation: {other:?}"),
        }
    }

    #[test]
    fn rejects_missing_diff_header() {
        let error = parse_patch(&PatchText::from("not a patch")).expect_err("should fail");
        assert!(matches!(error, ApplyPatchError::MissingDiffHeader));
    }

    #[test]
    fn rejects_unclosed_search_block() {
        let patch = concat!(
            "diff --git a/src/lib.rs b/src/lib.rs\n",
            "<<<<<<< SEARCH\n",
            "fn main() {}\n",
        );
        let error = parse_patch(&PatchText::from(patch)).expect_err("should fail");
        assert!(matches!(error, ApplyPatchError::UnclosedSearchBlock { .. }));
    }
}
