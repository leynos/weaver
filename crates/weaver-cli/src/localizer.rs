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

/// Resolves a single help message through the localizer.
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

/// After-help catalogue definitions: `(fluent_id, english_fallback)`.
///
/// These constants must match `locales/en-US/messages.ftl`; the
/// `after_help_fluent_and_fallback_are_identical` test guards against drift.
/// The authoritative operation list lives in
/// `crates/weaverd/src/dispatch/router.rs` (`DomainRoutingContext`).
#[cfg(test)]
pub(crate) mod after_help {
    use ortho_config::Localizer;

    pub(super) const HEADER: (&str, &str) = ("weaver-after-help-header", "Domains and operations:");
    pub(super) const OBSERVE_HEADING: (&str, &str) = (
        "weaver-after-help-observe-heading",
        "observe \u{2014} Query code structure and relationships",
    );
    pub(super) const OBSERVE_OPS_1: (&str, &str) = (
        "weaver-after-help-observe-ops-1",
        "get-definition    find-references    grep",
    );
    pub(super) const OBSERVE_OPS_2: (&str, &str) = (
        "weaver-after-help-observe-ops-2",
        "diagnostics       call-hierarchy",
    );
    pub(super) const ACT_HEADING: (&str, &str) = (
        "weaver-after-help-act-heading",
        "act \u{2014} Perform code modifications",
    );
    pub(super) const ACT_OPS_1: (&str, &str) = (
        "weaver-after-help-act-ops-1",
        "rename-symbol     apply-edits        apply-patch",
    );
    pub(super) const ACT_OPS_2: (&str, &str) =
        ("weaver-after-help-act-ops-2", "apply-rewrite     refactor");
    pub(super) const VERIFY_HEADING: (&str, &str) = (
        "weaver-after-help-verify-heading",
        "verify \u{2014} Validate code correctness",
    );
    pub(super) const VERIFY_OPS: (&str, &str) =
        ("weaver-after-help-verify-ops", "diagnostics       syntax");

    /// Renders the after-help domains-and-operations catalogue.
    ///
    /// Each line is resolved through the localizer with a hardcoded English
    /// fallback. Used by tests to verify Fluent and static `concat!`
    /// consistency.
    pub(crate) fn render_after_help(localizer: &dyn Localizer) -> String {
        let header = super::msg(localizer, &HEADER);
        let obs_h = super::msg(localizer, &OBSERVE_HEADING);
        let obs_1 = super::msg(localizer, &OBSERVE_OPS_1);
        let obs_2 = super::msg(localizer, &OBSERVE_OPS_2);
        let act_h = super::msg(localizer, &ACT_HEADING);
        let act_1 = super::msg(localizer, &ACT_OPS_1);
        let act_2 = super::msg(localizer, &ACT_OPS_2);
        let ver_h = super::msg(localizer, &VERIFY_HEADING);
        let ver_o = super::msg(localizer, &VERIFY_OPS);
        format!(
            "{header}\n\n  {obs_h}\n    {obs_1}\n    {obs_2}\n\n\
             \x20 {act_h}\n    {act_1}\n    {act_2}\n\n\
             \x20 {ver_h}\n    {ver_o}"
        )
    }
}
