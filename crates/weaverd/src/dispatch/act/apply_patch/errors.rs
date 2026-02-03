//! Error types for apply-patch parsing and application.

use serde::Serialize;

use crate::dispatch::act::apply_patch::types::FilePath;

#[derive(Debug)]
pub(crate) enum ApplyPatchError {
    EmptyPatch,
    BinaryPatch,
    MissingDiffHeader,
    InvalidDiffHeader { line: String },
    MissingSearchReplace { path: FilePath },
    MissingHunk { path: FilePath },
    UnclosedSearchBlock { path: FilePath },
    UnclosedReplaceBlock { path: FilePath },
    InvalidPath { path: FilePath, reason: String },
    FileNotFound { path: FilePath },
    FileAlreadyExists { path: FilePath },
    DeleteMissing { path: FilePath },
    SearchBlockNotFound { path: FilePath, block_index: usize },
}

impl ApplyPatchError {
    pub(crate) fn exit_status(&self) -> i32 {
        1
    }

    pub(crate) fn to_json(&self) -> Result<String, serde_json::Error> {
        let details = ApplyPatchErrorDetails {
            message: self.message(),
            path: self.path().map(str::to_string),
            operation: self.operation().map(str::to_string),
        };
        let envelope = ApplyPatchErrorEnvelope {
            status: "error",
            kind: "ApplyPatchError",
            details,
        };
        serde_json::to_string(&envelope)
    }

    fn message(&self) -> String {
        match self {
            Self::EmptyPatch => String::from("patch input was empty"),
            Self::BinaryPatch => String::from("patch contains binary data"),
            Self::MissingDiffHeader => String::from("patch is missing diff headers"),
            Self::InvalidDiffHeader { line } => format!("invalid diff header: {line}"),
            Self::MissingSearchReplace { .. } => {
                String::from("modify operation missing SEARCH/REPLACE blocks")
            }
            Self::MissingHunk { .. } => String::from("create operation missing diff hunk content"),
            Self::UnclosedSearchBlock { .. } => {
                String::from("SEARCH block was not closed before end of patch")
            }
            Self::UnclosedReplaceBlock { .. } => {
                String::from("REPLACE block was not closed before end of patch")
            }
            Self::InvalidPath { reason, .. } => format!("invalid path: {reason}"),
            Self::FileNotFound { .. } => String::from("target file does not exist"),
            Self::FileAlreadyExists { .. } => String::from("target file already exists"),
            Self::DeleteMissing { .. } => String::from("delete target does not exist"),
            Self::SearchBlockNotFound { block_index, .. } => {
                format!("SEARCH block {block_index} did not match")
            }
        }
    }

    fn path(&self) -> Option<&str> {
        match self {
            Self::MissingSearchReplace { path }
            | Self::MissingHunk { path }
            | Self::UnclosedSearchBlock { path }
            | Self::UnclosedReplaceBlock { path }
            | Self::InvalidPath { path, .. }
            | Self::FileNotFound { path }
            | Self::FileAlreadyExists { path }
            | Self::DeleteMissing { path }
            | Self::SearchBlockNotFound { path, .. } => Some(path.as_str()),
            Self::EmptyPatch
            | Self::BinaryPatch
            | Self::MissingDiffHeader
            | Self::InvalidDiffHeader { .. } => None,
        }
    }

    fn operation(&self) -> Option<&'static str> {
        match self {
            Self::MissingSearchReplace { .. }
            | Self::UnclosedSearchBlock { .. }
            | Self::UnclosedReplaceBlock { .. }
            | Self::SearchBlockNotFound { .. } => Some("modify"),
            Self::MissingHunk { .. } | Self::FileAlreadyExists { .. } => Some("create"),
            Self::DeleteMissing { .. } => Some("delete"),
            Self::InvalidPath { .. }
            | Self::FileNotFound { .. }
            | Self::EmptyPatch
            | Self::BinaryPatch
            | Self::MissingDiffHeader
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
