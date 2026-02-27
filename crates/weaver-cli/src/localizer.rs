//! Localization support for the Weaver CLI.
//!
//! Constructs a Fluent-backed localizer from embedded resources so
//! user-facing text can be translated without code changes.  Falls back
//! to [`NoOpLocalizer`] (hardcoded English) when the Fluent pipeline
//! fails.

use std::io::Write;

use ortho_config::{FluentLocalizer, Localizer, NoOpLocalizer};

/// Embedded en-US Fluent catalogue for the Weaver CLI.
pub(crate) static WEAVER_EN_US: &str = include_str!("../locales/en-US/messages.ftl");

/// Builds the application localizer.
///
/// Returns a [`FluentLocalizer`] loaded with the embedded en-US
/// catalogue.  Falls back to [`NoOpLocalizer`] on error so the CLI
/// never crashes due to a localization failure.
///
/// # Examples
///
/// ```rust,ignore
/// let loc = build_localizer();
/// let msg = loc.message("weaver-bare-help-usage", None, "fallback");
/// ```
pub(crate) fn build_localizer() -> Box<dyn Localizer> {
    match FluentLocalizer::with_en_us_defaults([WEAVER_EN_US]) {
        Ok(loc) => Box::new(loc),
        Err(_) => Box::new(NoOpLocalizer),
    }
}

/// Writes the bare-invocation help block to `writer`.
///
/// Each line is resolved through the localizer with a hardcoded English
/// fallback, so the output is correct even without Fluent resources.
///
/// # Errors
///
/// Returns [`std::io::Error`] if writing to the underlying stream fails.
///
/// # Examples
///
/// ```rust,ignore
/// let loc = ortho_config::NoOpLocalizer;
/// let mut buf = Vec::new();
/// write_bare_help(&mut buf, &loc).expect("write bare help");
/// assert!(String::from_utf8(buf).expect("valid UTF-8").contains("Usage:"));
/// ```
pub(crate) fn write_bare_help<W: Write>(
    writer: &mut W,
    localizer: &dyn Localizer,
) -> std::io::Result<()> {
    let usage = localizer.message(
        "weaver-bare-help-usage",
        None,
        "Usage: weaver <DOMAIN> <OPERATION> [ARG]...",
    );
    let header = localizer.message("weaver-bare-help-header", None, "Domains:");
    let observe = localizer.message(
        "weaver-bare-help-domain-observe",
        None,
        "observe   Query code structure and relationships",
    );
    let act = localizer.message(
        "weaver-bare-help-domain-act",
        None,
        "act       Perform code modifications",
    );
    let verify = localizer.message(
        "weaver-bare-help-domain-verify",
        None,
        "verify    Validate code correctness",
    );
    let pointer = localizer.message(
        "weaver-bare-help-pointer",
        None,
        "Run 'weaver --help' for more information.",
    );
    write!(
        writer,
        "{usage}\n\n{header}\n  {observe}\n  {act}\n  {verify}\n\n{pointer}\n",
    )
}
