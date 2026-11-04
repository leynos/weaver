//! Types used to model apply-patch operations.

/// Line ending style inferred from patch or file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineEnding {
    /// Line feed (`\n`).
    Lf,
    /// Carriage return + line feed (`\r\n`).
    CrLf,
}

/// Declares a `String` newtype with `new` and `as_str` accessors.
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
    fn from(value: String) -> Self { Self(value) }
}

impl From<&str> for PatchText {
    fn from(value: &str) -> Self { Self(value.to_owned()) }
}

string_newtype!(pub(crate) struct DiffHeaderLine, "Diff header line from a patch stream.");

string_newtype!(pub(crate) struct FilePath, "Path for a patch operation target.");

impl FilePath {
    /// Consumes the wrapper and returns the underlying path text.
    pub(crate) fn into_string(self) -> String { self.0 }
}

impl std::fmt::Display for FilePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

string_newtype!(
    pub(crate) struct FileContent,
    "Content of a file involved in patch operations."
);

impl FileContent {
    /// Consumes the wrapper and returns the underlying file text.
    pub(crate) fn into_string(self) -> String { self.0 }

    /// Replaces the byte range `range` with `replacement`.
    pub(crate) fn replace_range(&mut self, range: std::ops::Range<usize>, replacement: &str) {
        self.0.replace_range(range, replacement);
    }
}

string_newtype!(
    pub(crate) struct SearchPattern,
    "Search block content extracted from a patch."
);

string_newtype!(
    pub(crate) struct ReplacementText,
    "Replacement text extracted from a patch."
);

/// Search/replace block for modify operations.
#[derive(Debug, Clone)]
pub(crate) struct SearchReplaceBlock {
    /// Text the block searches for in the target file.
    pub(crate) search: SearchPattern,
    /// Text that replaces each match.
    pub(crate) replace: ReplacementText,
}

/// Parsed patch operation.
#[derive(Debug, Clone)]
pub(crate) enum PatchOperation {
    /// Replaces matched text in an existing file.
    Modify {
        /// Target file path.
        path: FilePath,
        /// Blocks to apply, in order.
        blocks: Vec<SearchReplaceBlock>,
    },
    /// Creates a new file with the supplied content.
    Create {
        /// Target file path.
        path: FilePath,
        /// Full contents to write.
        content: FileContent,
    },
    /// Deletes an existing file.
    Delete {
        /// Target file path.
        path: FilePath,
    },
}
