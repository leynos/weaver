//! Pre-clap guidance paths that should exit before configuration loading.

use std::io::Write;

use ortho_config::Localizer;

use crate::actionable_guidance;
use crate::config::ConfigArgumentSplit;
use crate::discoverability::{
    KnownDomain, should_emit_domain_guidance, write_missing_operation_guidance,
    write_unknown_domain_guidance,
};
use crate::{AppError, Cli};

/// Handles preflight exits after argv splitting and before configuration
/// loading, returning `Ok(())` when execution should continue or an
/// [`AppError`] that exits before daemon startup.
pub(crate) fn handle_preflight<ErrWriter: Write>(
    cli: &Cli,
    split: &ConfigArgumentSplit,
    stderr: &mut ErrWriter,
    localizer: &dyn Localizer,
) -> Result<(), AppError> {
    if cli.is_bare_invocation() && !split.has_config_flags() {
        actionable_guidance::write_bare_invocation_guidance(stderr, localizer)
            .map_err(AppError::EmitBareHelp)?;
        return Err(AppError::BareInvocation);
    }
    if should_emit_domain_guidance(cli) {
        let raw_domain = cli.domain.as_deref().map(str::trim).unwrap_or_default();
        emit_domain_guidance(cli, stderr, localizer, raw_domain)?;
    }
    Ok(())
}

/// Emits domain-specific guidance to stderr for unknown domains or missing
/// operations.
fn emit_domain_guidance<ErrWriter: Write>(
    cli: &Cli,
    stderr: &mut ErrWriter,
    localizer: &dyn Localizer,
    raw_domain: &str,
) -> Result<(), AppError> {
    let operation_is_missing = cli
        .operation
        .as_deref()
        .is_none_or(|op| op.trim().is_empty());

    match KnownDomain::try_parse(raw_domain) {
        Some(domain) if operation_is_missing => preflight_result(
            write_missing_operation_guidance(stderr, localizer, domain)
                .map_err(AppError::EmitGuidance)?,
        ),
        Some(_) => Ok(()),
        None => preflight_result(
            write_unknown_domain_guidance(stderr, localizer, raw_domain)
                .map_err(AppError::EmitGuidance)?,
        ),
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
    use std::ffi::OsString;

    fn test_localizer() -> impl Localizer {
        FluentLocalizer::with_en_us_defaults([WEAVER_EN_US])
            .expect("embedded Fluent catalogue must parse")
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

    #[test]
    fn bare_invocation_without_config_flags_returns_bare_invocation() {
        let localizer = test_localizer();
        let mut stderr = Vec::new();

        let result = handle_preflight(&cli(None, None), &split(false), &mut stderr, &localizer);

        assert!(matches!(result, Err(AppError::BareInvocation)));
        assert!(!stderr.is_empty(), "bare invocation should emit guidance");
    }

    #[test]
    fn bare_invocation_with_config_flags_continues() {
        let localizer = test_localizer();
        let mut stderr = Vec::new();

        let result = handle_preflight(&cli(None, None), &split(true), &mut stderr, &localizer);

        assert!(matches!(result, Ok(())));
        assert!(
            stderr.is_empty(),
            "config-backed bare invocation should not emit guidance"
        );
    }

    #[test]
    fn unknown_domain_returns_preflight_guidance() {
        let localizer = test_localizer();
        let mut stderr = Vec::new();

        let result = handle_preflight(
            &cli(Some("unknown-domain"), Some("status")),
            &split(false),
            &mut stderr,
            &localizer,
        );

        assert!(matches!(result, Err(AppError::PreflightGuidance)));
        let stderr = String::from_utf8(stderr).expect("guidance must be valid UTF-8");
        assert!(stderr.contains("unknown domain 'unknown-domain'"));
    }

    #[test]
    fn known_domain_without_operation_returns_preflight_guidance() {
        let localizer = test_localizer();
        let mut stderr = Vec::new();

        let result = handle_preflight(
            &cli(Some("observe"), None),
            &split(false),
            &mut stderr,
            &localizer,
        );

        assert!(matches!(result, Err(AppError::PreflightGuidance)));
        let stderr = String::from_utf8(stderr).expect("guidance must be valid UTF-8");
        assert!(stderr.contains("operation required for domain 'observe'"));
    }

    #[test]
    fn known_domain_with_operation_continues() {
        let localizer = test_localizer();
        let mut stderr = Vec::new();

        let result = handle_preflight(
            &cli(Some("observe"), Some("get-definition")),
            &split(false),
            &mut stderr,
            &localizer,
        );

        assert!(matches!(result, Ok(())));
        assert!(
            stderr.is_empty(),
            "complete invocation should not emit guidance"
        );
    }
}
