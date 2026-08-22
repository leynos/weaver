//! Tests for the after-help domains-and-operations catalogue.
//!
//! Verifies that `weaver --help` includes a catalogue listing all three
//! domains and every CLI-supported operation from the canonical command tree.

use crate::{domain_operations, help};

/// Splits the catalogue text into domain sections (separated by blank lines)
/// and verifies that each operation appears in the section belonging to its
/// domain. This catches false positives from operations like `diagnostics`
/// that appear in multiple domains.
fn assert_catalogue_complete(text: &str) {
    // Split into sections on blank lines. Each section after the header
    // starts with a domain heading (e.g. "  observe — …").
    let sections: Vec<&str> = text.split("\n\n").collect();
    for entry in domain_operations() {
        let Some(section) = sections.iter().find(|s| s.contains(entry.domain)) else {
            panic!("after-help missing domain {:?}", entry.domain);
        };
        for op in entry.operations {
            assert!(
                section.contains(op),
                "after-help: operation {op:?} not found under domain {:?}",
                entry.domain,
            );
        }
    }
}

#[test]
fn clap_after_help_comes_from_the_canonical_command_tree() {
    let command = help::command();
    let clap_after_help = command
        .get_after_help()
        .expect("augmented command must have after_help set")
        .to_string();
    assert_catalogue_complete(&clap_after_help);
}

#[test]
fn after_help_contains_header() {
    let text = help::command()
        .get_after_help()
        .expect("augmented command must have after_help set")
        .to_string();
    assert!(
        text.contains("Domains and operations:"),
        "after-help missing header"
    );
}
