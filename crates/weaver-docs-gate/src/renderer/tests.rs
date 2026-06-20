//! Unit, snapshot, and property coverage for Markdown matrix rendering.

use proptest::prelude::*;

use super::*;
use crate::{BoundaryManifest, UpstreamRef, UpstreamRole};

/// Prove numeric phase grouping does not confuse `1.*` with `12.*` tasks.
#[test]
fn groups_rows_by_phase_without_prefix_collisions() {
    let manifest = manifest_with_tasks(vec![task("1.1.1"), task("12.1.1")]);

    let rows = grouped_rows(&manifest);

    assert_eq!(rows.get("1").map(Vec::len), Some(1));
    assert_eq!(rows.get("12").map(Vec::len), Some(1));
}

/// Prove column widths are based on escaped cell text.
#[test]
fn computes_column_widths_from_escaped_cells() {
    let rows = vec![MatrixRow {
        roadmap_task: "[12.1.1](roadmap.md#task)".into(),
        gist: "contains \\| pipe".into(),
        state: "✓ consumes".into(),
        upstream: "upstream contract".into(),
        shipped_in: "4339a6f3".into(),
        gate_or_divergence: "n/a".into(),
        next_review_by: "n/a".into(),
        last_reviewed: "2026-06-20".into(),
    }];

    let widths = ColumnWidths::for_rows(&rows);

    assert_eq!(widths.gist, "contains \\| pipe".chars().count());
    assert_eq!(widths.last_reviewed, "Last reviewed".chars().count());
}

/// Pin the stable generated Markdown shape for one representative task.
#[test]
fn snapshots_rendered_matrix_shape() {
    let manifest = manifest_with_tasks(vec![task("12.1.1")]);

    insta::assert_snapshot!(render_matrix(&manifest), @r###"
    # OrthoConfig consumer boundary

    <!-- markdownlint-disable MD013 MD060 -->

    This matrix is generated from `docs/orthoconfig-consumer-boundary.toml`. Do not
    edit the table by hand; update the manifest and regenerate it with
    `cargo run -p weaver-docs-gate --example render_boundary_matrix -- docs/orthoconfig-consumer-boundary.toml docs/orthoconfig-consumer-boundary.md`.

    The matrix tracks every live Weaver command-contract roadmap task that consumes
    OrthoConfig, wraps it temporarily, waits on upstream shape, or deliberately
    diverges under ADR 007.

    ## Phase 12

    | Roadmap task                                                                       | Gist                | State      | Upstream OrthoConfig task | Shipped in | Removal gate or divergence | Next review by | Last reviewed |
    | ---------------------------------------------------------------------------------- | ------------------- | ---------- | ------------------------- | ---------- | -------------------------- | -------------- | ------------- |
    | [12.1.1](roadmap.md#121-confirm-reusable-contracts-that-weaver-must-not-duplicate) | Review \| renderer. | ✓ consumes | renderer-contract         | 4339a6f3   | n/a                        | n/a            | 2026-06-20    |
    <!-- markdownlint-enable MD013 MD060 -->
    "###);
}

proptest! {
    #[test]
    /// Prove arbitrary task text cannot add Markdown table columns.
    fn rendered_task_cells_never_emit_raw_table_pipes(
        gist in "[A-Za-z0-9 |\\n]{0,64}",
        upstream in "[A-Za-z0-9 |\\n]{0,64}",
    ) {
        let mut boundary_task = task("12.1.1");
        boundary_task.gist = gist;
        boundary_task.upstream = vec![UpstreamRef {
            task: upstream,
            role: UpstreamRole::Renderer,
        }];

        let rendered = render_matrix(&manifest_with_tasks(vec![boundary_task]));
        for line in rendered.lines().filter(|line| line.starts_with("| [")) {
            prop_assert!(line.matches(" | ").count() == 7);
        }
    }
}

/// Build a manifest containing the supplied task rows.
fn manifest_with_tasks(tasks: Vec<BoundaryTask>) -> BoundaryManifest {
    let managed_tasks = tasks.iter().map(|task| task.id.clone()).collect();
    BoundaryManifest {
        schema_version: 1,
        managed_tasks,
        tasks,
    }
}

/// Build a representative consumed boundary task.
fn task(id: &str) -> BoundaryTask {
    BoundaryTask {
        id: id.into(),
        gist: "Review | renderer.".into(),
        state: BoundaryState::Consumes,
        upstream: vec![UpstreamRef {
            task: "renderer-contract".into(),
            role: UpstreamRole::Renderer,
        }],
        shipped_in: Some("4339a6f3".into()),
        removal_gate: None,
        adr_anchor: None,
        next_review_by: None,
        last_reviewed: "2026-06-20".into(),
    }
}
