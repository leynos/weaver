//! Downstream compile-time usage of the `weaver-docs-gate` public API.

use std::{error::Error, fmt::Debug};

use camino::Utf8Path;
use weaver_docs_gate::{
    BoundaryError,
    BoundaryFileError,
    BoundaryManifest,
    BoundaryState,
    BoundaryTask,
    UpstreamRef,
    UpstreamRole,
    load_manifest,
    load_manifest_file,
    render_matrix,
};

fn assert_error_type<E>()
where
    E: Error + Debug + Send + Sync + 'static,
{
}

fn main() -> Result<(), Box<dyn Error>> {
    assert_error_type::<BoundaryError>();
    assert_error_type::<BoundaryFileError>();

    let manifest = BoundaryManifest {
        schema_version: 1,
        managed_tasks: vec!["12.1.1".into()],
        tasks: vec![BoundaryTask {
            id: "12.1.1".into(),
            gist: "Track the downstream consumer boundary.".into(),
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
        }],
    };

    let _state = BoundaryState::Consumes.as_str();
    let _role = UpstreamRole::Renderer.as_str();
    let _matrix = render_matrix(&manifest);

    let _loaded = load_manifest(
        br#"schema_version = 1
managed_tasks = []
task = []
"#
        .as_slice(),
    )?;

    let _file_result: Result<BoundaryManifest, BoundaryFileError> =
        load_manifest_file(Utf8Path::new("docs/orthoconfig-consumer-boundary.toml"));

    Ok(())
}
