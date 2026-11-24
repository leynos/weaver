//! Cross-crate checks that command discoverability matches daemon routing.

use std::collections::BTreeMap;

use weaver_cli::domain_operations;

/// Builds the CLI catalogue while rejecting duplicate domain declarations.
fn cli_catalogue() -> BTreeMap<&'static str, Vec<&'static str>> {
    catalogue_by_domain(
        domain_operations()
            .iter()
            .map(|entry| (entry.domain, entry.operations.to_vec())),
        "CLI",
    )
}

/// Builds the daemon catalogue while rejecting duplicate domain declarations.
fn daemon_catalogue() -> BTreeMap<&'static str, Vec<&'static str>> {
    catalogue_by_domain(
        weaverd::test_support::routing_catalogue()
            .iter()
            .map(|(domain, operations)| (*domain, operations.to_vec())),
        "daemon",
    )
}

/// Collects one source catalogue and fails closed when a domain repeats.
fn catalogue_by_domain(
    entries: impl IntoIterator<Item = (&'static str, Vec<&'static str>)>,
    source: &str,
) -> BTreeMap<&'static str, Vec<&'static str>> {
    let mut catalogue = BTreeMap::new();
    for (domain, operations) in entries {
        assert!(
            catalogue.insert(domain, operations).is_none(),
            "{source} catalogue repeats domain {domain:?}",
        );
    }
    catalogue
}

#[test]
#[should_panic(expected = "test catalogue repeats domain \"observe\"")]
fn duplicate_catalogue_domains_are_rejected() {
    let _ = catalogue_by_domain(
        [
            ("observe", vec!["get-definition"]),
            ("observe", vec!["get-card"]),
        ],
        "test",
    );
}

#[test]
fn catalogue_agreement() {
    assert_eq!(
        cli_catalogue(),
        daemon_catalogue(),
        "CLI discoverability must describe exactly the daemon's known operations",
    );
}
