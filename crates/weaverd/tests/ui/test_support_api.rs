//! Downstream compile-time usage of the `weaverd` `test-support` API.
//!
//! This fixture is compiled by `trybuild`, not by Cargo, and it only touches
//! items reachable from the crate root, exactly as an external consumer would.
//! It pins the dispatch handler constructor's parameter types, including the
//! `RuntimePaths` argument that replaced a raw `PathBuf`.

// `trybuild` mirrors the features of the crate under test onto the generated
// fixture crate and forwards them to `weaverd`. If that forwarding ever stops
// working, the fixture would compile against the ungated surface and pin
// nothing, so refuse to compile instead.
#[cfg(not(feature = "test-support"))]
compile_error!(
    "the `test-support` feature must reach this fixture; run the suite with \
     `cargo test -p weaverd --features test-support`"
);

use std::path::PathBuf;

use weaver_config::RuntimePaths;
use weaverd::{BackendManager, DispatchConnectionHandler};

fn main() {}

/// Type-checks the constructor call without needing a live backend.
///
/// The function is never called. It exists so the fixture pins the parameter
/// types of `DispatchConnectionHandler::new`, which is the surface this suite
/// protects.
fn construct_handler(
    backends: BackendManager,
    workspace_root: PathBuf,
    endpoint: String,
    runtime_paths: RuntimePaths,
) {
    let _ = DispatchConnectionHandler::new(backends, workspace_root, endpoint, runtime_paths);
}
