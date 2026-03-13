//! Argument parsing for `act refactor`.
//!
//! This module keeps CLI-token parsing separate from routing and plugin
//! execution so the handler can stay within the repository's file-size limit.

use crate::dispatch::errors::DispatchError;

/// Parsed `act refactor` arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefactorArgs {
    pub(crate) provider: Option<String>,
    pub(crate) refactoring: String,
    pub(crate) file: String,
    pub(crate) extra: Vec<String>,
}

/// Accumulates parsed flag values during argument iteration.
#[derive(Default)]
struct RefactorArgsBuilder {
    provider: Option<String>,
    refactoring: Option<String>,
    file: Option<String>,
    extra: Vec<String>,
}

impl RefactorArgsBuilder {
    /// Finalizes the builder, requiring the non-optional flags.
    fn build(self) -> Result<RefactorArgs, DispatchError> {
        Ok(RefactorArgs {
            provider: self.provider,
            refactoring: self.refactoring.ok_or_else(|| {
                DispatchError::invalid_arguments("act refactor requires --refactoring <operation>")
            })?,
            file: self.file.ok_or_else(|| {
                DispatchError::invalid_arguments("act refactor requires --file <path>")
            })?,
            extra: self.extra,
        })
    }
}

/// Parses the raw daemon request arguments for `act refactor`.
///
/// # Errors
///
/// Returns [`DispatchError::InvalidArguments`] when a required flag is missing
/// or a flag that expects a following value does not receive one.
pub(crate) fn parse_refactor_args(arguments: &[String]) -> Result<RefactorArgs, DispatchError> {
    let mut builder = RefactorArgsBuilder::default();
    let mut iter = arguments.iter();

    while let Some(arg) = iter.next() {
        apply_flag(arg, &mut iter, &mut builder)?;
    }

    builder.build()
}

/// Classifies a single argument token, consuming the next token as the value
/// when the argument is a recognised flag.
fn apply_flag<'a>(
    arg: &str,
    iter: &mut impl Iterator<Item = &'a String>,
    builder: &mut RefactorArgsBuilder,
) -> Result<(), DispatchError> {
    match arg {
        "--provider" => {
            builder.provider =
                Some(iter.next().cloned().ok_or_else(|| {
                    DispatchError::invalid_arguments("--provider requires a value")
                })?);
        }
        "--refactoring" => {
            builder.refactoring = Some(iter.next().cloned().ok_or_else(|| {
                DispatchError::invalid_arguments("--refactoring requires a value")
            })?);
        }
        "--file" => {
            builder.file = Some(
                iter.next()
                    .cloned()
                    .ok_or_else(|| DispatchError::invalid_arguments("--file requires a value"))?,
            );
        }
        other => builder.extra.push(other.to_owned()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_refactor_args;

    #[test]
    fn provider_is_optional() {
        let args = vec![
            String::from("--refactoring"),
            String::from("rename"),
            String::from("--file"),
            String::from("src/main.py"),
        ];

        let parsed = parse_refactor_args(&args).expect("parse succeeds");
        assert_eq!(parsed.provider, None);
        assert_eq!(parsed.refactoring, "rename");
        assert_eq!(parsed.file, "src/main.py");
    }

    #[test]
    fn missing_file_is_rejected() {
        let args = vec![
            String::from("--refactoring"),
            String::from("rename"),
            String::from("--file"),
        ];

        let error = parse_refactor_args(&args).expect_err("parse should fail");
        assert!(matches!(
            error,
            crate::dispatch::errors::DispatchError::InvalidArguments { .. }
        ));
    }
}
