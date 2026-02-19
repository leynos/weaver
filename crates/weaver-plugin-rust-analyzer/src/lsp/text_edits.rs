//! Workspace edit and position conversion helpers.

use std::path::Path;

use lsp_types::{
    AnnotatedTextEdit, DocumentChangeOperation, DocumentChanges, OneOf, Position, TextEdit, Uri,
    WorkspaceEdit,
};

use crate::{ByteOffset, RustAnalyzerAdapterError};

/// LSP position encoding used for character offsets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PositionEncoding {
    /// UTF-8 code units.
    Utf8,
    /// UTF-16 code units.
    Utf16,
}

/// Parses a rename result payload to a workspace edit.
pub(super) fn parse_workspace_edit(
    result: serde_json::Value,
) -> Result<WorkspaceEdit, RustAnalyzerAdapterError> {
    if result.is_null() {
        return Err(RustAnalyzerAdapterError::EngineFailed {
            message: String::from("rust-analyzer returned no workspace edit for rename"),
        });
    }

    serde_json::from_value(result).map_err(|source| RustAnalyzerAdapterError::InvalidOutput {
        message: format!("failed to deserialize workspace edit: {source}"),
    })
}

/// Ensures an LSP response payload is a JSON object.
pub(super) fn ensure_response_is_object(
    response: &serde_json::Value,
    method: &str,
) -> Result<(), RustAnalyzerAdapterError> {
    if response.is_object() {
        return Ok(());
    }

    Err(RustAnalyzerAdapterError::InvalidOutput {
        message: format!("{method} response payload was not a JSON object"),
    })
}

/// Converts a byte offset into an LSP UTF-16 position.
pub(super) fn byte_offset_to_lsp_position(
    content: &str,
    offset: ByteOffset,
    encoding: PositionEncoding,
) -> Result<Position, RustAnalyzerAdapterError> {
    let byte_offset = offset.as_usize();
    if byte_offset > content.len() {
        return Err(RustAnalyzerAdapterError::InvalidOutput {
            message: format!(
                "offset {byte_offset} is beyond file length {}",
                content.len()
            ),
        });
    }
    if !content.is_char_boundary(byte_offset) {
        return Err(RustAnalyzerAdapterError::InvalidOutput {
            message: format!("offset {byte_offset} is not at a UTF-8 character boundary"),
        });
    }

    let prefix = slice_checked(content, ..byte_offset, "prefix")?;
    let line =
        u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count()).map_err(|source| {
            RustAnalyzerAdapterError::InvalidOutput {
                message: format!("line count exceeds u32 range: {source}"),
            }
        })?;

    let line_start = prefix
        .rfind('\n')
        .map_or(0, |index| index + '\n'.len_utf8());
    let line_prefix = slice_checked(content, line_start..byte_offset, "line prefix")?;
    let character_units = match encoding {
        PositionEncoding::Utf8 => line_prefix.len(),
        PositionEncoding::Utf16 => line_prefix.encode_utf16().count(),
    };
    let character = u32::try_from(character_units).map_err(|source| {
        RustAnalyzerAdapterError::InvalidOutput {
            message: format!("character offset exceeds u32 range: {source}"),
        }
    })?;

    Ok(Position { line, character })
}

