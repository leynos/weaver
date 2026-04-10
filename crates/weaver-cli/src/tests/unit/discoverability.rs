//! Unit tests for CLI domain discoverability guidance helpers.

use ortho_config::FluentLocalizer;
use rstest::rstest;

use crate::discoverability::{
    KnownDomain, bounded_levenshtein, suggestion_for_unknown_domain,
    write_missing_operation_guidance, write_unknown_domain_guidance,
};
use crate::localizer::WEAVER_EN_US;

#[rstest]
#[case("observe", "observe", Some(0))]
#[case("obsrve", "observe", Some(1))]
#[case("obsve", "observe", Some(2))]
#[case("bogus", "observe", None)]
fn bounded_levenshtein_respects_threshold(
    #[case] left: &str,
    #[case] right: &str,
    #[case] expected: Option<usize>,
) {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    assert_eq!(bounded_levenshtein(&left_chars, &right_chars, 2), expected);
}

#[rstest]
#[case("obsrve", Some(KnownDomain::Observe))]
#[case("bogus", None)]
#[case("obsve", Some(KnownDomain::Observe))]
fn suggestion_for_unknown_domain_cases(#[case] input: &str, #[case] expected: Option<KnownDomain>) {
    assert_eq!(suggestion_for_unknown_domain(input), expected);
}

fn fluent_localizer() -> FluentLocalizer {
    FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
        .expect("embedded Fluent catalogue must parse")
}

fn assert_single_error_prefix(output: &str) {
    assert!(
        output.contains("error: "),
        "output should contain an error prefix"
    );
    assert!(
        !output.contains("error: error:"),
        "output should not contain a duplicated error prefix: {output}"
    );
}

fn assert_three_part_guidance(
    output: &str,
    error_text: &str,
    alternatives_text: &str,
    next_command: &str,
) {
    assert_single_error_prefix(output);
    let error_pos = output.find(error_text).expect("expected error text");
    let alternatives_pos = output
        .find(alternatives_text)
        .expect("expected alternatives text");
    let next_command_line = format!("Next command:\n  {next_command}");
    let next_command_pos = output
        .find(&next_command_line)
        .expect("expected exact Next command block");

    assert!(
        error_pos < alternatives_pos,
        "error line must precede alternatives block: {output}"
    );
    assert!(
        alternatives_pos < next_command_pos,
        "alternatives block must precede Next command: {output}"
    );
}

#[test]
fn fluent_missing_operation_guidance_has_single_error_prefix() {
    let localizer = fluent_localizer();
    let mut output = Vec::new();

    let emitted = write_missing_operation_guidance(&mut output, &localizer, KnownDomain::Observe)
        .expect("guidance write must succeed");

    assert!(emitted, "known domain should emit guidance");
    let output = String::from_utf8(output).expect("guidance must be valid UTF-8");
    assert_three_part_guidance(
        &output,
        "error: operation required for domain 'observe'",
        "Available operations:\n  get-definition",
        "weaver observe get-definition --help",
    );
}

#[test]
fn fluent_unknown_domain_guidance_has_single_error_prefix() {
    let localizer = fluent_localizer();
    let mut output = Vec::new();

    let emitted = write_unknown_domain_guidance(&mut output, &localizer, "unknown-domain")
        .expect("guidance write must succeed");

    assert!(emitted, "unknown domain should emit guidance");
    let output = String::from_utf8(output).expect("guidance must be valid UTF-8");
    assert_three_part_guidance(
        &output,
        "error: unknown domain 'unknown-domain'",
        "Valid domains: observe, act, verify",
        "weaver --help",
    );
}

#[test]
fn fluent_unknown_domain_guidance_uses_unique_suggestion_when_available() {
    let localizer = fluent_localizer();
    let mut output = Vec::new();

    let emitted = write_unknown_domain_guidance(&mut output, &localizer, "obsrve")
        .expect("guidance write must succeed");

    assert!(emitted, "unknown domain should emit guidance");
    let output = String::from_utf8(output).expect("guidance must be valid UTF-8");
    assert!(
        output.contains("Did you mean 'observe'?"),
        "unique suggestion should be rendered: {output}"
    );
    assert_three_part_guidance(
        &output,
        "error: unknown domain 'obsrve'",
        "Did you mean 'observe'?",
        "weaver observe get-definition --help",
    );
}
