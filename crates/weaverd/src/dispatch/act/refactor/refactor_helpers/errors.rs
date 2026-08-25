//! Dispatch-error extractors shared by the `act refactor` unit tests.

use crate::dispatch::errors::DispatchError;

/// Extracts the operator-facing message from an `InvalidArguments` error.
///
/// The mismatch is returned rather than panicked so the failure surfaces at the
/// calling test instead of inside this helper.
pub(crate) fn invalid_arguments_message(error: DispatchError) -> Result<String, String> {
    match error {
        DispatchError::InvalidArguments { message } => Ok(message),
        other => Err(format!("expected invalid arguments error, got: {other:?}")),
    }
}