/// Applies a workspace edit to the original content and returns the updated text.
pub(super) fn apply_workspace_edit(
    original: &str,
    workspace_edit: WorkspaceEdit,
    file_uri: &Uri,
    encoding: PositionEncoding,
) -> Result<String, RustAnalyzerAdapterError> {
    let mut edits = collect_text_edits(workspace_edit, file_uri)?;
    if edits.is_empty() {
        return Ok(String::from(original));
    }

    let mut ranges = edits
        .drain(..)
        .map(|edit| {
            let start = lsp_position_to_byte_offset(original, edit.range.start, encoding)?;
            let end = lsp_position_to_byte_offset(original, edit.range.end, encoding)?;
            if end < start {
                return Err(RustAnalyzerAdapterError::InvalidOutput {
                    message: format!("edit range end precedes start (start={start}, end={end})"),
                });
            }
            Ok((start, end, edit.new_text))
        })
        .collect::<Result<Vec<(usize, usize, String)>, RustAnalyzerAdapterError>>()?;

    ranges.sort_by(|left, right| right.0.cmp(&left.0));

    let mut updated = String::from(original);
    for (start, end, replacement) in ranges {
        if end > updated.len() || start > end {
            return Err(RustAnalyzerAdapterError::InvalidOutput {
                message: format!("edit range [{start}, {end}) is out of bounds"),
            });
        }
        if !updated.is_char_boundary(start) || !updated.is_char_boundary(end) {
            return Err(RustAnalyzerAdapterError::InvalidOutput {
                message: format!("edit range [{start}, {end}) is not UTF-8 aligned"),
            });
        }

        updated.replace_range(start..end, &replacement);
    }

    Ok(updated)
}

fn collect_text_edits(
    workspace_edit: WorkspaceEdit,
    file_uri: &Uri,
) -> Result<Vec<TextEdit>, RustAnalyzerAdapterError> {
    let mut edits = Vec::new();

    if let Some(changes) = workspace_edit.changes
        && let Some(file_edits) = changes.get(file_uri)
    {
        edits.extend(file_edits.clone());
    }

    if let Some(document_changes) = workspace_edit.document_changes {
        collect_document_changes(&mut edits, document_changes, file_uri)?;
    }

    Ok(edits)
}

fn collect_document_changes(
    target: &mut Vec<TextEdit>,
    document_changes: DocumentChanges,
    file_uri: &Uri,
) -> Result<(), RustAnalyzerAdapterError> {
    match document_changes {
        DocumentChanges::Edits(text_document_edits) => {
            for document_edit in text_document_edits {
                append_document_edits(
                    target,
                    &document_edit.text_document.uri,
                    document_edit.edits,
                    file_uri,
                );
            }
            Ok(())
        }
        DocumentChanges::Operations(operations) => {
            for operation in operations {
                collect_operation(target, operation, file_uri)?;
            }
            Ok(())
        }
    }
}

fn collect_operation(
    target: &mut Vec<TextEdit>,
    operation: DocumentChangeOperation,
    file_uri: &Uri,
) -> Result<(), RustAnalyzerAdapterError> {
    match operation {
        DocumentChangeOperation::Edit(document_edit) => {
            append_document_edits(
                target,
                &document_edit.text_document.uri,
                document_edit.edits,
                file_uri,
            );
            Ok(())
        }
        DocumentChangeOperation::Op(resource_operation) => {
            Err(RustAnalyzerAdapterError::InvalidOutput {
                message: format!(
                    "workspace edit includes unsupported resource operation: {resource_operation:?}"
                ),
            })
        }
    }
}

fn append_document_edits(
    target: &mut Vec<TextEdit>,
    uri: &Uri,
    edits: Vec<OneOf<TextEdit, AnnotatedTextEdit>>,
    requested_uri: &Uri,
) {
    if uri != requested_uri {
        return;
    }

    for edit in edits {
        match edit {
            OneOf::Left(text_edit) => target.push(text_edit),
            OneOf::Right(annotated_text_edit) => target.push(annotated_text_edit.text_edit),
        }
    }
}

