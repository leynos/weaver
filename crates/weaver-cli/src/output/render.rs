//! Human-readable rendering of source locations.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;

use super::source::SourceLocation;

const CONTEXT_LINES: u32 = 2;

/// Renders a list of source locations into a human-readable string.
#[must_use]
pub(crate) fn render_locations(locations: &[SourceLocation]) -> String {
    if locations.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<&SourceLocation>> = HashMap::new();

    for location in locations {
        let key = location.source.display();
        if !grouped.contains_key(&key) {
            order.push(key.clone());
        }
        grouped.entry(key).or_default().push(location);
    }

    for (group_index, key) in order.iter().enumerate() {
        if group_index > 0 {
            output.push('\n');
        }
        let Some(group) = grouped.get(key) else {
            continue;
        };
        let source = &group[0].source;
        writeln!(output, "{key}").expect("write header");

        let content_result = source
            .as_path()
            .map(|path| fs::read_to_string(path).map_err(|err| err.to_string()));

        for (index, location) in group.iter().enumerate() {
            if index > 0 {
                output.push('\n');
            }
            match content_result.as_ref() {
                Some(Ok(content)) => render_location_block(&mut output, location, Some(content)),
                Some(Err(error)) => {
                    render_unresolved(
                        &mut output,
                        location,
                        format!("source unavailable: {error}"),
                    );
                }
                None => render_location_block(&mut output, location, None),
            }
        }
    }

    output
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
            writeln!(output, "  --> {line}:{column}").expect("write location");
        }
        (Some(line), None) => {
            writeln!(output, "  --> {line}").expect("write location");
        }
        _ => {
            writeln!(output, "  --> (location unavailable)").expect("write location");
        }
    }
    writeln!(output, "  note: {reason}").expect("write reason");
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

    writeln!(output, "  --> {}:{}", point.line, point.column).expect("write location");
    writeln!(output, "   |").expect("write gutter");

    for current in start_line..=end_line {
        let text = lines[(current - 1) as usize];
        writeln!(output, "{current:>line_width$} | {text}").expect("write line");

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
    let line_len = context.text.chars().count();
    let column_index = context.column.saturating_sub(1) as usize;
    let caret_pos = column_index.min(line_len);
    let mut caret_line = String::new();
    caret_line.extend(std::iter::repeat_n(' ', caret_pos));
    caret_line.push('^');
    if !context.label.is_empty() {
        caret_line.push(' ');
        caret_line.push_str(context.label);
    }
    writeln!(
        output,
        "{0:>line_width$} | {caret_line}",
        "",
        line_width = context.line_width
    )
    .expect("write caret");
}

fn num_digits(value: u32) -> usize {
    value.to_string().len()
}

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
