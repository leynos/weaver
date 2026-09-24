//! GitHub-style Markdown heading anchors used by the boundary manifest gate.

use std::collections::BTreeSet;

/// Extract GitHub-style heading anchors from a Markdown document.
pub(super) fn heading_anchors(document: &str) -> BTreeSet<String> {
    document
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("## "))
        .map(markdown_anchor)
        .collect()
}

/// Convert a heading into the anchor form used by GitHub Markdown.
pub(super) fn markdown_anchor(heading: &str) -> String {
    let mut anchor = String::new();
    let mut previous_was_dash = false;

    for char in heading.chars().flat_map(char::to_lowercase) {
        if char.is_ascii_alphanumeric() {
            anchor.push(char);
            previous_was_dash = false;
        } else if char.is_whitespace() || char == '-' {
            push_dash(&mut anchor, &mut previous_was_dash);
        }
    }

    anchor.trim_matches('-').to_owned()
}

/// Append a single collapsed dash while building an anchor.
fn push_dash(anchor: &mut String, previous_was_dash: &mut bool) {
    if !*previous_was_dash && !anchor.is_empty() {
        anchor.push('-');
        *previous_was_dash = true;
    }
}
