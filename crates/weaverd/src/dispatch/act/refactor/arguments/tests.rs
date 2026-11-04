//! Unit tests for act refactor argument parsing.
use rstest::rstest;

use super::{LineCol, parse_refactor_args};
use crate::dispatch::act::refactor::{
    metrics::NullPositionMetrics,
    refactor_helpers::errors::invalid_arguments_message,
};
fn args(tokens: &[&str]) -> Vec<String> { tokens.iter().copied().map(String::from).collect() }
/// Parses `args`, expecting rejection, and returns the operator message.
fn rejection_message(args: &[String]) -> Result<String, String> {
    let metrics = NullPositionMetrics;
    let error = parse_refactor_args(args, &metrics)
        .err()
        .ok_or_else(|| String::from("parse should fail"))?;
    invalid_arguments_message(error)
}
#[track_caller]
fn assert_message_contains(message: &str, expected_substrings: &[&str]) {
    for expected in expected_substrings {
        assert!(
            message.contains(expected),
            "missing {expected:?} from: {message}"
        );
    }
}
#[rstest]
#[case::missing_flag_value(
    vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
    ],
    vec!["requires a value"],
)]
#[case::flag_as_value(
    vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("--provider"),
    ],
    vec!["requires a value"],
)]
#[case::unsupported_provider(
    vec![
        String::from("--provider"),
        String::from("missing-provider"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("src/main.py"),
        String::from("--position"),
        String::from("1:1"),
    ],
    vec!["does not support provider 'missing-provider'", "Providers: rope, rust-analyzer"],
)]
#[case::unsupported_refactoring(
    vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("extract-method"),
        String::from("--file"),
        String::from("src/main.py"),
        String::from("--position"),
        String::from("1:1"),
    ],
    vec!["does not support refactoring 'extract-method'", "Refactorings: rename"],
)]
#[case::unexpected_top_level_flag(
    vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("src/main.py"),
        String::from("--position"),
        String::from("1:1"),
        String::from("--bogus"),
    ],
    vec!["invalid trailing arguments: '--bogus'", "trailing KEY=VALUE arguments"],
)]
#[case::malformed_trailing_arguments(
    vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("src/main.py"),
        String::from("--position"),
        String::from("1:1"),
        String::from("offset"),
        String::from("=woven"),
        String::from("new_name"),
    ],
    vec!["invalid trailing arguments: 'offset', '=woven', 'new_name'", "trailing KEY=VALUE arguments"],
)]
#[case::interleaved_trailing_arguments(
    vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("new_name=woven"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("src/main.py"),
    ],
    vec!["interleaved KEY=VALUE arguments", "before flag '--refactoring'"],
)]
#[case::missing_position_value(
    vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("src/main.py"),
        String::from("--position"),
    ],
    vec!["--position requires a value"],
)]
#[case::invalid_position_format(
    vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("src/main.py"),
        String::from("--position"),
        String::from("1"),
    ],
    vec!["position must be LINE:COL"],
)]
#[case::zero_position_column(
    vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("src/main.py"),
        String::from("--position"),
        String::from("1:0"),
    ],
    vec!["column number must be >= 1"],
)]
fn invalid_arguments_are_rejected(
    #[case] args: Vec<String>,
    #[case] expected_substrings: Vec<&str>,
) {
    let message = rejection_message(&args).expect("parse should reject these arguments");

    assert_message_contains(&message, &expected_substrings);
}
#[test]
fn parses_complete_argument_set() {
    let args = args(&[
        "--provider",
        "rope",
        "--refactoring",
        "rename",
        "--file",
        "src/main.py",
        "--position",
        "1:5",
    ]);

    let metrics = NullPositionMetrics;
    let parsed = parse_refactor_args(&args, &metrics).expect("parse succeeds");
    assert_eq!(parsed.provider, "rope");
    assert_eq!(parsed.refactoring, "rename");
    assert_eq!(parsed.file, "src/main.py");
    assert_eq!(parsed.position, Some(LineCol { line: 1, column: 5 }));
}
#[rstest]
#[case::no_arguments(Vec::new())]
#[case::missing_provider(args(&["--refactoring", "rename", "--file", "src/main.py"]))]
#[case::missing_refactoring(args(&["--provider", "rope", "--file", "src/main.py"]))]
#[case::missing_file(args(&["--provider", "rope", "--refactoring", "rename"]))]
#[case::missing_position(args(&[
    "--provider",
    "rope",
    "--refactoring",
    "rename",
    "--file",
    "src/main.py",
]))]
fn missing_required_flags_report_full_contract(#[case] args: Vec<String>) {
    let message = rejection_message(&args).expect("parse should reject incomplete arguments");

    assert_message_contains(
        &message,
        &[
            "--provider <plugin>",
            "--refactoring <operation>",
            "--file <path>",
            "--position <line:col>",
        ],
    );
    assert!(message.contains("Providers: rope, rust-analyzer"));
    assert!(message.contains("Refactorings: rename"));
    assert!(message.contains("Next command:"));
}
