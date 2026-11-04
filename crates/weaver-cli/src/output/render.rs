//! Human-readable rendering of source locations.

use std::{
    collections::{HashMap, hash_map::Entry},
    fmt,
    fmt::Write as _,
    path::Path,
};

use cap_std::fs::Dir;
use unicode_width::UnicodeWidthChar;

use super::source::SourceLocation;

/// Number of context lines rendered before and after the target line.
const CONTEXT_LINES: u32 = 2;

/// Renders a list of source locations into a human-readable string.
#[must_use]
pub(crate) fn render_locations(locations: &[SourceLocation]) -> String {
    if locations.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    let (order, grouped) = group_locations_by_source(locations);
    for (group_index, key) in order.iter().enumerate() {
        if group_index > 0 {
            output.push('\n');
        }
        if let Some(group) = grouped.get(key) {
            render_group(&mut output, key, group);
        }
    }

    output
}

/// Groups locations by their display source, preserving first-seen order.
fn group_locations_by_source(
    locations: &[SourceLocation],
) -> (Vec<String>, HashMap<String, Vec<&SourceLocation>>) {
    let mut order = Vec::new();
    let mut grouped: HashMap<String, Vec<&SourceLocation>> = HashMap::new();

    for location in locations {
        let key = location.source.display();
        match grouped.entry(key.clone()) {
            Entry::Vacant(entry) => {
                order.push(key);
                entry.insert(vec![location]);
            }
            Entry::Occupied(mut entry) => {
                entry.get_mut().push(location);
            }
        }
    }

    (order, grouped)
}

/// Renders one source group with its surrounding context lines.
fn render_group(output: &mut String, key: &str, group: &[&SourceLocation]) {
    let Some(first) = group.first() else {
        return;
    };
    let source = &first.source;
    write_render_line(output, format_args!("{key}\n"));

    let content_result = source
        .as_path()
        .map(|path| read_source_content(path).map_err(|err| err.to_string()));

    for (index, location) in group.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        render_single_location(output, location, content_result.as_ref());
    }
}

/// Renders one location using the shared source content result.
fn render_single_location(
    output: &mut String,
    location: &SourceLocation,
    content_result: Option<&Result<String, String>>,
) {
    match content_result {
        Some(Ok(content)) => render_location_block(output, location, Some(content)),
        Some(Err(error)) => {
            render_unresolved(output, location, format!("source unavailable: {error}"));
        }
        None => render_location_block(output, location, None),
    }
}

/// Renders a location with source context when the content is available.
fn render_location_block(output: &mut String, location: &SourceLocation, content: Option<&str>) {
    let line = location.position.line;
    let column = location.position.column;

    if let Some(reason) = location.source.reason() {
        render_unresolved(output, location, reason);
        return;
    }

    let Some(source_text) = content else {
        render_unresolved(output, location, String::from("source unavailable"));
        return;
    };

    let Some(line_number) = line else {
        render_unresolved(output, location, String::from("missing line information"));
        return;
    };

    let resolved_column = column.unwrap_or(1);
    render_context(
        output,
        location,
        source_text,
        &LineColumn {
            line: line_number,
            column: resolved_column,
        },
    );
}

/// Renders a note explaining why a location could not be resolved.
fn render_unresolved(output: &mut String, location: &SourceLocation, reason: impl Into<String>) {
    let reason_text = reason.into();
    match (location.position.line, location.position.column) {
        (Some(line), Some(column)) => {
            write_render_line(output, format_args!("  --> {line}:{column}\n"));
        }
        (Some(line), None) => {
            write_render_line(output, format_args!("  --> {line}\n"));
        }
        _ => {
            write_render_line(output, format_args!("  --> (location unavailable)\n"));
        }
    }
    write_render_line(output, format_args!("  note: {reason_text}\n"));
}

/// Renders the context window around `point` within `content`.
fn render_context(
    output: &mut String,
    location: &SourceLocation,
    content: &str,
    point: &LineColumn,
) {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        render_unresolved(output, location, String::from("source is empty"));
        return;
    }

    let total_lines = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    if point.line == 0 || point.line > total_lines {
        render_unresolved(output, location, String::from("line out of range"));
        return;
    }

    let start_line = point.line.saturating_sub(CONTEXT_LINES).max(1);
    let end_line = (point.line + CONTEXT_LINES).min(total_lines);
    let line_width = num_digits(end_line);

    write_render_line(
        output,
        format_args!("  --> {}:{}\n", point.line, point.column),
    );
    write_render_line(output, format_args!("   |\n"));

    for current in start_line..=end_line {
        let line_index = usize::try_from(current - 1).unwrap_or(usize::MAX);
        let Some(text) = lines.get(line_index).copied() else {
            break;
        };
        write_render_line(output, format_args!("{current:>line_width$} | {text}\n"));

        if current == point.line {
            render_caret_line(
                output,
                &CaretContext {
                    line_width,
                    text,
                    column: point.column,
                    label: &location.label,
                },
            );
        }
    }
}

