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

fn render_group(output: &mut String, key: &str, group: &[&SourceLocation]) {
    if group.is_empty() {
        return;
    }

    let source = &group[0].source;
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

fn render_location_block(output: &mut String, location: &SourceLocation, content: Option<&str>) {
    let line = location.position.line;
    let column = location.position.column;

    if let Some(reason) = location.source.reason() {
        render_unresolved(output, location, reason);
        return;
    }

    let Some(content) = content else {
        render_unresolved(output, location, String::from("source unavailable"));
        return;
    };

    let Some(line) = line else {
        render_unresolved(output, location, String::from("missing line information"));
        return;
    };

    let column = column.unwrap_or(1);
    render_context(output, location, content, LineColumn { line, column });
}

fn render_unresolved(output: &mut String, location: &SourceLocation, reason: impl Into<String>) {
    let reason = reason.into();
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
    write_render_line(output, format_args!("  note: {reason}\n"));
}

fn render_context(
    output: &mut String,
    location: &SourceLocation,
    content: &str,
    point: LineColumn,
) {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        render_unresolved(output, location, String::from("source is empty"));
        return;
    }

    let total_lines = lines.len() as u32;
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
        let text = lines[(current - 1) as usize];
        write_render_line(output, format_args!("{current:>line_width$} | {text}\n"));

        if current == point.line {
            render_caret_line(
                output,
                CaretContext {
                    line_width,
                    text,
                    column: point.column,
                    label: &location.label,
                },
            );
        }
    }
}

fn render_caret_line(output: &mut String, context: CaretContext<'_>) {
    let line_len = context.text.encode_utf16().count() as u32;
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

fn write_render_line(output: &mut String, args: fmt::Arguments<'_>) { output.write_fmt(args).ok(); }

fn caret_display_offset(text: &str, target_units: u32) -> usize {
    let mut units_consumed = 0u32;
    let mut width = 0usize;

    for ch in text.chars() {
        let next_units = units_consumed + ch.len_utf16() as u32;
        if next_units > target_units {
            break;
        }
        units_consumed = next_units;
        width += UnicodeWidthChar::width(ch).unwrap_or(0);
    }

    width
}

fn num_digits(value: u32) -> usize { value.to_string().len() }

struct LineColumn {
    line: u32,
    column: u32,
}

struct CaretContext<'a> {
    line_width: usize,
    text: &'a str,
    column: u32,
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
                LineColumn { line: 2, column: 5 },
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
