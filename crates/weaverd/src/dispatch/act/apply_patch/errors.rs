//! Error types for apply-patch parsing and application.

use serde::Serialize;
use thiserror::Error;

use crate::dispatch::act::apply_patch::types::FilePath;

/// Failures raised while parsing or applying an apply-patch request.
#[derive(Debug, Error)]
pub(crate) enum ApplyPatchError {
    /// The patch input was empty after trimming whitespace.
    #[error("patch input was empty")]
    EmptyPatch,
    /// The patch contained NUL bytes, indicating binary content.
    #[error("patch contains binary data")]
    BinaryPatch,
    /// No `diff --git` header was found in the patch.
    #[error("patch is missing diff headers")]
    MissingDiffHeader,
    /// Parsing succeeded but produced no file changes.
    #[error("patch parsed successfully but produced no changes")]
    EmptyTransaction,
    /// A diff header line could not be parsed.
    #[error("invalid diff header: {line}")]
    InvalidDiffHeader {
        /// Offending header line, with trailing line endings removed.
        line: String,
    },
    /// A modify operation contained no `SEARCH`/`REPLACE` blocks.
    #[error("modify operation missing SEARCH/REPLACE blocks")]
    MissingSearchReplace {
        /// Target file path for the operation.
        path: FilePath,
    },
    /// A create operation contained no diff hunk content.
    #[error("create operation missing diff hunk content")]
    MissingHunk {
        /// Target file path for the operation.
        path: FilePath,
    },
    /// A `SEARCH` block was not closed before the end of the patch.
    #[error("SEARCH block was not closed before end of patch")]
    UnclosedSearchBlock {
        /// Target file path for the operation.
        path: FilePath,
    },
    /// A `REPLACE` block was not closed before the end of the patch.
    #[error("REPLACE block was not closed before end of patch")]
    UnclosedReplaceBlock {
        /// Target file path for the operation.
        path: FilePath,
    },
    /// The patch path failed workspace validation.
    #[error("invalid path: {reason}")]
    InvalidPath {
        /// Path supplied by the patch.
        path: FilePath,
        /// Human-readable reason the path was rejected.
        reason: String,
    },
    /// A modify target does not exist in the workspace.
    #[error("target file does not exist")]
    FileNotFound {
        /// Target file path for the operation.
        path: FilePath,
    },
    /// A create target already exists in the workspace.
    #[error("target file already exists")]
    FileAlreadyExists {
        /// Target file path for the operation.
        path: FilePath,
    },
    /// A delete target does not exist in the workspace.
    #[error("delete target does not exist")]
    DeleteMissing {
        /// Target file path for the operation.
        path: FilePath,
    },
    /// A `SEARCH` block did not match the remaining content.
    #[error("SEARCH block {block_index} did not match")]
    SearchBlockNotFound {
        /// Target file path for the operation.
        path: FilePath,
        /// One-based ordinal of the failing block.
        block_index: usize,
    },
    /// An I/O operation failed while reading or writing a target.
    #[error("I/O error for {path}: {message} ({kind})")]
    Io {
        /// Target file path for the operation.
        path: FilePath,
        /// Underlying I/O error kind.
        kind: std::io::ErrorKind,
        /// Rendered I/O error message.
        message: String,
    },
}

impl ApplyPatchError {
    /// Returns the daemon exit status used for apply-patch failures.
    pub(crate) const fn exit_status() -> i32 { 1 }

    /// Serialises the error as the JSON envelope written to stderr.
    ///
    /// # Errors
    ///
    /// Returns a [`serde_json::Error`] when the envelope cannot be serialised.
    pub(crate) fn to_json(&self) -> Result<String, serde_json::Error> {
        let details = ApplyPatchErrorDetails {
            message: self.to_string(),
            path: self.path(),
            operation: self.operation().map(str::to_owned),
        };
        let envelope = ApplyPatchErrorEnvelope {
            status: "error",
            kind: "ApplyPatchError",
            details,
        };
        serde_json::to_string(&envelope)
    }

    /// Returns the target path carried by the error, if the error names one.
    fn path(&self) -> Option<String> {
        match self {
            Self::MissingSearchReplace { path }
            | Self::MissingHunk { path }
            | Self::UnclosedSearchBlock { path }
            | Self::UnclosedReplaceBlock { path }
            | Self::InvalidPath { path, .. }
            | Self::FileNotFound { path }
            | Self::FileAlreadyExists { path }
            | Self::DeleteMissing { path }
            | Self::SearchBlockNotFound { path, .. }
            | Self::Io { path, .. } => Some(path.clone().into_string()),
            Self::EmptyPatch
            | Self::BinaryPatch
            | Self::MissingDiffHeader
            | Self::EmptyTransaction
            | Self::InvalidDiffHeader { .. } => None,
        }
    }

    /// Returns the patch operation kind implied by the error, if it is specific.
    const fn operation(&self) -> Option<&'static str> {
        match self {
            Self::MissingSearchReplace { .. }
            | Self::UnclosedSearchBlock { .. }
            | Self::UnclosedReplaceBlock { .. }
            | Self::SearchBlockNotFound { .. }
            | Self::FileNotFound { .. } => Some("modify"),
            Self::MissingHunk { .. } | Self::FileAlreadyExists { .. } => Some("create"),
            Self::DeleteMissing { .. } => Some("delete"),
            Self::InvalidPath { .. }
            | Self::Io { .. }
            | Self::EmptyPatch
            | Self::BinaryPatch
            | Self::MissingDiffHeader
            | Self::EmptyTransaction
            | Self::InvalidDiffHeader { .. } => None,
        }
    }
}

/// Wire envelope for an [`ApplyPatchError`] rendered as JSON.
#[derive(Debug, Serialize)]
struct ApplyPatchErrorEnvelope {
    /// Serialized status string, always `"error"`.
    status: &'static str,
    /// Error discriminator, serialized under the `"type"` key.
    #[serde(rename = "type")]
    kind: &'static str,
    /// Structured error details.
    details: ApplyPatchErrorDetails,
}

/// Structured details carried inside [`ApplyPatchErrorEnvelope`].
#[derive(Debug, Serialize)]
struct ApplyPatchErrorDetails {
    /// Human-readable error message.
    message: String,
    /// Target path when the error relates to a single file.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    /// Patch operation kind (`modify`, `create`, or `delete`) when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<String>,
}
