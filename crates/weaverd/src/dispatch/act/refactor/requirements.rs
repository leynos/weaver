//! Canonical operator-facing requirements for `act refactor`.
//!
//! This module keeps the required flags, supported provider names, supported
//! refactorings, and actionable error text aligned from one source of truth.

use weaver_plugins::CapabilityId;

use crate::dispatch::errors::DispatchError;

use super::manifests::built_in_provider_names;

const REQUIRED_FLAGS: &[&str] = &[
    "--provider <plugin>",
    "--refactoring <operation>",
    "--file <path>",
];
const NEXT_COMMAND: &str = concat!(
    "weaver act refactor --provider rope --refactoring rename ",
    "--file path/to/file.py offset=1 new_name=renamed_symbol"
);

struct SupportedRefactoring {
    user_facing: &'static str,
    capability_operation: &'static str,
    capability: CapabilityId,
}

macro_rules! supported_refactoring_catalogue {
    (
        $(
            {
                user_facing: $user_facing:expr,
                capability_operation: $capability_operation:expr,
                capability: $capability:expr
            }
        ),+ $(,)?
    ) => {
        const SUPPORTED_REFACTORINGS: &[SupportedRefactoring] = &[
            $(
                SupportedRefactoring {
                    user_facing: $user_facing,
                    capability_operation: $capability_operation,
                    capability: $capability,
                },
            )+
        ];

        const SUPPORTED_REFACTORING_NAMES: &[&str] = &[$($user_facing),+];
    };
}

supported_refactoring_catalogue!({
    user_facing: "rename",
    capability_operation: "rename-symbol",
    capability: CapabilityId::RenameSymbol
});

pub(crate) fn supported_provider_names() -> &'static [&'static str] {
    built_in_provider_names()
}

pub(crate) fn supported_refactoring_names() -> &'static [&'static str] {
    SUPPORTED_REFACTORING_NAMES
}

pub(crate) fn validate_provider(provider: &str) -> Result<(), DispatchError> {
    validate_value("provider", supported_provider_names(), provider)
}

pub(crate) fn validate_refactoring(refactoring: &str) -> Result<(), DispatchError> {
    validate_value("refactoring", supported_refactoring_names(), refactoring)
}

pub(crate) fn effective_operation(refactoring: &str) -> Result<&'static str, DispatchError> {
    SUPPORTED_REFACTORINGS
        .iter()
        .find(|supported| supported.user_facing == refactoring)
        .map(|supported| supported.capability_operation)
        .ok_or_else(|| invalid_supported_value("refactoring", refactoring))
}

pub(crate) fn capability_for_operation(operation: &str) -> Result<CapabilityId, DispatchError> {
    SUPPORTED_REFACTORINGS
        .iter()
        .find(|supported| supported.capability_operation == operation)
        .map(|supported| supported.capability)
        .ok_or_else(|| {
            DispatchError::invalid_arguments(format!(
                "act refactor does not support capability resolution for '{operation}' (only 'rename-symbol' is currently implemented)"
            ))
        })
}

fn validate_value(kind: &str, supported: &[&str], value: &str) -> Result<(), DispatchError> {
    if supported.contains(&value) {
        Ok(())
    } else {
        Err(invalid_supported_value(kind, value))
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
    let providers = supported_provider_names();
    let refactorings = supported_refactoring_names();
    format!(
        "Valid alternatives:\n  - Providers: {}\n  - Refactorings: {}\n\nNext command:\n  {}",
        providers.join(", "),
        refactorings.join(", "),
        NEXT_COMMAND
    )
}

fn invalid_supported_value(kind: &str, value: &str) -> DispatchError {
    DispatchError::invalid_arguments(format!(
        "act refactor does not support {kind} '{value}'\n\n{}",
        guidance_lines()
    ))
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
        capability_for_operation, effective_operation, missing_requirements_error,
        supported_provider_names, supported_refactoring_names, validate_provider,
        validate_refactoring,
    };
    use crate::dispatch::errors::DispatchError;
    use weaver_plugins::CapabilityId;

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

    #[test]
    fn refactoring_table_drives_operation_mapping() {
        assert_eq!(
            effective_operation("rename").expect("supported"),
            "rename-symbol"
        );
        assert_eq!(
            capability_for_operation("rename-symbol").expect("supported"),
            CapabilityId::RenameSymbol
        );
    }
}