/// Renders the caret line beneath the target source line.
fn render_caret_line(output: &mut String, context: &CaretContext<'_>) {
    let line_len = u32::try_from(context.text.encode_utf16().count()).unwrap_or(u32::MAX);
    let target_units = context.column.saturating_sub(1).min(line_len);
    let caret_pos = caret_display_offset(context.text, target_units);
    let mut caret_line = String::new();
    caret_line.extend(std::iter::repeat_n(' ', caret_pos));
    caret_line.push('^');
    if !context.label.is_empty() {
        caret_line.push(' ');
        caret_line.push_str(context.label);
    }
    write_render_line(
        output,
        format_args!(
            "{0:>line_width$} | {caret_line}\n",
            "",
            line_width = context.line_width
        ),
    );
}

/// Reads `path` through its parent directory capability.
///
/// # Errors
///
/// Returns an [`std::io::Error`] when `path` has no file name, when the parent
/// directory cannot be opened, or when the file is not readable UTF-8.
fn read_source_content(path: &Path) -> std::io::Result<String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing file name")
    })?;
    let directory = Dir::open_ambient_dir(parent, cap_std::ambient_authority())?;
    directory.read_to_string(file_name)
}

/// Appends formatted text to `output`, ignoring formatting failures.
fn write_render_line(output: &mut String, args: fmt::Arguments<'_>) { output.write_fmt(args).ok(); }

/// Computes the display width of the caret offset for `target_units`.
///
/// `target_units` counts UTF-16 code units, matching the columns reported by
/// language servers, while the result counts terminal display cells.
fn caret_display_offset(text: &str, target_units: u32) -> usize {
    let mut units_consumed = 0u32;
    let mut width = 0usize;

    for ch in text.chars() {
        let next_units = units_consumed + u32::try_from(ch.len_utf16()).unwrap_or(u32::MAX);
        if next_units > target_units {
            break;
        }
        units_consumed = next_units;
        width += UnicodeWidthChar::width(ch).unwrap_or(0);
    }

    width
}

/// Counts the decimal digits needed to print `value`.
fn num_digits(value: u32) -> usize { value.to_string().len() }

/// A 1-indexed line and column pair.
struct LineColumn {
    /// Line number as reported by the language server.
    line: u32,
    /// Column number as reported by the language server.
    column: u32,
}

/// Inputs for rendering the caret line beneath a source line.
struct CaretContext<'a> {
    /// Width reserved for the line-number gutter.
    line_width: usize,
    /// Source line the caret is rendered under.
    text: &'a str,
    /// 1-indexed column the caret marks.
    column: u32,
    /// Label appended after the caret, empty when no label is wanted.
    label: &'a str,
}

#[cfg(test)]
mod tests {
    //! Unit tests for output rendering and formatting.

    use super::*;
    use crate::output::source::{SourceLocation, SourcePosition, SourceReference};

    #[test]
    fn renders_basic_context() {
        let location = SourceLocation {
            source: SourceReference::Path("/tmp/example.rs".into()),
            position: SourcePosition::new(Some(2), Some(5)),
            label: String::from("definition"),
        };
        let content = "fn main() {\n    let value = 1;\n    value\n}";
        let output = {
            let mut buffer = String::new();
            render_context(
                &mut buffer,
                &location,
                content,
                &LineColumn { line: 2, column: 5 },
            );
            buffer
        };
        assert!(output.contains("2 |"));
        assert!(output.contains("^ definition"));
    }

    #[test]
    fn renders_unresolved_location() {
        let location = SourceLocation::unresolved(
            String::from("/missing/file.rs"),
            SourcePosition::new(Some(3), Some(1)),
            String::from("diagnostic"),
            String::from("file not found"),
        );
        let output = render_locations(&[location]);
        assert!(output.contains("note: file not found"));
    }
}
