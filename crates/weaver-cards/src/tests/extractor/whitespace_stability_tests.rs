//! Whitespace-stability tests for attachments and comments.

use std::path::Path;

use super::common::{ExtractRequest, extract};
use crate::DetailLevel;

#[test]
fn rust_comment_bundling_is_stable_under_whitespace_edits() {
    let baseline = extract(ExtractRequest {
        path: Path::new("fixture.rs"),
        source: "/// Greets callers.\n/// Returns a count.\nfn greet(name: &str) -> usize {\n    \
                 name.len()\n}\n",
        line: 3,
        column: 4,
        detail: DetailLevel::Structure,
    });
    let edited = extract(ExtractRequest {
        path: Path::new("fixture.rs"),
        source:
            "/// Greets callers.   \n/// Returns a count.\nfn greet(name: &str) -> usize {\n    \
             name.len()\n}\n",
        line: 3,
        column: 4,
        detail: DetailLevel::Structure,
    });

    assert_eq!(baseline.attachments, edited.attachments);
    assert_eq!(baseline.doc, edited.doc);
}

#[test]
fn decorator_bundling_is_stable_under_whitespace_edits() {
    let baseline = extract(ExtractRequest {
        path: Path::new("fixture.ts"),
        source: "@sealed\nclass Widget {\n  render(): void {}\n}\n",
        line: 2,
        column: 7,
        detail: DetailLevel::Structure,
    });
    let edited = extract(ExtractRequest {
        path: Path::new("fixture.ts"),
        source: "   @sealed   \nclass Widget {\n  render(): void {}\n}\n",
        line: 2,
        column: 7,
        detail: DetailLevel::Structure,
    });

    assert_eq!(baseline.attachments, edited.attachments);
}
