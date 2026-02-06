//! Types used to model apply-patch operations.

/// Line ending style inferred from patch or file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineEnding {
    /// Line feed (`\n`).
    Lf,
    /// Carriage return + line feed (`\r\n`).
    CrLf,
}

macro_rules! string_newtype {
    ($vis:vis struct $name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone)]
        $vis struct $name(String);

        impl $name {
            $vis fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            $vis fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

string_newtype!(pub(crate) struct PatchText, "Raw patch input text.");

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

string_newtype!(pub(crate) struct DiffHeaderLine, "Diff header line from a patch stream.");

string_newtype!(pub(crate) struct FilePath, "Path for a patch operation target.");

impl FilePath {
    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for FilePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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

string_newtype!(
    pub(crate) struct SearchPattern,
    "Search block content extracted from a patch."
);

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
