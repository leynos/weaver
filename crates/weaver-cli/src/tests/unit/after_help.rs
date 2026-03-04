//! Tests for the after-help domains-and-operations catalogue.
//!
//! Verifies that `weaver --help` includes a catalogue listing all three
//! domains and every CLI-supported operation, and that the static clap
//! text and Fluent resources remain synchronised.

use clap::CommandFactory;
use ortho_config::{FluentLocalizer, NoOpLocalizer};

use crate::cli::Cli;
use crate::localizer::WEAVER_EN_US;
use crate::localizer::after_help::render_after_help;

/// All operations that must appear in the after-help catalogue.
/// Sourced from `DomainRoutingContext` in
/// `crates/weaverd/src/dispatch/router.rs`.
const ALL_OPERATIONS: &[&str] = &[
    "get-definition",
    "find-references",
    "grep",
    "diagnostics",
    "call-hierarchy",
    "rename-symbol",
    "apply-edits",
    "apply-patch",
    "apply-rewrite",
    "refactor",
    "syntax",
];

/// All domain names that must appear in the after-help catalogue.
const ALL_DOMAINS: &[&str] = &["observe", "act", "verify"];

fn assert_catalogue_complete(text: &str) {
    for domain in ALL_DOMAINS {
        assert!(
            text.contains(domain),
            "after-help missing domain {domain:?}"
        );
    }
    for operation in ALL_OPERATIONS {
        assert!(
            text.contains(operation),
            "after-help missing operation {operation:?}"
        );
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
