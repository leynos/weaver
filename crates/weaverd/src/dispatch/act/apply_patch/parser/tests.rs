//! Tests for apply-patch parsing.

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
    assert_eq!(ops.len(), 1);
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
    assert_eq!(ops.len(), 1);
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
    assert_eq!(ops.len(), 1);
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