fn lsp_position_to_byte_offset(
    content: &str,
    position: Position,
    encoding: PositionEncoding,
) -> Result<usize, RustAnalyzerAdapterError> {
    let line_start = find_line_start_offset(content, position.line)?;
    let from_line_start = slice_checked(content, line_start.., "line start")?;
    let line_end = from_line_start
        .find('\n')
        .map_or(content.len(), |relative| line_start + relative);
    let line_content = slice_checked(content, line_start..line_end, "line content")?;

    match encoding {
        PositionEncoding::Utf8 => {
            let character_offset = usize::try_from(position.character).map_err(|source| {
                RustAnalyzerAdapterError::InvalidOutput {
                    message: format!("UTF-8 character offset conversion failed: {source}"),
                }
            })?;
            let byte_offset = line_start + character_offset;
            if byte_offset > line_end {
                return Err(RustAnalyzerAdapterError::InvalidOutput {
                    message: format!(
                        "position {position:?} exceeds line UTF-8 width {}",
                        line_content.len()
                    ),
                });
            }
            if !content.is_char_boundary(byte_offset) {
                return Err(RustAnalyzerAdapterError::InvalidOutput {
                    message: format!("position {position:?} splits a UTF-8 code point"),
                });
            }
            Ok(byte_offset)
        }
        PositionEncoding::Utf16 => {
            let mut utf16_units = 0_u32;
            for (index, character) in line_content.char_indices() {
                if utf16_units == position.character {
                    return Ok(line_start + index);
                }

                let char_width = u32::try_from(character.len_utf16()).map_err(|source| {
                    RustAnalyzerAdapterError::InvalidOutput {
                        message: format!("character width conversion failed: {source}"),
                    }
                })?;
                utf16_units += char_width;

                if utf16_units > position.character {
                    return Err(RustAnalyzerAdapterError::InvalidOutput {
                        message: format!(
                            "position {position:?} splits a UTF-16 code unit sequence"
                        ),
                    });
                }
            }

            if utf16_units == position.character {
                return Ok(line_end);
            }

            Err(RustAnalyzerAdapterError::InvalidOutput {
                message: format!("position {position:?} exceeds line UTF-16 width {utf16_units}"),
            })
        }
    }
}

fn find_line_start_offset(
    content: &str,
    target_line: u32,
) -> Result<usize, RustAnalyzerAdapterError> {
    if target_line == 0 {
        return Ok(0);
    }

    let mut current_line = 0_u32;
    for (index, character) in content.char_indices() {
        if character == '\n' {
            current_line += 1;
            if current_line == target_line {
                return Ok(index + '\n'.len_utf8());
            }
        }
    }

    Err(RustAnalyzerAdapterError::InvalidOutput {
        message: format!("line {target_line} is beyond the end of the document"),
    })
}

/// Writes a minimal `Cargo.toml` so rust-analyzer can open the workspace.
pub(super) fn write_stub_cargo_toml(workspace_root: &Path) -> Result<(), RustAnalyzerAdapterError> {
    let cargo_toml = workspace_root.join("Cargo.toml");
    let content = concat!(
        "[package]\n",
        "name = \"weaver-rust-analyzer-workspace\"\n",
        "version = \"0.1.0\"\n",
        "edition = \"2024\"\n",
    );

    std::fs::write(&cargo_toml, content).map_err(|source| {
        RustAnalyzerAdapterError::WorkspaceWrite {
            path: cargo_toml,
            source,
        }
    })
}

/// Converts an absolute path to an `lsp_types::Uri` using `file://` encoding.
pub(super) fn path_to_file_uri(path: &Path) -> Result<Uri, RustAnalyzerAdapterError> {
    let file_url =
        url::Url::from_file_path(path).map_err(|()| RustAnalyzerAdapterError::InvalidPath {
            message: format!("failed to convert '{}' to file:// URI", path.display()),
        })?;

    file_url
        .as_str()
        .parse()
        .map_err(|source| RustAnalyzerAdapterError::InvalidOutput {
            message: format!("failed to parse file URI '{}': {source}", file_url.as_str()),
        })
}

fn slice_checked<'a, R>(
    content: &'a str,
    range: R,
    slice_name: &str,
) -> Result<&'a str, RustAnalyzerAdapterError>
where
    R: std::slice::SliceIndex<str, Output = str> + std::fmt::Debug,
{
    let range_debug = format!("{range:?}");
    content
        .get(range)
        .ok_or_else(|| RustAnalyzerAdapterError::InvalidOutput {
            message: format!("invalid UTF-8 slice for {slice_name}: {range_debug}"),
        })
}
