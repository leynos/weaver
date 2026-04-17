//! Canonical operator-facing requirements for `act refactor`.
//!
//! This module keeps the required flags, supported provider names, supported
//! refactorings, and actionable error text aligned from one source of truth.

use crate::dispatch::errors::DispatchError;

const SUPPORTED_PROVIDERS: &[&str] = &["rope", "rust-analyzer"];
const SUPPORTED_REFACTORINGS: &[&str] = &["rename"];
const REQUIRED_FLAGS: &[&str] = &[
    "--provider <plugin>",
    "--refactoring <operation>",
    "--file <path>",
];
const NEXT_COMMAND: &str = concat!(
    "weaver act refactor --provider rope --refactoring rename ",
    "--file path/to/file.py offset=1 new_name=renamed_symbol"
);

pub(crate) fn supported_provider_names() -> &'static [&'static str] {
    SUPPORTED_PROVIDERS
}

pub(crate) fn supported_refactoring_names() -> &'static [&'static str] {
    SUPPORTED_REFACTORINGS
}

pub(crate) fn validate_provider(provider: &str) -> Result<(), DispatchError> {
    validate_value("provider", supported_provider_names(), provider)
}

pub(crate) fn validate_refactoring(refactoring: &str) -> Result<(), DispatchError> {
    validate_value("refactoring", supported_refactoring_names(), refactoring)
}

fn validate_value(kind: &str, supported: &[&str], value: &str) -> Result<(), DispatchError> {
    if supported.contains(&value) {
        Ok(())
    } else {
        Err(DispatchError::invalid_arguments(format!(
            "act refactor does not support {kind} '{value}'\n\n{}",
            guidance_lines()
        )))
    }
}

pub(crate) fn missing_requirements_error() -> DispatchError {
    DispatchError::invalid_arguments(format!(
        "act refactor requires {}\n\n{}",
        format_required_flags(),
        guidance_lines()
    ))
}

fn guidance_lines() -> String {
    format!(
        "Valid alternatives:\n  - Providers: {}\n  - Refactorings: {}\n\nNext command:\n  {}",
        supported_provider_names().join(", "),
        supported_refactoring_names().join(", "),
        NEXT_COMMAND
    )
}

fn format_required_flags() -> String {
    let [first, second, third] = REQUIRED_FLAGS else {
        unreachable!("required act refactor flag list must stay in sync");
    };
    format!("{first}, {second}, and {third}")
}

#[cfg(test)]
mod tests {
    use super::{
        missing_requirements_error, supported_provider_names, supported_refactoring_names,
        validate_provider, validate_refactoring,
    };
    use crate::dispatch::errors::DispatchError;

    fn invalid_arguments_message(error: DispatchError) -> String {
        match error {
            DispatchError::InvalidArguments { message } => message,
            other => panic!("expected invalid arguments error, got: {other:?}"),
        }
    }

    #[test]
    fn missing_requirements_error_lists_full_contract() {
        let message = invalid_arguments_message(missing_requirements_error());

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

    #[test]
    fn invalid_provider_error_lists_supported_values() {
        let message =
            invalid_arguments_message(validate_provider("missing-provider").expect_err("invalid"));

        assert!(message.contains("does not support provider 'missing-provider'"));
        assert!(message.contains("Providers: rope, rust-analyzer"));
    }

    #[test]
    fn invalid_refactoring_error_lists_supported_values() {
        let message =
            invalid_arguments_message(validate_refactoring("extract-method").expect_err("invalid"));

        assert!(message.contains("does not support refactoring 'extract-method'"));
        assert!(message.contains("Refactorings: rename"));
    }

    #[test]
    fn supported_lists_stay_canonical() {
        assert_eq!(supported_provider_names(), ["rope", "rust-analyzer"]);
        assert_eq!(supported_refactoring_names(), ["rename"]);
    }
}
