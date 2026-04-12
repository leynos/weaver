//! Tests for the `weaver-cards` crate.

use std::path::{Path, PathBuf};

mod behaviour;
mod cache_behaviour;
mod cache_tests;
mod extractor;
mod extractor_boundaries;
mod fixtures;
mod graph_slice_behaviour;
mod graph_slice_fixtures;
mod graph_slice_snapshot_tests;
mod round_trip_tests;
mod snapshot_tests;
mod test_utils;

fn absolute_test_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new("/tmp/weaver-cards-tests").join(path)
    }
}
