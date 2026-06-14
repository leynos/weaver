//! Integration tests for the `OrthoConfig` consumer boundary manifest.

use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use weaver_docs_gate::{
    BoundaryManifest,
    BoundaryState,
    BoundaryTask,
    load_manifest,
    render_matrix,
};

const ADR_007: &str = "docs/adr-007-agent-native-command-surface.md";
const MANIFEST: &str = "docs/orthoconfig-consumer-boundary.toml";
const MATRIX: &str = "docs/orthoconfig-consumer-boundary.md";
const ROADMAP: &str = "docs/roadmap.md";

type TestResult<T = ()> = Result<T, String>;

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
    ensure_unique(&managed_ids, "managed_tasks")?;

    let roadmap = read_doc(ROADMAP).map_err(|error| format!("read {ROADMAP}: {error}"))?;
    let roadmap_ids = roadmap_task_ids(&roadmap);
    for task_id in managed_ids {
        ensure(
            roadmap_ids.contains(task_id),
            format!("manifest task {task_id} is missing from {ROADMAP}"),
        )?;
    }

    Ok(())
}

#[test]
fn boundary_state_rows_have_required_evidence() -> TestResult {
    let manifest = manifest().map_err(|error| format!("load boundary manifest: {error}"))?;

    for task in &manifest.tasks {
        validate_date(&task.last_reviewed, "last_reviewed", task)?;
        ensure(
            !task.upstream.is_empty(),
            format!("task {} must name at least one upstream reference", task.id),
        )?;

        match task.state {
            BoundaryState::Consumes => validate_consumes_evidence(task),
            BoundaryState::Wraps => validate_wraps_evidence(task),
            BoundaryState::Pending => validate_pending_evidence(task),
            BoundaryState::Divergent => validate_divergence_evidence(task),
        }?;
    }

    Ok(())
}

#[test]
fn divergent_rows_reference_existing_adr_007_anchors() -> TestResult {
    let manifest = manifest().map_err(|error| format!("load boundary manifest: {error}"))?;
    let adr = read_doc(ADR_007).map_err(|error| format!("read {ADR_007}: {error}"))?;
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

#[test]
fn committed_matrix_matches_manifest_rendering() -> TestResult {
    let manifest = manifest().map_err(|error| format!("load boundary manifest: {error}"))?;
    let expected = render_matrix(&manifest);
    let actual = read_doc(MATRIX).map_err(|error| format!("read {MATRIX}: {error}"))?;

    ensure_equal(
        &expected,
        &actual,
        format!("{MATRIX} is not generated from {MANIFEST}"),
    )
}

fn manifest() -> TestResult<BoundaryManifest> {
    load_manifest(&repo_path(MANIFEST)?).map_err(|error| error.to_string())
}

fn read_doc(doc_path: &str) -> TestResult<String> {
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

fn repo_path(path: &str) -> TestResult<Utf8PathBuf> {
    let crate_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir
        .parent()
        .and_then(Utf8Path::parent)
        .ok_or_else(|| format!("cannot resolve repository root from {crate_dir}"))?;
    Ok(repo_root.join(path))
}

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

fn heading_anchors(document: &str) -> BTreeSet<String> {
    document
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("## "))
        .map(markdown_anchor)
        .collect()
}

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

fn push_dash(anchor: &mut String, previous_was_dash: &mut bool) {
    if !*previous_was_dash && !anchor.is_empty() {
        anchor.push('-');
        *previous_was_dash = true;
    }
}

fn ensure(condition: bool, message: String) -> TestResult {
    if condition { Ok(()) } else { Err(message) }
}

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

fn ensure_unique(values: &[&str], label: &str) -> TestResult {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure(
            seen.insert(*value),
            format!("{label} contains duplicate value {value}"),
        )?;
    }
    Ok(())
}

fn validate_consumes_evidence(task: &BoundaryTask) -> TestResult {
    ensure(
        task.shipped_in.is_some(),
        format!("consumes task {} must name shipped_in", task.id),
    )?;
    ensure(
        task.removal_gate.is_none(),
        format!("consumes task {} must not carry removal_gate", task.id),
    )?;
    ensure(
        task.adr_anchor.is_none(),
        format!("consumes task {} must not carry adr_anchor", task.id),
    )?;
    ensure(
        task.next_review_by.is_none(),
        format!("consumes task {} must not carry next_review_by", task.id),
    )
}

fn validate_wraps_evidence(task: &BoundaryTask) -> TestResult {
    ensure(
        task.removal_gate.is_some(),
        format!("wraps task {} must name a removal gate", task.id),
    )?;
    ensure(
        task.shipped_in.is_none(),
        format!("wraps task {} must not carry shipped_in", task.id),
    )?;
    ensure(
        task.adr_anchor.is_none(),
        format!("wraps task {} must not carry adr_anchor", task.id),
    )?;
    ensure(
        task.next_review_by.is_none(),
        format!("wraps task {} must not carry next_review_by", task.id),
    )
}

fn validate_pending_evidence(task: &BoundaryTask) -> TestResult {
    let next_review_by = task
        .next_review_by
        .as_deref()
        .ok_or_else(|| format!("pending task {} must provide next_review_by", task.id))?;
    validate_date(next_review_by, "next_review_by", task)?;
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

fn validate_divergence_evidence(task: &BoundaryTask) -> TestResult {
    ensure(
        task.adr_anchor.is_some(),
        format!("divergent task {} must name an ADR 007 anchor", task.id),
    )?;
    ensure(
        task.shipped_in.is_none(),
        format!("divergent task {} must not carry shipped_in", task.id),
    )?;
    ensure(
        task.removal_gate.is_none(),
        format!("divergent task {} must not carry removal_gate", task.id),
    )?;
    ensure(
        task.next_review_by.is_none(),
        format!("divergent task {} must not carry next_review_by", task.id),
    )
}

fn validate_date(value: &str, field: &str, task: &BoundaryTask) -> TestResult {
    ensure(
        is_iso_date(value),
        format!("task {} has invalid {field} date {value:?}", task.id),
    )
}

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
