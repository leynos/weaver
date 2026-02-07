//! Error types for apply-patch parsing and application.

use serde::Serialize;
use thiserror::Error;

use crate::dispatch::act::apply_patch::types::FilePath;

#[derive(Debug, Error)]
pub(crate) enum ApplyPatchError {
    #[error("patch input was empty")]
    EmptyPatch,
    #[error("patch contains binary data")]
    BinaryPatch,
    #[error("patch is missing diff headers")]
    MissingDiffHeader,
    #[error("patch parsed successfully but produced no changes")]
    EmptyTransaction,
    #[error("invalid diff header: {line}")]
    InvalidDiffHeader { line: String },
    #[error("modify operation missing SEARCH/REPLACE blocks")]
    MissingSearchReplace { path: FilePath },
    #[error("create operation missing diff hunk content")]
    MissingHunk { path: FilePath },
    #[error("SEARCH block was not closed before end of patch")]
    UnclosedSearchBlock { path: FilePath },
    #[error("REPLACE block was not closed before end of patch")]
    UnclosedReplaceBlock { path: FilePath },
    #[error("invalid path: {reason}")]
    InvalidPath { path: FilePath, reason: String },
    #[error("target file does not exist")]
    FileNotFound { path: FilePath },
    #[error("target file already exists")]
    FileAlreadyExists { path: FilePath },
    #[error("delete target does not exist")]
    DeleteMissing { path: FilePath },
    #[error("SEARCH block {block_index} did not match")]
    SearchBlockNotFound { path: FilePath, block_index: usize },
    #[error("I/O error for {path}: {message} ({kind})")]
    Io {
        path: FilePath,
        kind: std::io::ErrorKind,
        message: String,
    },
}

impl ApplyPatchError {
    pub(crate) const fn exit_status(&self) -> i32 {
        1
    }

    pub(crate) fn to_json(&self) -> Result<String, serde_json::Error> {
        let details = ApplyPatchErrorDetails {
            message: self.to_string(),
            path: self.path(),
            operation: self.operation().map(str::to_string),
        };
        let envelope = ApplyPatchErrorEnvelope {
            status: "error",
            kind: "ApplyPatchError",
            details,
        };
        serde_json::to_string(&envelope)
    }

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

    fn operation(&self) -> Option<&'static str> {
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

#[derive(Debug, Serialize)]
struct ApplyPatchErrorEnvelope {
    status: &'static str,
    #[serde(rename = "type")]
    kind: &'static str,
    details: ApplyPatchErrorDetails,
}

#[derive(Debug, Serialize)]
struct ApplyPatchErrorDetails {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<String>,
}
