//! Tests for the after-help domains-and-operations catalogue.
//!
//! Verifies that `weaver --help` includes a catalogue listing all three
//! domains and every CLI-supported operation, and that the static clap
//! text and Fluent resources remain synchronized.

use clap::CommandFactory;
use ortho_config::{FluentLocalizer, NoOpLocalizer};

use crate::{
    cli::Cli,
    discoverability::{DOMAIN_OPERATIONS, fluent_entries::render_after_help},
    localizer::WEAVER_EN_US,
};

/// Splits the catalogue text into domain sections (separated by blank lines)
/// and verifies that each operation appears in the section belonging to its
/// domain. This catches false positives from operations like `diagnostics`
/// that appear in multiple domains.
fn assert_catalogue_complete(text: &str) {
    // Split into sections on blank lines. Each section after the header
    // starts with a domain heading (e.g. "  observe — …").
    let sections: Vec<&str> = text.split("\n\n").collect();
    for (domain, _, operations) in DOMAIN_OPERATIONS {
        let section = sections
            .iter()
            .find(|s| s.contains(domain))
            .unwrap_or_else(|| panic!("after-help missing domain {domain:?}"));
        for op in *operations {
            assert!(
                section.contains(op),
                "after-help: operation {op:?} not found under domain {domain:?}"
            );
        }
    }
}

#[test]
fn render_after_help_with_noop_contains_all_domains_and_operations() {
    let text = render_after_help(&NoOpLocalizer);
    assert_catalogue_complete(&text);
}

#[test]
fn render_after_help_with_fluent_contains_all_domains_and_operations() {
    let localizer = FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
        .expect("embedded Fluent catalogue must parse");
    let text = render_after_help(&localizer);
    assert_catalogue_complete(&text);
}

#[test]
fn after_help_fluent_and_fallback_are_identical() {
    let fluent_localizer = FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
        .expect("embedded Fluent catalogue must parse");
    let fluent_output = render_after_help(&fluent_localizer);
    let fallback_output = render_after_help(&NoOpLocalizer);
    assert_eq!(
        fluent_output, fallback_output,
        "Fluent catalogue and fallback strings have diverged"
    );
}

#[test]
fn clap_after_help_matches_fluent_render() {
    let command = Cli::command();
    let clap_after_help = command
        .get_after_help()
        .expect("Cli must have after_help set")
        .to_string();
    let rendered = render_after_help(&NoOpLocalizer);
    assert_eq!(
        clap_after_help.trim(),
        rendered.trim(),
        "static after_help in cli.rs and render_after_help() have diverged"
    );
}

#[test]
fn after_help_contains_header() {
    let text = render_after_help(&NoOpLocalizer);
    assert!(
        text.contains("Domains and operations:"),
        "after-help missing header"
    );
}
