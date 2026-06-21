//! Unit, snapshot, and property coverage for Markdown matrix rendering.
//!
//! These tests exercise the private helpers behind `renderer::render_matrix`.
//! They prove that manifest rows are grouped by roadmap phase, escaped into
//! Markdown table cells, padded with stable column widths, and rendered into
//! the generated matrix shape consumed by the integration gate.

use proptest::prelude::*;

use super::*;
use crate::{BoundaryManifest, UpstreamRef, UpstreamRole};

/// Prove numeric phase grouping does not confuse `1.*` with `12.*` tasks.
#[test]
fn groups_rows_by_phase_without_prefix_collisions() {
    let manifest = manifest_with_task_ids(vec!["1.1.1".into(), "12.1.1".into()]);

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
    let manifest = manifest_with_task_ids(vec!["12.1.1".into()]);

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
    /// Prove computed widths accommodate every generated escaped cell.
    fn column_widths_accommodate_all_generated_cells(
        roadmap_task in "[A-Za-z0-9 .|\\n]{0,32}",
        gist in "[A-Za-z0-9 .|\\n]{0,64}",
        upstream in "[A-Za-z0-9 .|\\n]{0,64}",
        gate_or_divergence in "[A-Za-z0-9 .|\\n]{0,64}",
    ) {
        let row = MatrixRow {
            roadmap_task: escape_cell(&roadmap_task),
            gist: escape_cell(&gist),
            state: "✓ consumes".into(),
            upstream: escape_cell(&upstream),
            shipped_in: "4339a6f3".into(),
            gate_or_divergence: escape_cell(&gate_or_divergence),
            next_review_by: "n/a".into(),
            last_reviewed: "2026-06-20".into(),
        };

        let widths = ColumnWidths::for_rows(&[row]);

        prop_assert!(widths.roadmap_task >= cell_width(&escape_cell(&roadmap_task)));
        prop_assert!(widths.gist >= cell_width(&escape_cell(&gist)));
        prop_assert!(widths.upstream >= cell_width(&escape_cell(&upstream)));
        prop_assert!(widths.gate_or_divergence >= cell_width(&escape_cell(&gate_or_divergence)));
    }

    #[test]
    /// Prove cell escaping removes raw newlines and unescaped table pipes.
    fn escaped_cells_do_not_contain_table_breaks(value in "[A-Za-z0-9 .|\\n]{0,128}") {
        let escaped = escape_cell(&value);

        prop_assert!(!escaped.contains('\n'));
        prop_assert!(!has_unescaped_pipe(&escaped));
    }

    #[test]
    /// Prove arbitrary numeric phase IDs are grouped exactly once.
    fn phase_grouping_preserves_arbitrary_phase_ids(phases in proptest::collection::vec(1u16..=999, 1..32)) {
        let task_ids = phases
            .iter()
            .enumerate()
            .map(|(index, phase)| format!("{phase}.{index}.1"))
            .collect::<Vec<_>>();
        let manifest = manifest_with_task_ids(task_ids);
        let rows = grouped_rows(&manifest);
        let expected = phases
            .iter()
            .map(u16::to_string)
            .collect::<std::collections::BTreeSet<_>>();

        prop_assert_eq!(rows.keys().map(|phase| (*phase).to_owned()).collect::<std::collections::BTreeSet<_>>(), expected);
        for (phase, grouped) in rows {
            let prefix = format!("[{phase}.");
            prop_assert!(grouped.iter().all(|row| row.roadmap_task.starts_with(&prefix)));
        }
    }

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

        let rendered = render_matrix(&BoundaryManifest {
            schema_version: 1,
            managed_tasks: vec!["12.1.1".into()],
            tasks: vec![boundary_task],
        });
        for line in rendered.lines().filter(|line| line.starts_with("| [")) {
            prop_assert!(line.matches(" | ").count() == 7);
        }
    }
}

/// Return whether a rendered cell contains a pipe not escaped for Markdown.
fn has_unescaped_pipe(value: &str) -> bool {
    let mut previous_was_escape = false;
    for character in value.chars() {
        if character == '|' && !previous_was_escape {
            return true;
        }
        previous_was_escape = character == '\\' && !previous_was_escape;
    }
    false
}

/// Build a manifest containing representative rows for the supplied task IDs.
fn manifest_with_task_ids(task_ids: Vec<String>) -> BoundaryManifest {
    let (managed_tasks, tasks) = task_ids
        .into_iter()
        .map(|id| {
            let boundary_task = task(&id);
            (id, boundary_task)
        })
        .unzip();
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
