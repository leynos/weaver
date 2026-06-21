//! Integration tests for the `OrthoConfig` consumer boundary manifest.

#[path = "support/pending_review_date.rs"]
mod pending_review_date;

use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use time::{Date, macros::date};
use weaver_docs_gate::{
    BoundaryManifest,
    BoundaryState,
    BoundaryTask,
    load_manifest,
    render_matrix,
};

const ADR_007: &str = "docs/adr-007-agent-native-command-surface.md";
const MANIFEST: &str = "docs/orthoconfig-consumer-boundary.toml";
const MANIFEST_BUILD_DATE: Date = date!(2026 - 06 - 21);
const MATRIX: &str = "docs/orthoconfig-consumer-boundary.md";
const ROADMAP: &str = "docs/roadmap.md";

type TestResult<T = ()> = Result<T, String>;

#[derive(Clone, Copy)]
enum FieldName {
    LastReviewed,
    NextReviewBy,
    ManagedTasks,
}

impl FieldName {
    /// Return the manifest spelling for a validated field.
    const fn as_str(self) -> &'static str {
        match self {
            Self::LastReviewed => "last_reviewed",
            Self::NextReviewBy => "next_review_by",
            Self::ManagedTasks => "managed_tasks",
        }
    }
}

/// Prove the explicit managed-task registry matches rows and roadmap tasks.
#[test]
fn manifest_registry_matches_rows_and_roadmap_tasks() -> TestResult {
    let manifest = manifest().map_err(|error| format!("load boundary manifest: {error}"))?;
    let row_ids = manifest
        .tasks
        .iter()
        .map(|task| task.id.as_str())
        .collect::<Vec<_>>();
    let managed_ids = manifest
        .managed_tasks
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    ensure_equal(&managed_ids, &row_ids, "managed_tasks must match task rows")?;
    ensure_unique(&managed_ids, FieldName::ManagedTasks)?;

    let roadmap =
        read_doc(Utf8Path::new(ROADMAP)).map_err(|error| format!("read {ROADMAP}: {error}"))?;
    let roadmap_ids = roadmap_task_ids(&roadmap);
    for task_id in managed_ids {
        ensure(
            roadmap_ids.contains(task_id),
            format!("manifest task {task_id} is missing from {ROADMAP}"),
        )?;
    }

    Ok(())
}

/// Prove every row carries the evidence required by its state.
#[test]
fn boundary_state_rows_have_required_evidence() -> TestResult {
    let manifest = manifest().map_err(|error| format!("load boundary manifest: {error}"))?;

    for task in &manifest.tasks {
        validate_date(&task.last_reviewed, FieldName::LastReviewed, task)?;
        ensure(
            !task.upstream.is_empty(),
            format!("task {} must name at least one upstream reference", task.id),
        )?;

        validate_state_evidence(task)?;
    }

    Ok(())
}

/// Prove divergent rows link to existing ADR 007 sections.
#[test]
fn divergent_rows_reference_existing_adr_007_anchors() -> TestResult {
    let manifest = manifest().map_err(|error| format!("load boundary manifest: {error}"))?;
    let adr =
        read_doc(Utf8Path::new(ADR_007)).map_err(|error| format!("read {ADR_007}: {error}"))?;
    let anchors = heading_anchors(&adr);

    for task in manifest
        .tasks
        .iter()
        .filter(|task| task.state == BoundaryState::Divergent)
    {
        let anchor = task
            .adr_anchor
            .as_deref()
            .ok_or_else(|| format!("divergent task {} has no ADR anchor", task.id))?;
        ensure(
            anchors.contains(anchor),
            format!(
                "task {} references missing ADR 007 anchor {anchor}",
                task.id
            ),
        )?;
    }

    Ok(())
}

/// Prove the checked-in Markdown matrix is generated from the manifest.
#[test]
fn committed_matrix_matches_manifest_rendering() -> TestResult {
    let manifest = manifest().map_err(|error| format!("load boundary manifest: {error}"))?;
    let expected = render_matrix(&manifest);
    let actual =
        read_doc(Utf8Path::new(MATRIX)).map_err(|error| format!("read {MATRIX}: {error}"))?;

    ensure_equal(
        &expected,
        &actual,
        format!("{MATRIX} is not generated from {MANIFEST}"),
    )
}

/// Load the boundary manifest from the repository docs.
fn manifest() -> TestResult<BoundaryManifest> {
    let contents =
        read_doc(Utf8Path::new(MANIFEST)).map_err(|error| format!("read {MANIFEST}: {error}"))?;
    load_manifest(contents.as_bytes()).map_err(|error| error.to_string())
}

/// Read a repository documentation file as UTF-8 text.
fn read_doc(doc_path: &Utf8Path) -> TestResult<String> {
    let resolved_path = repo_path(doc_path)?;
    let parent = resolved_path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = resolved_path
        .file_name()
        .ok_or_else(|| format!("invalid doc path: {resolved_path}"))?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|error| format!("open {parent}: {error}"))?;
    dir.read_to_string(file_name)
        .map_err(|error| format!("read {resolved_path}: {error}"))
}

/// Resolve a repository-relative path from the test crate directory.
fn repo_path(path: &Utf8Path) -> TestResult<Utf8PathBuf> {
    let crate_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir
        .parent()
        .and_then(Utf8Path::parent)
        .ok_or_else(|| format!("cannot resolve repository root from {crate_dir}"))?;
    Ok(repo_root.join(path))
}

