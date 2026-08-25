//! Tests for contextual guidance when a domain is missing its operation.
//!
//! Verifies that known domains fail fast with actionable guidance before
//! configuration loading, while preserving the client-side-only UX path.

use std::{ffi::OsString, io::Cursor, process::ExitCode};

use rstest::rstest;
use weaver_config::Config;

use crate::{AppError, ConfigLoader, IoStreams, run_with_loader, tests::support};

struct PreflightOutput {
    exit: ExitCode,
    stdout: Vec<u8>,
    stderr: String,
}

fn run_with_panicking_loader(args: &[&str]) -> anyhow::Result<PreflightOutput> {
    let cli_args = std::iter::once("weaver")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect::<Vec<_>>();
    let (exit, stdout, stderr) = support::run_with_panicking_loader(cli_args);
    let stderr_text = String::from_utf8(stderr)?;

    Ok(PreflightOutput {
        exit,
        stdout,
        stderr: stderr_text,
    })
}

fn assert_preflight_failure(output: &PreflightOutput) {
    assert_eq!(output.exit, ExitCode::FAILURE);
    assert!(
        output.stdout.is_empty(),
        "guidance must not write to stdout"
    );
    assert_no_daemon_start_text(output);
}

fn assert_no_daemon_start_text(output: &PreflightOutput) {
    let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    let stderr = output.stderr.to_ascii_lowercase();

    for phrase in ["starting daemon", "daemon started"] {
        assert!(
            !stdout.contains(phrase),
            "stdout should not contain daemon-start text '{phrase}'"
        );
        assert!(
            !stderr.contains(phrase),
            "stderr should not contain daemon-start text '{phrase}'"
        );
    }
}

/// Expected shape of one flavour of preflight guidance: the error line it must
/// carry, plus the fragments that must and must not accompany it.
///
/// Describing each flavour as data keeps a single validation algorithm below,
/// rather than one near-identical predicate per flavour.
struct GuidanceTemplate {
    /// Names the flavour in failure messages, such as `unknown-domain guidance`.
    description: &'static str,
    error_line: String,
    required_fragments: &'static [&'static str],
    forbidden_fragments: &'static [&'static str],
}

/// Guidance expected when the domain itself is not recognised.
///
/// Operation guidance is forbidden because it would imply the domain was
/// valid.
fn unknown_domain_guidance_template(domain: &str) -> GuidanceTemplate {
    GuidanceTemplate {
        description: "unknown-domain guidance",
        error_line: format!("error: unknown domain '{domain}'"),
        required_fragments: &["Valid domains: observe, act, verify", "Next command:"],
        forbidden_fragments: &["Available operations:"],
    }
}

/// Guidance expected when the domain is known but the operation is missing.
///
/// Domain listings and suggestions are forbidden because the domain was
/// understood.
fn known_domain_operation_guidance_template(domain: &str) -> GuidanceTemplate {
    GuidanceTemplate {
        description: "known-domain missing-operation guidance",
        error_line: format!("error: operation required for domain '{domain}'"),
        required_fragments: &["Available operations:"],
        forbidden_fragments: &["Valid domains:", "Did you mean"],
    }
}

/// Checks `stderr` against `template`, reporting the first unmet expectation.
///
/// # Errors
/// Returns the failing expectation, named by the template description and
/// accompanied by the offending transcript.
fn guidance_matches(stderr: &str, template: GuidanceTemplate) -> anyhow::Result<()> {
    let GuidanceTemplate {
        description,
        error_line,
        required_fragments,
        forbidden_fragments,
    } = template;

    anyhow::ensure!(
        stderr.contains(&error_line),
        "{description} should report {error_line:?}\nstderr:\n{stderr}"
    );

    for &fragment in required_fragments {
        anyhow::ensure!(
            stderr.contains(fragment),
            "{description} should contain {fragment:?}\nstderr:\n{stderr}"
        );
    }

    for &fragment in forbidden_fragments {
        anyhow::ensure!(
            !stderr.contains(fragment),
            "{description} should not contain {fragment:?}\nstderr:\n{stderr}"
        );
    }

    Ok(())
}

/// Panic boundary: fails the calling test when `output` is not a preflight
/// failure carrying the guidance `template` describes.
fn assert_preflight_guidance(output: &PreflightOutput, template: GuidanceTemplate) {
    assert_preflight_failure(output);
    if let Err(error) = guidance_matches(&output.stderr, template) {
        panic!("preflight guidance must match the expected template: {error}");
    }
}

