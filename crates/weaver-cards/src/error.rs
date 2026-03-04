//! Error types for the `observe get-card` request pipeline.
//!
//! These errors represent problems that occur during request parsing,
//! before any symbol extraction takes place.

/// Errors that can occur during `get-card` request parsing.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GetCardError {
    /// A required argument is missing.
    #[error("missing required argument: {flag}")]
    MissingArgument {
        /// The flag that was expected.
        flag: String,
    },
    /// An argument value is malformed.
    #[error("invalid argument value for {flag}: {message}")]
    InvalidValue {
        /// The flag whose value was invalid.
        flag: String,
        /// Description of the problem.
        message: String,
    },
    /// An unknown argument was provided.
    #[error("unknown argument: {argument}")]
    UnknownArgument {
        /// The unrecognised argument.
        argument: String,
    },
}
