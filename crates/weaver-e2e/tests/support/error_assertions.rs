//! Assertions for snapshot operations that are expected to fail.
//!
//! The pattern and rewrite suites both need "this input must be rejected, and
//! the message must explain why". Keeping the shape here means the failure
//! diagnostics stay consistent between them.

use std::fmt::Display;

/// Returns the rendered error from `outcome`, or `None` when it succeeded.
///
/// This is a pure query so the assertion below can be read at a glance.
pub(crate) fn error_message<T, E: Display>(outcome: Result<T, E>) -> Option<String> {
    outcome.err().map(|error| error.to_string())
}

/// Checks `outcome` failed with a message mentioning `expected_substring`.
///
/// `subject` names the rejected input so a failure identifies the offending
/// case without the reader having to count `#[case]` attributes.
///
/// # Errors
/// Returns a description of the mismatch when `outcome` succeeded or when its
/// error message does not mention `expected_substring`. Callers in `#[test]`
/// bodies turn that into a panic, keeping the panic out of shared support.
pub(crate) fn assert_error_mentions<T, E: Display>(
    outcome: Result<T, E>,
    subject: &str,
    expected_substring: &str,
) -> Result<(), String> {
    let Some(message) = error_message(outcome) else {
        return Err(format!(
            "expected {subject} to be rejected, but it succeeded"
        ));
    };
    if message.contains(expected_substring) {
        Ok(())
    } else {
        Err(format!(
            "error for {subject} should mention '{expected_substring}': {message}"
        ))
    }
}
