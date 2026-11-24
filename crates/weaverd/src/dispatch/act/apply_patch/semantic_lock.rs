//! LSP-backed semantic lock adapter for apply-patch.

use std::{collections::HashSet, path::Path, str::FromStr};

use weaver_lsp_host::{Language, LspHost};

use crate::{
    safety_harness::{
        SafetyHarnessError,
        SemanticLock,
        SemanticLockResult,
        VerificationContext,
        VerificationFailure,
    },
    semantic_provider::SemanticBackendProvider,
};

/// Semantic lock adapter that uses the LSP host.
pub(crate) struct LspSemanticLockAdapter<'a> {
    /// Provider used to borrow an exclusive LSP host per validation.
    provider: &'a SemanticBackendProvider,
}

impl<'a> LspSemanticLockAdapter<'a> {
    /// Creates an adapter that validates through `provider`.
    #[rustfmt::skip]
    pub(crate) const fn new(provider: &'a SemanticBackendProvider) -> Self {
        Self { provider }
    }
}

impl SemanticLock for LspSemanticLockAdapter<'_> {
    fn validate(
        &self,
        context: &VerificationContext,
    ) -> Result<SemanticLockResult, SafetyHarnessError> {
        if context.is_empty() {
            return Ok(SemanticLockResult::Passed);
        }

        let failures = self
            .provider
            .with_lsp_host_mut(|host| collect_failures(host, context))
            .map_err(|_| SafetyHarnessError::SemanticBackendUnavailable {
                message: String::from("LSP host lock poisoned"),
            })?;

        let backend_failures =
            failures.ok_or_else(|| SafetyHarnessError::SemanticBackendUnavailable {
                message: String::from("LSP host unavailable"),
            })??;

        if backend_failures.is_empty() {
            Ok(SemanticLockResult::Passed)
        } else {
            Ok(SemanticLockResult::Failed {
                failures: backend_failures,
            })
        }
    }
}

/// Infers the LSP language for a file from its extension.
fn infer_language(path: &Path) -> Option<Language> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match extension.as_str() {
        "rs" => Some(Language::Rust),
        "py" => Some(Language::Python),
        "ts" | "tsx" => Some(Language::TypeScript),
        _ => None,
    }
}

/// Converts a filesystem path into an LSP document URI.
///
/// # Errors
///
/// Returns [`SafetyHarnessError::SemanticBackendUnavailable`] when the path is
/// not absolute or the resulting URL is not a valid URI.
fn to_uri(path: &Path) -> Result<lsp_types::Uri, SafetyHarnessError> {
    let url = url::Url::from_file_path(path).map_err(|()| {
        SafetyHarnessError::SemanticBackendUnavailable {
            message: format!("failed to build URI for {}", path.display()),
        }
    })?;
    lsp_types::Uri::from_str(url.as_str()).map_err(|_| {
        SafetyHarnessError::SemanticBackendUnavailable {
            message: format!("failed to parse URI for {}", path.display()),
        }
    })
}

/// Builds the `textDocument/didOpen` parameters for a file.
fn did_open_params(
    uri: lsp_types::Uri,
    language: Language,
    text: &str,
) -> lsp_types::DidOpenTextDocumentParams {
    lsp_types::DidOpenTextDocumentParams {
        text_document: lsp_types::TextDocumentItem {
            uri,
            language_id: language.as_str().to_owned(),
            version: 1,
            text: text.to_owned(),
        },
    }
}

/// Collects verification failures for every modified file in `context`.
///
/// # Errors
///
/// Returns [`SafetyHarnessError::SemanticBackendUnavailable`] when the LSP host
/// cannot open, change, or query a document.
fn collect_failures(
    host: &mut LspHost,
    context: &VerificationContext,
) -> Result<Vec<VerificationFailure>, SafetyHarnessError> {
    let mut failures = Vec::new();
    for (path, modified) in context.modified_files() {
        let Some(language) = infer_language(path) else {
            // Skip files without a supported language to avoid noisy LSP errors.
            continue;
        };
        let input = FileValidation {
            context,
            path,
            modified: modified.as_str(),
            language,
        };
        failures.extend(validate_file(host, &input)?);
    }
    Ok(failures)
}

/// Per-file inputs needed to drive one LSP validation round.
struct FileValidation<'a> {
    /// Verification context holding the original file contents.
    context: &'a VerificationContext,
    /// Path of the file being validated.
    path: &'a Path,
    /// Modified file contents to push into the language server.
    modified: &'a str,
    /// Language server dialect used for this file.
    language: Language,
}

/// Validates one modified file by diffing its diagnostics against the original.
///
/// # Errors
///
/// Returns [`SafetyHarnessError::SemanticBackendUnavailable`] when the language
/// server cannot be started, opened, changed, or queried.
fn validate_file(
    host: &mut LspHost,
    input: &FileValidation<'_>,
) -> Result<Vec<VerificationFailure>, SafetyHarnessError> {
    let uri = to_uri(input.path)?;
    initialise_lsp(host, input.language)?;
    let original = input
        .context
        .original(input.path)
        .map(String::as_str)
        .unwrap_or_default();
    host.did_open(
        input.language,
        did_open_params(uri.clone(), input.language, original),
    )
    .map_err(|e| lsp_error("did_open", e))?;

    let result = validate_open_document(host, input, uri.clone());
    let close_result = host
        .did_close(input.language, did_close_params(uri))
        .map_err(|e| lsp_error("did_close", e));

    match (result, close_result) {
        (Err(err), _) | (Ok(_), Err(err)) => Err(err),
        (Ok(failures), Ok(())) => Ok(failures),
    }
}

