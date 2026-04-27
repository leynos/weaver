//! Post-parse (pre-configuration) guidance paths that operate on an already
//! parsed `Cli` and should exit before configuration loading.

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use ortho_config::Localizer;

use crate::actionable_guidance;
use crate::config::ConfigArgumentSplit;
use crate::discoverability::{
    KnownDomain, should_emit_domain_guidance, write_missing_operation_guidance,
    write_unknown_domain_guidance,
};
use crate::{AppError, Cli};

static PREFLIGHT_GUIDANCE_EMISSIONS: AtomicU64 = AtomicU64::new(0);

enum DomainGuidanceToEmit {
    MissingOperation(KnownDomain),
    UnknownDomain,
}

/// Handles preflight exits after argv splitting and before configuration
/// loading, returning `Ok(())` when no early exit is needed or an [`AppError`]
/// that exits before daemon startup.
pub(crate) fn handle_preflight<ErrWriter: Write>(
    cli: &Cli,
    split: &ConfigArgumentSplit,
    stderr: &mut ErrWriter,
    localizer: &dyn Localizer,
) -> Result<(), AppError> {
    if cli.is_bare_invocation() && !split.has_config_flags() {
        tracing::debug!("emitting bare invocation guidance");
        actionable_guidance::write_bare_invocation_guidance(stderr, localizer)
            .map_err(AppError::EmitBareHelp)?;
        PREFLIGHT_GUIDANCE_EMISSIONS.fetch_add(1, Ordering::Relaxed);
        return Err(AppError::BareInvocation);
    }
    if should_emit_domain_guidance(cli) {
        let raw_domain = cli.domain.as_deref().map(str::trim).unwrap_or_default();
        tracing::debug!(domain = raw_domain, "evaluating preflight domain guidance");
        emit_domain_guidance(cli, stderr, localizer, raw_domain)?;
    }
    Ok(())
}

/// Dispatches domain-specific guidance to stderr for unknown domains or missing
/// operations.
fn emit_domain_guidance<ErrWriter: Write>(
    cli: &Cli,
    stderr: &mut ErrWriter,
    localizer: &dyn Localizer,
    raw_domain: &str,
) -> Result<(), AppError> {
    let Some(guidance) = domain_guidance(cli, raw_domain) else {
        return Ok(());
    };

    let written = match guidance {
        DomainGuidanceToEmit::MissingOperation(domain) => {
            tracing::debug!(?domain, "emitting missing operation guidance");
            write_missing_operation_guidance(stderr, localizer, domain)
        }
        DomainGuidanceToEmit::UnknownDomain => {
            tracing::debug!(domain = raw_domain, "emitting unknown domain guidance");
            write_unknown_domain_guidance(stderr, localizer, raw_domain)
        }
    }
    .map_err(|error| {
        tracing::warn!(domain = raw_domain, error = %error, "failed to emit preflight guidance");
        AppError::EmitGuidance(error)
    })?;
    if written {
        PREFLIGHT_GUIDANCE_EMISSIONS.fetch_add(1, Ordering::Relaxed);
    }
    preflight_result(written)
}

fn domain_guidance(cli: &Cli, raw_domain: &str) -> Option<DomainGuidanceToEmit> {
    let operation_is_missing = cli
        .operation
        .as_deref()
        .is_none_or(|op| op.trim().is_empty());

    let Some(domain) = KnownDomain::try_parse(raw_domain) else {
        return Some(DomainGuidanceToEmit::UnknownDomain);
    };

    if operation_is_missing {
        Some(DomainGuidanceToEmit::MissingOperation(domain))
    } else {
        None
    }
}

