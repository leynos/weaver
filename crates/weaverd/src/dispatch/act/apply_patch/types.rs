//! Types used to model apply-patch operations.

/// Line ending style inferred from patch or file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineEnding {
    /// Line feed (`\n`).
    Lf,
    /// Carriage return + line feed (`\r\n`).
    CrLf,
}

/// Search/replace block for modify operations.
#[derive(Debug, Clone)]
pub(crate) struct SearchReplaceBlock {
    pub(crate) search: String,
    pub(crate) replace: String,
}

/// Parsed patch operation.
#[derive(Debug, Clone)]
pub(crate) enum PatchOperation {
    Modify {
        path: String,
        blocks: Vec<SearchReplaceBlock>,
    },
    Create {
        path: String,
        content: String,
    },
    Delete {
        path: String,
    },
}
