//! Cross-crate checks that command discoverability matches daemon routing.

use std::collections::BTreeMap;

use weaver_cli::domain_operations;

fn cli_catalogue() -> BTreeMap<&'static str, Vec<&'static str>> {
    domain_operations()
        .iter()
        .map(|entry| (entry.domain, entry.operations.to_vec()))
        .collect()
}

fn daemon_catalogue() -> BTreeMap<&'static str, Vec<&'static str>> {
    weaverd::test_support::routing_catalogue()
        .iter()
        .map(|(domain, operations)| (*domain, operations.to_vec()))
        .collect()
}

#[test]
fn catalogue_agreement() {
    assert_eq!(
        cli_catalogue(),
        daemon_catalogue(),
        "CLI discoverability must describe exactly the daemon's known operations",
    );
}
