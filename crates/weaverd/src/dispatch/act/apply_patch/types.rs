//! Types used to model apply-patch operations.

/// Line ending style inferred from patch or file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineEnding {
    /// Line feed (`\n`).
    Lf,
    /// Carriage return + line feed (`\r\n`).
    CrLf,
}

/// Raw patch input text.
#[derive(Debug, Clone)]
pub(crate) struct PatchText(String);

impl PatchText {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for PatchText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<String> for PatchText {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for PatchText {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// Diff header line from a patch stream.
#[derive(Debug, Clone)]
pub(crate) struct DiffHeaderLine(String);

impl DiffHeaderLine {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for DiffHeaderLine {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Path for a patch operation target.
#[derive(Debug, Clone)]
pub(crate) struct FilePath(String);

impl FilePath {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for FilePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Content of a file involved in patch operations.
#[derive(Debug, Clone)]
pub(crate) struct FileContent(String);

impl FileContent {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn into_string(self) -> String {
        self.0
    }

    pub(crate) fn replace_range(&mut self, range: std::ops::Range<usize>, replacement: &str) {
        self.0.replace_range(range, replacement);
    }
}

impl AsRef<str> for FileContent {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Search block content extracted from a patch.
#[derive(Debug, Clone)]
pub(crate) struct SearchPattern(String);

impl SearchPattern {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SearchPattern {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Search/replace block for modify operations.
#[derive(Debug, Clone)]
pub(crate) struct SearchReplaceBlock {
    pub(crate) search: SearchPattern,
    pub(crate) replace: String,
}

/// Parsed patch operation.
#[derive(Debug, Clone)]
pub(crate) enum PatchOperation {
    Modify {
        path: FilePath,
        blocks: Vec<SearchReplaceBlock>,
    },
    Create {
        path: FilePath,
        content: String,
    },
    Delete {
        path: FilePath,
    },
}
