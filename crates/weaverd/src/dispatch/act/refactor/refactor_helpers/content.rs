//! Deterministic file contents and diff payloads for refactor tests.

use std::path::Path;

fn format_diff(path: &Path, git_header: &str) -> String {
    let original = original_content_for(path);
    let updated = updated_content_for(path);
    format!("{git_header}\n<<<<<<< SEARCH\n{original}=======\n{updated}>>>>>>> REPLACE\n",)
}

pub(crate) enum FileKind {
    Python,
    Rust,
    Other,
}

pub(crate) fn classify_file(path: &Path) -> FileKind {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("py") => FileKind::Python,
        Some("rs") => FileKind::Rust,
        _ => FileKind::Other,
    }
}

struct FileContents {
    original: &'static str,
    updated: &'static str,
}

fn content_table(kind: FileKind) -> FileContents {
    match kind {
        FileKind::Python => FileContents {
            original: "old_name = 1\nprint(old_name)\n",
            updated: "woven = 1\nprint(woven)\n",
        },
        FileKind::Rust => FileContents {
            original: concat!(
                "fn main() {\n",
                "    let old_name = 1;\n",
                "    println!(\"{}\", old_name);\n",
                "}\n",
            ),
            updated: concat!(
                "fn main() {\n",
                "    let woven = 1;\n",
                "    println!(\"{}\", woven);\n",
                "}\n",
            ),
        },
        FileKind::Other => FileContents {
            original: "hello world\n",
            updated: "hello woven\n",
        },
    }
}

pub(crate) fn original_content_for(path: &Path) -> &'static str {
    content_table(classify_file(path)).original
}

pub(crate) fn updated_content_for(path: &Path) -> &'static str {
    content_table(classify_file(path)).updated
}

pub(crate) fn routed_patch_path(path: &Path) -> &Path {
    match classify_file(path) {
        FileKind::Python | FileKind::Rust => Path::new("notes.txt"),
        FileKind::Other => path,
    }
}

fn routed_format_diff(path: &Path, make_header: impl Fn(&str) -> String) -> String {
    let patch_path = routed_patch_path(path);
    format_diff(path, &make_header(&patch_path.to_string_lossy()))
}

pub(crate) fn routed_diff_for(path: &Path) -> String {
    routed_format_diff(path, |p| format!("diff --git a/{p} b/{p}"))
}

pub(crate) fn routed_malformed_diff_for(path: &Path) -> String {
    routed_format_diff(path, |p| format!("diff --git a/{p}"))
}

const _: fn(&Path) -> FileKind = classify_file;
const _: fn(&Path) -> &'static str = original_content_for;
const _: fn(&Path) -> &'static str = updated_content_for;
const _: fn(&Path) -> &Path = routed_patch_path;
const _: fn(&Path) -> String = routed_diff_for;
const _: fn(&Path) -> String = routed_malformed_diff_for;
