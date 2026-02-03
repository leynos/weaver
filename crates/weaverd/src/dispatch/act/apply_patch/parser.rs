//! Patch parser for the apply-patch command.

use crate::dispatch::act::apply_patch::errors::ApplyPatchError;
use crate::dispatch::act::apply_patch::types::{PatchOperation, SearchPattern, SearchReplaceBlock};

pub(crate) fn parse_patch(patch: &str) -> Result<Vec<PatchOperation>, ApplyPatchError> {
    if patch.trim().is_empty() {
        return Err(ApplyPatchError::EmptyPatch);
    }
    if patch.as_bytes().contains(&0) {
        return Err(ApplyPatchError::BinaryPatch);
    }

    let chunks = split_operations(patch)?;
    let mut operations = Vec::new();
    for chunk in chunks {
        operations.push(parse_operation(chunk)?);
    }

    Ok(operations)
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
    let mut offset = 0;
    let mut header_seen = false;
    let mut path = String::new();
    let mut mode = OperationMode::Unknown;
    let mut blocks: Vec<SearchReplaceBlock> = Vec::new();
    let mut create_content = String::new();
    let mut in_hunk = false;
    let mut capture_hunk = false;
    let mut saw_hunk = false;
    let mut search_start: Option<usize> = None;
    let mut replace_start: Option<usize> = None;

    for line in chunk.split_inclusive('\n') {
        let line_start = offset;
        let line_end = offset + line.len();
        let trimmed = trim_line(line);

        if !header_seen {
            if !trimmed.starts_with("diff --git ") {
                return Err(ApplyPatchError::InvalidDiffHeader {
                    line: trimmed.to_string(),
                });
            }
            let (_, b_path) = parse_diff_paths(trimmed)?;
            path = strip_prefix(b_path);
            header_seen = true;
            offset = line_end;
            continue;
        }

        if trimmed == "<<<<<<< SEARCH" {
            mode = mode.promote(OperationMode::Modify);
            search_start = Some(line_end);
            replace_start = None;
            offset = line_end;
            continue;
        }

        if trimmed == "=======" {
            if let Some(start) = search_start {
                let search = &chunk[start..line_start];
                replace_start = Some(line_end);
                blocks.push(SearchReplaceBlock {
                    search: SearchPattern::new(search),
                    replace: String::new(),
                });
            }
            offset = line_end;
            continue;
        }

        if trimmed == ">>>>>>> REPLACE" {
            let Some(start) = replace_start else {
                return Err(ApplyPatchError::UnclosedSearchBlock { path });
            };
            let replace = &chunk[start..line_start];
            if let Some(last) = blocks.last_mut() {
                last.replace = replace.to_string();
            }
            search_start = None;
            replace_start = None;
            offset = line_end;
            continue;
        }

        if trimmed.starts_with("new file mode ") {
            mode = mode.promote(OperationMode::Create);
        }
        if trimmed.starts_with("deleted file mode ") {
            mode = mode.promote(OperationMode::Delete);
        }

        if trimmed.starts_with("@@") {
            saw_hunk = true;
            if mode == OperationMode::Create && !in_hunk {
                in_hunk = true;
                capture_hunk = true;
            } else if mode == OperationMode::Create {
                capture_hunk = false;
            }
        } else if trimmed.starts_with("diff --git ") {
            return Err(ApplyPatchError::InvalidDiffHeader {
                line: trimmed.to_string(),
            });
        }

        if mode == OperationMode::Create && capture_hunk && trimmed.starts_with('+') {
            let (content, ending) = split_line_content(line);
            create_content.push_str(content);
            create_content.push_str(ending);
        }

        offset = line_end;
    }

    if search_start.is_some() && replace_start.is_none() {
        return Err(ApplyPatchError::UnclosedSearchBlock { path });
    }
    if replace_start.is_some() {
        return Err(ApplyPatchError::UnclosedReplaceBlock { path });
    }

    match mode {
        OperationMode::Modify => {
            if blocks.is_empty() {
                return Err(ApplyPatchError::MissingSearchReplace { path });
            }
            Ok(PatchOperation::Modify { path, blocks })
        }
        OperationMode::Create => {
            if !saw_hunk {
                return Err(ApplyPatchError::MissingHunk { path });
            }
            Ok(PatchOperation::Create {
                path,
                content: create_content,
            })
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

fn strip_prefix(path: String) -> String {
    path.strip_prefix("b/")
        .or_else(|| path.strip_prefix("b\\"))
        .unwrap_or(path.as_str())
        .to_string()
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
        let ops = parse_patch(patch).expect("parse patch");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            PatchOperation::Modify { path, blocks } => {
                assert_eq!(path, "src/lib.rs");
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
        let ops = parse_patch(patch).expect("parse patch");
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
        let ops = parse_patch(patch).expect("parse patch");
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
        let ops = parse_patch(patch).expect("parse patch");
        match &ops[0] {
            PatchOperation::Delete { path } => {
                assert_eq!(path, "src/remove.rs");
            }
            other => panic!("unexpected operation: {other:?}"),
        }
    }

    #[test]
    fn rejects_missing_diff_header() {
        let error = parse_patch("not a patch").expect_err("should fail");
        assert!(matches!(error, ApplyPatchError::MissingDiffHeader));
    }

    #[test]
    fn rejects_unclosed_search_block() {
        let patch = concat!(
            "diff --git a/src/lib.rs b/src/lib.rs\n",
            "<<<<<<< SEARCH\n",
            "fn main() {}\n",
        );
        let error = parse_patch(patch).expect_err("should fail");
        assert!(matches!(error, ApplyPatchError::UnclosedSearchBlock { .. }));
    }
}
