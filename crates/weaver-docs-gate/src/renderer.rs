//! Markdown rendering for the `OrthoConfig` consumer boundary matrix.

use std::collections::BTreeMap;

use crate::{BoundaryManifest, BoundaryState, BoundaryTask};

const NA: &str = "n/a";

struct MatrixRow {
    roadmap_task: String,
    gist: String,
    state: String,
    upstream: String,
    shipped_in: String,
    gate_or_divergence: String,
    next_review_by: String,
    last_reviewed: String,
}

struct ColumnWidths {
    roadmap_task: usize,
    gist: usize,
    state: usize,
    upstream: usize,
    shipped_in: usize,
    gate_or_divergence: usize,
    next_review_by: usize,
    last_reviewed: usize,
}

impl ColumnWidths {
    fn for_rows(rows: &[MatrixRow]) -> Self {
        let mut widths = Self::headers();
        for row in rows {
            widths.include(row);
        }
        widths
    }

    const fn headers() -> Self {
        Self {
            roadmap_task: 12,
            gist: 4,
            state: 5,
            upstream: 25,
            shipped_in: 10,
            gate_or_divergence: 26,
            next_review_by: 14,
            last_reviewed: 13,
        }
    }

    fn include(&mut self, row: &MatrixRow) {
        self.roadmap_task = self.roadmap_task.max(cell_width(&row.roadmap_task));
        self.gist = self.gist.max(cell_width(&row.gist));
        self.state = self.state.max(cell_width(&row.state));
        self.upstream = self.upstream.max(cell_width(&row.upstream));
        self.shipped_in = self.shipped_in.max(cell_width(&row.shipped_in));
        self.gate_or_divergence = self
            .gate_or_divergence
            .max(cell_width(&row.gate_or_divergence));
        self.next_review_by = self.next_review_by.max(cell_width(&row.next_review_by));
        self.last_reviewed = self.last_reviewed.max(cell_width(&row.last_reviewed));
    }
}

/// Render the human-readable boundary matrix from the manifest.
///
/// # Examples
/// ```
/// use weaver_docs_gate::{BoundaryManifest, BoundaryState, BoundaryTask, render_matrix};
///
/// let manifest = BoundaryManifest {
///     schema_version: 1,
///     managed_tasks: vec!["12.1.1".into()],
///     tasks: vec![BoundaryTask {
///         id: "12.1.1".into(),
///         gist: "Track the downstream consumer boundary.".into(),
///         state: BoundaryState::Consumes,
///         upstream: Vec::new(),
///         shipped_in: Some("4339a6f3".into()),
///         removal_gate: None,
///         adr_anchor: None,
///         next_review_by: None,
///         last_reviewed: "2026-06-14".into(),
///     }],
/// };
///
/// assert!(render_matrix(&manifest).contains("12.1.1"));
/// ```
#[must_use]
pub fn render_matrix(manifest: &BoundaryManifest) -> String {
    let mut rendered = String::from(concat!(
        "# OrthoConfig consumer boundary\n\n",
        "<!-- markdownlint-disable MD013 MD060 -->\n\n",
        "This matrix is generated from ",
        "`docs/orthoconfig-consumer-boundary.toml`. Do not\n",
        "edit the table by hand; update the manifest and regenerate it with\n",
        "`cargo run -p weaver-docs-gate --example render_boundary_matrix -- ",
        "docs/orthoconfig-consumer-boundary.toml ",
        "docs/orthoconfig-consumer-boundary.md`.\n\n",
        "The matrix tracks every live Weaver command-contract roadmap task ",
        "that consumes\n",
        "OrthoConfig, wraps it temporarily, waits on upstream shape, or ",
        "deliberately\n",
        "diverges under ADR 007.\n\n",
    ));

    for (phase, rows) in grouped_rows(manifest) {
        push_phase(&mut rendered, phase, &rows);
    }

    while rendered.ends_with("\n\n") {
        rendered.pop();
    }
    rendered.push_str("<!-- markdownlint-enable MD013 MD060 -->\n");

    rendered
}

fn grouped_rows(manifest: &BoundaryManifest) -> BTreeMap<&str, Vec<MatrixRow>> {
    let mut phases = BTreeMap::new();
    for task in &manifest.tasks {
        let phase = task.id.split('.').next().unwrap_or_default();
        phases
            .entry(phase)
            .or_insert_with(Vec::new)
            .push(MatrixRow::from(task));
    }
    phases
}

fn push_phase(rendered: &mut String, phase: &str, rows: &[MatrixRow]) {
    let widths = ColumnWidths::for_rows(rows);

    rendered.push_str("## Phase ");
    rendered.push_str(phase);
    rendered.push_str("\n\n");
    push_header(rendered, &widths);
    push_separator(rendered, &widths);
    for row in rows {
        push_row(rendered, row, &widths);
    }
    rendered.push('\n');
}

fn push_header(rendered: &mut String, widths: &ColumnWidths) {
    push_cells(
        rendered,
        widths,
        &MatrixRow {
            roadmap_task: "Roadmap task".into(),
            gist: "Gist".into(),
            state: "State".into(),
            upstream: "Upstream OrthoConfig task".into(),
            shipped_in: "Shipped in".into(),
            gate_or_divergence: "Removal gate or divergence".into(),
            next_review_by: "Next review by".into(),
            last_reviewed: "Last reviewed".into(),
        },
    );
}