/// Pushes the modified text and reports diagnostics introduced by the change.
///
/// # Errors
///
/// Returns [`SafetyHarnessError::SemanticBackendUnavailable`] when a diagnostic
/// request fails.
fn validate_open_document(
    host: &mut LspHost,
    input: &FileValidation<'_>,
    uri: lsp_types::Uri,
) -> Result<Vec<VerificationFailure>, SafetyHarnessError> {
    let baseline = fetch_diagnostics(host, input.language, uri.clone())?;

    host.did_change(
        input.language,
        did_change_params(uri.clone(), input.modified),
    )
    .map_err(|e| lsp_error("did_change", e))?;

    let updated = fetch_diagnostics(host, input.language, uri)?;

    Ok(filter_new_failures(input.path, &baseline, updated))
}

/// Starts the language server for `language`, ignoring an already-started host.
///
/// # Errors
///
/// Returns [`SafetyHarnessError::SemanticBackendUnavailable`] when the host
/// cannot initialise the language server.
fn initialise_lsp(host: &mut LspHost, language: Language) -> Result<(), SafetyHarnessError> {
    host.initialize(language)
        .map(|_| ())
        .map_err(|e| lsp_error("initialise", e))
}

/// Fetches all diagnostics for a document from the LSP host.
///
/// # Errors
///
/// Returns [`SafetyHarnessError::SemanticBackendUnavailable`] when the
/// diagnostic request fails.
fn fetch_diagnostics(
    host: &mut LspHost,
    language: Language,
    uri: lsp_types::Uri,
) -> Result<Vec<lsp_types::Diagnostic>, SafetyHarnessError> {
    host.diagnostics(language, uri)
        .map_err(|e| lsp_error("diagnostics", e))
}

/// Keeps the high-severity diagnostics that were absent from `baseline`.
fn filter_new_failures(
    path: &Path,
    baseline: &[lsp_types::Diagnostic],
    updated: Vec<lsp_types::Diagnostic>,
) -> Vec<VerificationFailure> {
    let baseline_set = diagnostics_signature_set(baseline);
    updated
        .into_iter()
        .filter(|diagnostic| is_high_severity(diagnostic.severity))
        .filter(|diagnostic| !baseline_set.contains(&DiagnosticSignature::from(diagnostic)))
        .map(|diagnostic| {
            let position = diagnostic.range.start;
            VerificationFailure::new(path.to_path_buf(), diagnostic.message)
                .at_location(position.line + 1, position.character + 1)
        })
        .collect()
}

/// Converts any displayable LSP client error into a backend-unavailable error.
fn lsp_error(action: &str, error: impl std::fmt::Display) -> SafetyHarnessError {
    SafetyHarnessError::SemanticBackendUnavailable {
        message: format!("LSP {action} failed: {error}"),
    }
}

/// Builds the full-document `textDocument/didChange` parameters.
fn did_change_params(uri: lsp_types::Uri, text: &str) -> lsp_types::DidChangeTextDocumentParams {
    lsp_types::DidChangeTextDocumentParams {
        text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version: 2 },
        content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.to_owned(),
        }],
    }
}

/// Builds the `textDocument/didClose` parameters for a document.
const fn did_close_params(uri: lsp_types::Uri) -> lsp_types::DidCloseTextDocumentParams {
    lsp_types::DidCloseTextDocumentParams {
        text_document: lsp_types::TextDocumentIdentifier { uri },
    }
}

/// Reports whether a diagnostic severity should fail the semantic lock.
///
/// A missing severity is treated as an error, matching the LSP default.
const fn is_high_severity(severity: Option<lsp_types::DiagnosticSeverity>) -> bool {
    matches!(
        severity,
        None | Some(lsp_types::DiagnosticSeverity::ERROR | lsp_types::DiagnosticSeverity::WARNING)
    )
}

/// Builds the comparison key set for the high-severity diagnostics.
fn diagnostics_signature_set(
    diagnostics: &[lsp_types::Diagnostic],
) -> HashSet<DiagnosticSignature> {
    diagnostics
        .iter()
        .filter(|diag| is_high_severity(diag.severity))
        .map(DiagnosticSignature::from)
        .collect()
}

/// Identity of a diagnostic, used to tell pre-existing errors from new ones.
#[derive(Debug, Hash, PartialEq, Eq)]
struct DiagnosticSignature {
    /// Zero-based start line of the diagnostic range.
    line: u32,
    /// Zero-based start character of the diagnostic range.
    character: u32,
    /// Numeric LSP severity code, when the server supplied one.
    severity: Option<u32>,
    /// Diagnostic message text.
    message: String,
    /// Diagnostic code rendered as text, when present.
    code: Option<String>,
}

impl From<&lsp_types::Diagnostic> for DiagnosticSignature {
    fn from(diagnostic: &lsp_types::Diagnostic) -> Self {
        let code = diagnostic.code.as_ref().map(|code| match code {
            lsp_types::NumberOrString::Number(value) => value.to_string(),
            lsp_types::NumberOrString::String(value) => value.clone(),
        });
        Self {
            line: diagnostic.range.start.line,
            character: diagnostic.range.start.character,
            severity: diagnostic.severity.map(severity_code),
            message: diagnostic.message.clone(),
            code,
        }
    }
}

/// Maps an LSP severity to its stable numeric code for comparison.
const fn severity_code(severity: lsp_types::DiagnosticSeverity) -> u32 {
    match severity {
        lsp_types::DiagnosticSeverity::ERROR => 1,
        lsp_types::DiagnosticSeverity::WARNING => 2,
        lsp_types::DiagnosticSeverity::INFORMATION => 3,
        lsp_types::DiagnosticSeverity::HINT => 4,
        _ => 0,
    }
}