/// Extract roadmap task IDs from Markdown task headings.
fn roadmap_task_ids(roadmap: &str) -> BTreeSet<&str> {
    roadmap
        .lines()
        .filter_map(|line| line.trim_start().split_once("] ").map(|(_, task)| task))
        .filter_map(|task| task.split_once(". ").map(|(task_id, _)| task_id))
        .filter(|task_id| {
            task_id
                .chars()
                .all(|char| char.is_ascii_digit() || char == '.')
        })
        .collect()
}

/// Extract GitHub-style heading anchors from a Markdown document.
fn heading_anchors(document: &str) -> BTreeSet<String> {
    document
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("## "))
        .map(markdown_anchor)
        .collect()
}

/// Convert a heading into the anchor form used by GitHub Markdown.
fn markdown_anchor(heading: &str) -> String {
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

/// Return an error when a test invariant is false.
fn ensure(condition: bool, message: impl Into<String>) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

/// Return a diff-friendly error when two values differ.
fn ensure_equal<T>(left: &T, right: &T, message: impl Into<String>) -> TestResult
where
    T: std::fmt::Debug + PartialEq,
{
    if left == right {
        Ok(())
    } else {
        Err(format!(
            "{}\nleft: {left:?}\nright: {right:?}",
            message.into()
        ))
    }
}

/// Ensure a manifest field contains no duplicate values.
fn ensure_unique(values: &[&str], label: FieldName) -> TestResult {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure(
            seen.insert(*value),
            format!("{} contains duplicate value {value}", label.as_str()),
        )?;
    }
    Ok(())
}

/// Validate required and forbidden evidence fields for one state.
fn validate_field_constraints(
    task: &BoundaryTask,
    state: &str,
    required: (&str, bool),
    forbidden: &[(&str, bool)],
) -> TestResult {
    ensure(
        required.1,
        format!("{state} task {} must name {}", task.id, required.0),
    )?;
    for &(field, is_some) in forbidden {
        ensure(
            !is_some,
            format!("{state} task {} must not carry {field}", task.id),
        )?;
    }
    Ok(())
}

/// Validate the additional freshness evidence required for pending rows.
fn validate_pending_evidence(task: &BoundaryTask) -> TestResult {
    let next_review_by = task
        .next_review_by
        .as_deref()
        .ok_or_else(|| format!("pending task {} must provide next_review_by", task.id))?;
    validate_date(next_review_by, FieldName::NextReviewBy, task)?;
    pending_review_date::validate_pending_review_date(
        next_review_by,
        &task.id,
        MANIFEST_BUILD_DATE,
    )?;
    ensure(
        task.shipped_in.is_none(),
        format!("pending task {} must not carry shipped_in", task.id),
    )?;
    ensure(
        task.removal_gate.is_none(),
        format!("pending task {} must not carry removal_gate", task.id),
    )?;
    ensure(
        task.adr_anchor.is_none(),
        format!("pending task {} must not carry adr_anchor", task.id),
    )
}

/// Validate state-specific evidence for one manifest row.
fn validate_state_evidence(task: &BoundaryTask) -> TestResult {
    let (state, required, forbidden) = match task.state {
        BoundaryState::Consumes => (
            "consumes",
            ("shipped_in", task.shipped_in.is_some()),
            [
                ("removal_gate", task.removal_gate.is_some()),
                ("adr_anchor", task.adr_anchor.is_some()),
                ("next_review_by", task.next_review_by.is_some()),
            ],
        ),
        BoundaryState::Wraps => (
            "wraps",
            ("removal_gate", task.removal_gate.is_some()),
            [
                ("shipped_in", task.shipped_in.is_some()),
                ("adr_anchor", task.adr_anchor.is_some()),
                ("next_review_by", task.next_review_by.is_some()),
            ],
        ),
        BoundaryState::Divergent => (
            "divergent",
            ("adr_anchor", task.adr_anchor.is_some()),
            [
                ("shipped_in", task.shipped_in.is_some()),
                ("removal_gate", task.removal_gate.is_some()),
                ("next_review_by", task.next_review_by.is_some()),
            ],
        ),
        BoundaryState::Pending => return validate_pending_evidence(task),
    };
    validate_field_constraints(task, state, required, &forbidden)
}

/// Validate an ISO-like date field in one manifest row.
fn validate_date(value: &str, field: FieldName, task: &BoundaryTask) -> TestResult {
    ensure(is_iso_date(value), invalid_date_message(value, field, task))
}

/// Build a diagnostic for an invalid date field.
fn invalid_date_message(value: &str, field: FieldName, task: &BoundaryTask) -> String {
    format!(
        "task {} has invalid {} date {value:?}",
        task.id,
        field.as_str()
    )
}

/// Return whether a value is shaped as an ISO `YYYY-MM-DD` date.
fn is_iso_date(value: &str) -> bool {
    value.len() == 10
        && value
            .chars()
            .enumerate()
            .all(|(index, char)| matches!(index, 4 | 7) == (char == '-'))
        && value
            .chars()
            .filter(|char| *char != '-')
            .all(|char| char.is_ascii_digit())
}
