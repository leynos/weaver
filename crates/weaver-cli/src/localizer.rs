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

/// Bare-help message definitions: `(fluent_id, english_fallback)`.
///
/// The fallback values must match `locales/en-US/messages.ftl`; the
/// `fluent_and_fallback_outputs_are_identical` test guards against drift.
mod bare_help {
    pub(super) const USAGE: (&str, &str) = (
        "weaver-bare-help-usage",
        "Usage: weaver <DOMAIN> <OPERATION> [ARG]...",
    );
    pub(super) const HEADER: (&str, &str) = ("weaver-bare-help-header", "Domains:");
    pub(super) const OBSERVE: (&str, &str) = (
        "weaver-bare-help-domain-observe",
        "observe   Query code structure and relationships",
    );
    pub(super) const ACT: (&str, &str) = (
        "weaver-bare-help-domain-act",
        "act       Perform code modifications",
    );
    pub(super) const VERIFY: (&str, &str) = (
        "weaver-bare-help-domain-verify",
        "verify    Validate code correctness",
    );
    pub(super) const POINTER: (&str, &str) = (
        "weaver-bare-help-pointer",
        "Run 'weaver --help' for more information.",
    );
}

/// Resolves a single bare-help message through the localizer.
fn msg(localizer: &dyn Localizer, entry: &(&str, &str)) -> String {
    localizer.message(entry.0, None, entry.1)
}

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
    use bare_help::{ACT, HEADER, OBSERVE, POINTER, USAGE, VERIFY};
    let usage = msg(localizer, &USAGE);
    let header = msg(localizer, &HEADER);
    let observe = msg(localizer, &OBSERVE);
    let act = msg(localizer, &ACT);
    let verify = msg(localizer, &VERIFY);
    let pointer = msg(localizer, &POINTER);
    write!(
        writer,
        "{usage}\n\n{header}\n  {observe}\n  {act}\n  {verify}\n\n{pointer}\n",
    )
}
