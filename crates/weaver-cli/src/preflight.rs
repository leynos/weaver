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

fn preflight_result(written: bool) -> Result<(), AppError> {
    if written {
        Err(AppError::PreflightGuidance)
    } else {
        Ok(())
    }
}
