//! Argument parsing for `act refactor`.
//!
//! This module keeps CLI-token parsing separate from routing and plugin
//! execution so the handler can stay within the repository's file-size limit.

use crate::dispatch::errors::DispatchError;

use super::requirements::{missing_requirements_error, validate_provider, validate_refactoring};

/// Parsed `act refactor` arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefactorArgs {
    pub(crate) provider: String,
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
    /// Finalizes the builder and validates the operator-facing contract.
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
        let invalid_extra_arguments: Vec<&str> = self
            .extra
            .iter()
            .map(String::as_str)
            .filter(|extra| !is_valid_extra_argument(extra))
            .collect();
        if !invalid_extra_arguments.is_empty() {
            let offending_tokens = invalid_extra_arguments
                .iter()
                .map(|token| format!("'{token}'"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(DispatchError::invalid_arguments(format!(
                "act refactor only accepts trailing KEY=VALUE arguments; invalid trailing arguments: {offending_tokens}. Use only --provider <plugin>, --refactoring <operation>, --file <path>, and trailing KEY=VALUE arguments"
            )));
        }

        validate_provider(&provider)?;
        validate_refactoring(&refactoring)?;

        Ok(RefactorArgs {
            provider,
            refactoring,
            file,
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
        "--provider" => builder.provider = Some(parse_flag_value(arg, iter)?),
        "--refactoring" => builder.refactoring = Some(parse_flag_value(arg, iter)?),
        "--file" => builder.file = Some(parse_flag_value(arg, iter)?),
        other => builder.extra.push(other.to_owned()),
    }
    Ok(())
}

fn parse_flag_value<'a>(
    flag: &str,
    iter: &mut impl Iterator<Item = &'a String>,
) -> Result<String, DispatchError> {
    let error = || DispatchError::invalid_arguments(format!("{flag} requires a value"));
    let value = iter.next().ok_or_else(error)?;
    if value.starts_with("--") {
        return Err(error());
    }
    Ok(value.clone())
}

fn is_valid_extra_argument(argument: &str) -> bool {
    if argument.starts_with("--") {
        return false;
    }

    let Some((key, _value)) = argument.split_once('=') else {
        return false;
    };
    !key.is_empty()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::parse_refactor_args;
    use crate::dispatch::errors::DispatchError;

    fn invalid_arguments_message(error: DispatchError) -> String {
        match error {
            DispatchError::InvalidArguments { message } => message,
            other => panic!("expected invalid arguments error, got: {other:?}"),
        }
    }

    #[track_caller]
    fn assert_invalid_args_contains(args: Vec<String>, expected_substrings: &[&str]) {
        let message =
            invalid_arguments_message(parse_refactor_args(&args).expect_err("parse should fail"));
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
            String::from("offset"),
            String::from("=woven"),
            String::from("new_name"),
        ],
        vec!["invalid trailing arguments: 'offset', '=woven', 'new_name'", "trailing KEY=VALUE arguments"],
    )]
    fn invalid_arguments_are_rejected(
        #[case] args: Vec<String>,
        #[case] expected_substrings: Vec<&str>,
    ) {
        assert_invalid_args_contains(args, &expected_substrings);
    }

    #[test]
    fn parses_complete_argument_set() {
        let args = vec![
            String::from("--provider"),
            String::from("rope"),
            String::from("--refactoring"),
            String::from("rename"),
            String::from("--file"),
            String::from("src/main.py"),
        ];

        let parsed = parse_refactor_args(&args).expect("parse succeeds");
        assert_eq!(parsed.provider, "rope");
        assert_eq!(parsed.refactoring, "rename");
        assert_eq!(parsed.file, "src/main.py");
    }

    #[rstest]
    #[case::no_arguments(vec![])]
    #[case::missing_provider(vec![
        String::from("--refactoring"),
        String::from("rename"),
        String::from("--file"),
        String::from("src/main.py"),
    ])]
    #[case::missing_refactoring(vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--file"),
        String::from("src/main.py"),
    ])]
    #[case::missing_file(vec![
        String::from("--provider"),
        String::from("rope"),
        String::from("--refactoring"),
        String::from("rename"),
    ])]
    fn missing_required_flags_report_full_contract(#[case] args: Vec<String>) {
        let message =
            invalid_arguments_message(parse_refactor_args(&args).expect_err("parse should fail"));

        for required in [
            "--provider <plugin>",
            "--refactoring <operation>",
            "--file <path>",
        ] {
            assert!(
                message.contains(required),
                "missing '{required}' from: {message}"
            );
        }
        assert!(message.contains("Providers: rope, rust-analyzer"));
        assert!(message.contains("Refactorings: rename"));
        assert!(message.contains("Next command:"));
    }

}
