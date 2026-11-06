//! Argument parsing for `act refactor`.
//! This module keeps CLI-token parsing separate from routing and plugin
//! execution so the handler can stay within the repository's file-size limit.
use super::{
    metrics::PositionMetrics,
    positions::{LineCol, parse_line_col},
    requirements::{missing_requirements_error, validate_provider, validate_refactoring},
};
use crate::dispatch::errors::DispatchError;
/// Parsed `act refactor` arguments after validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefactorArgs {
    /// Plugin that supplies the refactoring.
    pub(crate) provider: String,
    /// Refactoring operation requested from the plugin.
    pub(crate) refactoring: String,
    /// Path of the file to refactor.
    pub(crate) file: String,
    /// One-based line and column of the refactoring target.
    pub(crate) position: Option<LineCol>,
    /// Trailing `KEY=VALUE` arguments passed through to the plugin.
    pub(crate) extra: Vec<String>,
}
/// Recognised long-form command-line flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flag {
    /// `--provider` flag naming the plugin.
    Provider,
    /// `--refactoring` flag naming the operation.
    Refactoring,
    /// `--file` flag naming the target file.
    File,
    /// `--position` flag holding a `LINE:COL` coordinate.
    Position,
}
impl Flag {
    /// Parses a CLI token into a known flag, if it matches one.
    fn parse(s: &str) -> Option<Self> {
        match s {
            "--provider" => Some(Self::Provider),
            "--refactoring" => Some(Self::Refactoring),
            "--file" => Some(Self::File),
            "--position" => Some(Self::Position),
            _ => None,
        }
    }
    /// Returns the canonical CLI spelling of the flag.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Provider => "--provider",
            Self::Refactoring => "--refactoring",
            Self::File => "--file",
            Self::Position => "--position",
        }
    }
}
/// Accumulates flags and trailing arguments before validation.
#[derive(Default)]
struct RefactorArgsBuilder {
    /// Value captured for `--provider`.
    provider: Option<String>,
    /// Value captured for `--refactoring`.
    refactoring: Option<String>,
    /// Value captured for `--file`.
    file: Option<String>,
    /// Value captured for `--position`.
    position: Option<LineCol>,
    /// Trailing `KEY=VALUE` arguments in source order.
    extra: Vec<String>,
}
impl RefactorArgsBuilder {
    /// Finalises the builder, rejecting missing or invalid arguments.
    ///
    /// # Errors
    ///
    /// Returns an invalid-arguments error when a required flag is missing, the
    /// position contract is violated, or a provider or refactoring value is
    /// unsupported.
    fn build(self) -> Result<RefactorArgs, DispatchError> {
        let Some(provider) = self.provider else {
            return Err(missing_requirements_error());
        };
        let Some(refactoring) = self.refactoring else {
            return Err(missing_requirements_error());
        };
        let Some(file) = self.file else {
            return Err(missing_requirements_error());
        };
        let position = self.position;
        validate_position_contract(position, &self.extra)?;
        validate_trailing_extra_arguments(&self.extra)?;
        validate_provider(&provider)?;
        validate_refactoring(&refactoring)?;
        Ok(RefactorArgs {
            provider,
            refactoring,
            file,
            position,
            extra: self.extra,
        })
    }
}
/// Enforces that exactly one of `--position` or `offset=` is supplied.
///
/// # Errors
///
/// Returns an invalid-arguments error when neither form is present or both are
/// present together.
fn validate_position_contract(
    position: Option<LineCol>,
    extra: &[String],
) -> Result<(), DispatchError> {
    if position.is_none() && !has_deprecated_offset_argument(extra) {
        return Err(missing_requirements_error());
    }
    if position.is_some() && has_deprecated_offset_argument(extra) {
        return Err(DispatchError::invalid_arguments(
            "refactor rename must not supply both '--position' and deprecated 'offset='",
        ));
    }
    Ok(())
}
/// Rejects trailing arguments that are not `KEY=VALUE` pairs.
///
/// # Errors
///
/// Returns an invalid-arguments error naming every rejected trailing token.
fn validate_trailing_extra_arguments(extra: &[String]) -> Result<(), DispatchError> {
    let invalid_extra_arguments: Vec<&str> = extra
        .iter()
        .map(String::as_str)
        .filter(|argument| !is_valid_extra_argument(argument))
        .collect();
    if invalid_extra_arguments.is_empty() {
        return Ok(());
    }
    let offending_tokens = invalid_extra_arguments
        .iter()
        .map(|token| format!("'{token}'"))
        .collect::<Vec<_>>()
        .join(", ");
    Err(DispatchError::invalid_arguments(format!(
        "act refactor only accepts trailing KEY=VALUE arguments; invalid trailing arguments: \
         {offending_tokens}. Use only --provider <plugin>, --refactoring <operation>, --file \
         <path>, --position <line:col>, and trailing KEY=VALUE arguments"
    )))
}
/// Parses and validates the argument list for `act refactor`.
///
/// # Errors
///
/// Returns an invalid-arguments error when a flag is malformed, a required flag
/// is missing, or a value fails provider, refactoring, or position validation.
pub(crate) fn parse_refactor_args(
    arguments: &[String],
    metrics: &dyn PositionMetrics,
) -> Result<RefactorArgs, DispatchError> {
    let mut builder = RefactorArgsBuilder::default();
    let mut iter = arguments.iter();
    while let Some(arg) = iter.next() {
        apply_flag(arg, &mut iter, &mut builder, metrics)?;
    }
    builder.build()
}
/// Consumes one CLI token, storing a flag value or a trailing argument.
///
/// # Errors
///
/// Returns an invalid-arguments error when a flag value is missing, a flag
/// appears after trailing arguments, or position parsing fails.
fn apply_flag<'a>(
    arg: &str,
    iter: &mut impl Iterator<Item = &'a String>,
    builder: &mut RefactorArgsBuilder,
    metrics: &dyn PositionMetrics,
) -> Result<(), DispatchError> {
    let Some(flag) = Flag::parse(arg) else {
        builder.extra.push(arg.to_owned());
        return Ok(());
    };
    if !builder.extra.is_empty() {
        return Err(DispatchError::invalid_arguments(format!(
            "act refactor only accepts trailing KEY=VALUE arguments after all flags; interleaved \
             KEY=VALUE arguments cannot appear before flag '{}'",
            flag.as_str()
        )));
    }
    match flag {
        Flag::Provider => builder.provider = Some(parse_flag_value(flag, iter)?),
        Flag::Refactoring => builder.refactoring = Some(parse_flag_value(flag, iter)?),
        Flag::File => builder.file = Some(parse_flag_value(flag, iter)?),
        Flag::Position => builder.position = Some(parse_position_flag(flag, iter, metrics)?),
    }
    Ok(())
}
/// Parses the `LINE:COL` value following `--position`.
///
/// # Errors
///
/// Returns an invalid-arguments error when the value is missing or is not a
/// valid one-based line and column pair.
fn parse_position_flag<'a>(
    flag: Flag,
    iter: &mut impl Iterator<Item = &'a String>,
    metrics: &dyn PositionMetrics,
) -> Result<LineCol, DispatchError> {
    let value = parse_flag_value(flag, iter)?;
    let position = parse_line_col(&value).inspect_err(|_error| {
        metrics.increment_parse_error();
    })?;
    tracing::debug!(position = value, "stored valid act refactor position flag");
    Ok(position)
}
/// Reads the value token following `flag`, rejecting a following flag.
///
/// # Errors
///
/// Returns an invalid-arguments error when the value is absent or itself looks
/// like a flag.
fn parse_flag_value<'a>(
    flag: Flag,
    iter: &mut impl Iterator<Item = &'a String>,
) -> Result<String, DispatchError> {
    let flag_str = flag.as_str();
    let error = || DispatchError::invalid_arguments(format!("{flag_str} requires a value"));
    let value = iter.next().ok_or_else(error)?;
    if value.starts_with("--") {
        return Err(error());
    }
    Ok(value.clone())
}
/// Reports whether `argument` is a non-empty-key `KEY=VALUE` pair.
fn is_valid_extra_argument(argument: &str) -> bool {
    if argument.starts_with("--") {
        return false;
    }
    let Some((key, _value)) = argument.split_once('=') else {
        return false;
    };
    !key.is_empty()
}
/// Reports whether any trailing argument uses the deprecated `offset=` form.
fn has_deprecated_offset_argument(arguments: &[String]) -> bool {
    arguments
        .iter()
        .any(|argument| argument.starts_with("offset="))
}

#[cfg(test)]
mod tests;