/// Verifies the unified three-part error template for known domain without operation.
/// Per roadmap 2.3.3: error, alternatives, Next command.
fn assert_three_part_template(
    output: &PreflightOutput,
    expected_alternatives: &str,
) -> anyhow::Result<()> {
    let stderr = &output.stderr;

    // Part 1: error line
    anyhow::ensure!(stderr.contains("error:"), "must have explicit error line");

    // Part 3: Next command line
    anyhow::ensure!(
        stderr.contains("Next command:"),
        "must include Next command line"
    );

    // Verify ordering
    let Some(error_pos) = stderr.find("error:") else {
        anyhow::bail!("error line must be present");
    };
    let Some(next_cmd_pos) = stderr.find("Next command:") else {
        anyhow::bail!("Next command line must be present");
    };
    anyhow::ensure!(
        error_pos < next_cmd_pos,
        "error line must come before Next command"
    );

    let alternatives_pos = stderr[error_pos..next_cmd_pos]
        .find(expected_alternatives)
        .map(|relative| error_pos + relative);
    anyhow::ensure!(
        alternatives_pos.is_some(),
        "alternatives block must contain '{expected_alternatives}' between error and Next command"
    );
    Ok(())
}

fn assert_no_domain_guidance(output: &PreflightOutput) {
    assert!(
        !output
            .stderr
            .contains("error: operation required for domain")
    );
    assert!(!output.stderr.contains("Available operations:"));
    assert!(
        !output
            .stderr
            .contains("Valid domains: observe, act, verify")
    );
}

#[test]
fn known_domain_without_operation_emits_contextual_guidance() {
    let output =
        run_with_panicking_loader(&["observe"]).expect("guidance command output must be UTF-8");

    assert_preflight_guidance(&output, known_domain_operation_guidance_template("observe"));
    assert!(output.stderr.contains("get-definition"));
    assert!(output.stderr.contains("get-card"));
    assert!(
        output
            .stderr
            .contains("weaver observe get-definition --help")
    );
    assert_three_part_template(&output, "Available operations:")
        .expect("known-domain guidance must follow the three-part template");
}

#[rstest]
#[case(
    &["unknown-domain"],
    "unknown-domain",
    &["Next command:\n  weaver --help"],
    &["weaver observe get-definition --help"]
)]
#[case(
    &["unknown-domain", "get-definition"],
    "unknown-domain",
    &["Next command:\n  weaver --help"],
    &["Waiting for daemon start..."]
)]
#[case(&["obsrve", "get-definition"], "obsrve", &["Did you mean 'observe'?"], &[])]
#[case(&["bogus", "get-definition"], "bogus", &[], &["Did you mean"])]
fn unknown_domain_preflight_guidance(
    #[case] args: &[&str],
    #[case] domain: &str,
    #[case] required_contains: &[&str],
    #[case] forbidden_contains: &[&str],
) {
    let output = run_with_panicking_loader(args).expect("guidance command output must be UTF-8");

    assert_preflight_guidance(&output, unknown_domain_guidance_template(domain));
    assert_three_part_template(&output, "Valid domains: observe, act, verify")
        .expect("unknown-domain guidance must follow the three-part template");

    if output.stderr.contains("Did you mean 'observe'?") {
        assert!(
            output
                .stderr
                .contains("Next command:\n  weaver observe get-definition --help"),
            "unknown domain with suggestion must include actionable suggested command"
        );
    } else {
        assert!(
            output.stderr.contains("Next command:\n  weaver --help"),
            "unknown domain without suggestions must fall back to generic help"
        );
    }

    for text in required_contains {
        assert!(
            output.stderr.contains(text),
            "stderr should contain '{text}'"
        );
    }

    for text in forbidden_contains {
        assert!(
            !output.stderr.contains(text),
            "stderr should not contain '{text}'"
        );
    }
}

#[test]
fn complete_command_still_reports_configuration_failures() {
    struct FailingLoader;

    impl ConfigLoader for FailingLoader {
        fn load(&self, _args: &[OsString]) -> Result<Config, AppError> {
            Err(AppError::MissingDomain)
        }
    }

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = Cursor::new(Vec::new());
    let mut io = IoStreams::new(&mut stdin, &mut stdout, &mut stderr, false);
    let exit = run_with_loader(
        vec![
            OsString::from("weaver"),
            OsString::from("observe"),
            OsString::from("get-definition"),
        ],
        &mut io,
        &FailingLoader,
    );

    let output = PreflightOutput {
        exit,
        stdout,
        stderr: String::from_utf8(stderr).expect("stderr utf8"),
    };
    assert_eq!(output.exit, ExitCode::FAILURE);
    assert!(output.stderr.contains("command domain"));
    assert_no_domain_guidance(&output);
}