/// Converts the `written` flag into either `Ok(())` or
/// `Err(AppError::PreflightGuidance)`.
fn preflight_result(written: bool) -> Result<(), AppError> {
    if written {
        Err(AppError::PreflightGuidance)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::handle_preflight;
    use crate::config::ConfigArgumentSplit;
    use crate::localizer::WEAVER_EN_US;
    use crate::{AppError, Cli, OutputFormat};
    use ortho_config::{FluentLocalizer, Localizer};
    use rstest::{fixture, rstest};
    use std::ffi::OsString;
    use std::io;
    use std::io::Write;

    enum ExpectedPreflightResult {
        Continue,
        BareInvocation,
        PreflightGuidance,
    }

    struct PreflightScenario {
        domain: Option<&'static str>,
        operation: Option<&'static str>,
        has_config_flags: bool,
        expected_result: ExpectedPreflightResult,
        expected_stderr_substring: Option<&'static str>,
    }

    enum WriteFailureScenario {
        BareInvocation,
        DomainGuidance,
    }

    impl WriteFailureScenario {
        fn domain(&self) -> Option<&str> {
            match self {
                WriteFailureScenario::BareInvocation => None,
                WriteFailureScenario::DomainGuidance => Some("unknown-domain"),
            }
        }

        fn operation(&self) -> Option<&str> {
            match self {
                WriteFailureScenario::BareInvocation => None,
                WriteFailureScenario::DomainGuidance => Some("status"),
            }
        }
    }

    struct PreflightContext {
        localizer: Box<dyn Localizer>,
        stderr: Vec<u8>,
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "write failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn test_localizer() -> impl Localizer {
        FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
            .expect("embedded Fluent catalogue must parse")
    }

    #[fixture]
    fn preflight_context() -> PreflightContext {
        PreflightContext {
            localizer: Box::new(test_localizer()),
            stderr: Vec::new(),
        }
    }

    fn cli(domain: Option<&str>, operation: Option<&str>) -> Cli {
        Cli {
            capabilities: false,
            output: OutputFormat::Auto,
            command: None,
            domain: domain.map(str::to_string),
            operation: operation.map(str::to_string),
            arguments: Vec::new(),
        }
    }

    fn split(has_config_flags: bool) -> ConfigArgumentSplit {
        let mut config_arguments = vec![OsString::from("weaver")];
        if has_config_flags {
            config_arguments.push(OsString::from("--config-path"));
            config_arguments.push(OsString::from("weaver.toml"));
        }
        ConfigArgumentSplit {
            config_arguments,
            command_start: 1,
        }
    }

    #[rstest]
    #[case(PreflightScenario {
        domain: None,
        operation: None,
        has_config_flags: false,
        expected_result: ExpectedPreflightResult::BareInvocation,
        expected_stderr_substring: Some("Usage: weaver"),
    })]
    #[case(PreflightScenario {
        domain: None,
        operation: None,
        has_config_flags: true,
        expected_result: ExpectedPreflightResult::Continue,
        expected_stderr_substring: None,
    })]
    #[case(PreflightScenario {
        domain: Some("unknown-domain"),
        operation: Some("status"),
        has_config_flags: false,
        expected_result: ExpectedPreflightResult::PreflightGuidance,
        expected_stderr_substring: Some("unknown domain 'unknown-domain'"),
    })]
    #[case(PreflightScenario {
        domain: Some("observe"),
        operation: None,
        has_config_flags: false,
        expected_result: ExpectedPreflightResult::PreflightGuidance,
        expected_stderr_substring: Some("operation required for domain 'observe'"),
    })]
    #[case(PreflightScenario {
        domain: Some("observe"),
        operation: Some("get-definition"),
        has_config_flags: false,
        expected_result: ExpectedPreflightResult::Continue,
        expected_stderr_substring: None,
    })]
    fn preflight_guidance_paths_match_expected_contract(
        #[case] scenario: PreflightScenario,
        preflight_context: PreflightContext,
    ) {
        let PreflightContext {
            localizer,
            mut stderr,
        } = preflight_context;

        let result = handle_preflight(
            &cli(scenario.domain, scenario.operation),
            &split(scenario.has_config_flags),
            &mut stderr,
            localizer.as_ref(),
        );

        match scenario.expected_result {
            ExpectedPreflightResult::Continue => assert!(matches!(result, Ok(()))),
            ExpectedPreflightResult::BareInvocation => {
                assert!(matches!(result, Err(AppError::BareInvocation)));
            }
            ExpectedPreflightResult::PreflightGuidance => {
                assert!(matches!(result, Err(AppError::PreflightGuidance)));
            }
        }

        if let Some(needle) = scenario.expected_stderr_substring {
            let stderr = String::from_utf8(stderr).expect("guidance must be valid UTF-8");
            assert!(stderr.contains(needle), "stderr should contain {needle:?}");
        } else {
            assert!(
                stderr.is_empty(),
                "invocation without guidance should not emit stderr"
            );
        }
    }

    #[fixture]
    fn failing_writer() -> FailingWriter {
        FailingWriter
    }

    #[rstest]
    #[case(WriteFailureScenario::BareInvocation)]
    #[case(WriteFailureScenario::DomainGuidance)]
    fn preflight_guidance_propagates_write_failure(
        #[case] scenario: WriteFailureScenario,
        mut failing_writer: FailingWriter,
    ) {
        let localizer = test_localizer();

        let result = handle_preflight(
            &cli(scenario.domain(), scenario.operation()),
            &split(false),
            &mut failing_writer,
            &localizer,
        );

        match scenario {
            WriteFailureScenario::BareInvocation => {
                assert!(
                    matches!(result, Err(AppError::EmitBareHelp(error)) if error.kind() == io::ErrorKind::BrokenPipe)
                );
            }
            WriteFailureScenario::DomainGuidance => {
                assert!(
                    matches!(result, Err(AppError::EmitGuidance(error)) if error.kind() == io::ErrorKind::BrokenPipe)
                );
            }
        }
    }
}