fn push_separator(rendered: &mut String, widths: &ColumnWidths) {
    push_cells(
        rendered,
        widths,
        &MatrixRow {
            roadmap_task: "-".repeat(widths.roadmap_task),
            gist: "-".repeat(widths.gist),
            state: "-".repeat(widths.state),
            upstream: "-".repeat(widths.upstream),
            shipped_in: "-".repeat(widths.shipped_in),
            gate_or_divergence: "-".repeat(widths.gate_or_divergence),
            next_review_by: "-".repeat(widths.next_review_by),
            last_reviewed: "-".repeat(widths.last_reviewed),
        },
    );
}

fn push_row(rendered: &mut String, row: &MatrixRow, widths: &ColumnWidths) {
    push_cells(rendered, widths, row);
}

fn push_cells(rendered: &mut String, widths: &ColumnWidths, row: &MatrixRow) {
    rendered.push_str("| ");
    rendered.push_str(&padded(&row.roadmap_task, widths.roadmap_task));
    rendered.push_str(" | ");
    rendered.push_str(&padded(&row.gist, widths.gist));
    rendered.push_str(" | ");
    rendered.push_str(&padded(&row.state, widths.state));
    rendered.push_str(" | ");
    rendered.push_str(&padded(&row.upstream, widths.upstream));
    rendered.push_str(" | ");
    rendered.push_str(&padded(&row.shipped_in, widths.shipped_in));
    rendered.push_str(" | ");
    rendered.push_str(&padded(&row.gate_or_divergence, widths.gate_or_divergence));
    rendered.push_str(" | ");
    rendered.push_str(&padded(&row.next_review_by, widths.next_review_by));
    rendered.push_str(" | ");
    rendered.push_str(&padded(&row.last_reviewed, widths.last_reviewed));
    rendered.push_str(" |\n");
}

impl From<&BoundaryTask> for MatrixRow {
    fn from(task: &BoundaryTask) -> Self {
        Self {
            roadmap_task: format!("[{}]({})", escape_cell(&task.id), roadmap_anchor(&task.id)),
            gist: escape_cell(&task.gist),
            state: state_label(task.state).into(),
            upstream: upstream_tasks(task),
            shipped_in: optional_cell(task.shipped_in.as_deref()),
            gate_or_divergence: gate_or_divergence(task),
            next_review_by: optional_cell(task.next_review_by.as_deref()),
            last_reviewed: escape_cell(&task.last_reviewed),
        }
    }
}

fn upstream_tasks(task: &BoundaryTask) -> String {
    let mut upstream = String::new();
    for reference in &task.upstream {
        if !upstream.is_empty() {
            upstream.push_str(", ");
        }
        upstream.push_str(&reference.task);
    }
    optional_cell(Some(&upstream))
}

fn gate_or_divergence(task: &BoundaryTask) -> String {
    if let Some(gate) = task.removal_gate.as_deref() {
        return escape_cell(gate);
    }

    task.adr_anchor.as_deref().map_or_else(
        || NA.into(),
        |anchor| format!("[ADR 007](adr-007-agent-native-command-surface.md#{anchor})"),
    )
}

fn optional_cell(value: Option<&str>) -> String {
    value
        .filter(|inner| !inner.is_empty())
        .map_or_else(|| NA.into(), escape_cell)
}

const fn state_label(state: BoundaryState) -> &'static str {
    match state {
        BoundaryState::Consumes => "✓ consumes",
        BoundaryState::Wraps => "~ wraps",
        BoundaryState::Pending => "? pending",
        BoundaryState::Divergent => "× divergent",
    }
}

fn roadmap_anchor(task_id: &str) -> &'static str {
    match task_id.split('.').next().unwrap_or_default() {
        "12" => "roadmap.md#121-confirm-reusable-contracts-that-weaver-must-not-duplicate",
        "13" => "roadmap.md#13-command-contract-proving-slice",
        "14" => "roadmap.md#14-code-reading-loop-slice",
        "15" => "roadmap.md#15-sempai-selector-to-context-slice",
        "16" => "roadmap.md#16-safe-change-loop-slice",
        "17" => "roadmap.md#17-impact-and-history-slice",
        "18" => "roadmap.md#18-provider-ecosystem-slice",
        "19" => "roadmap.md#19-agent-workflow-and-assurance-slice",
        "20" => "roadmap.md#20-deferred-extensions-after-the-core-product-promise",
        _ => "roadmap.md",
    }
}

fn escape_cell(value: &str) -> String { value.replace('|', "\\|").replace('\n', "<br>") }

fn padded(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(cell_width(value));
    format!("{value}{}", " ".repeat(padding))
}

fn cell_width(value: &str) -> usize { value.chars().count() }

#[cfg(test)]
mod tests {
    //! Unit, snapshot, and property coverage for Markdown matrix rendering.

    use proptest::prelude::*;

    use super::*;
    use crate::{BoundaryManifest, UpstreamRef, UpstreamRole};

    #[test]
    fn groups_rows_by_phase_without_prefix_collisions() {
        let manifest = manifest_with_tasks(vec![task("1.1.1"), task("12.1.1")]);

        let rows = grouped_rows(&manifest);

        assert_eq!(rows.get("1").map(Vec::len), Some(1));
        assert_eq!(rows.get("12").map(Vec::len), Some(1));
    }

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

    fn manifest_with_tasks(tasks: Vec<BoundaryTask>) -> BoundaryManifest {
        let managed_tasks = tasks.iter().map(|task| task.id.clone()).collect();
        BoundaryManifest {
            schema_version: 1,
            managed_tasks,
            tasks,
        }
    }

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
}
