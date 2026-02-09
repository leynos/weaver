//! LSP-backed semantic lock adapter for apply-patch.

use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;

use crate::safety_harness::{
    SafetyHarnessError, SemanticLock, SemanticLockResult, VerificationContext, VerificationFailure,
};
use crate::semantic_provider::SemanticBackendProvider;
use weaver_lsp_host::{Language, LspHost};

/// Semantic lock adapter that uses the LSP host.
pub(crate) struct LspSemanticLockAdapter<'a> {
    provider: &'a SemanticBackendProvider,
}

impl<'a> LspSemanticLockAdapter<'a> {
    #[rustfmt::skip]
    pub(crate) fn new(provider: &'a SemanticBackendProvider) -> Self { Self { provider } }
}

impl<'a> SemanticLock for LspSemanticLockAdapter<'a> {
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

        let failures =
            failures.ok_or_else(|| SafetyHarnessError::SemanticBackendUnavailable {
                message: String::from("LSP host unavailable"),
            })??;

        if failures.is_empty() {
            Ok(SemanticLockResult::Passed)
        } else {
            Ok(SemanticLockResult::Failed { failures })
        }
    }
}

fn infer_language(path: &Path) -> Option<Language> {
    let extension = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match extension.as_str() {
        "rs" => Some(Language::Rust),
        "py" => Some(Language::Python),
        "ts" | "tsx" => Some(Language::TypeScript),
        _ => None,
    }
}

fn to_uri(path: &Path) -> Result<lsp_types::Uri, SafetyHarnessError> {
    let url = url::Url::from_file_path(path).map_err(|_| {
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

fn did_open_params(
    uri: lsp_types::Uri,
    language: Language,
    text: &str,
) -> lsp_types::DidOpenTextDocumentParams {
    lsp_types::DidOpenTextDocumentParams {
        text_document: lsp_types::TextDocumentItem {
            uri,
            language_id: language.as_str().to_string(),
            version: 1,
            text: text.to_string(),
        },
    }
}

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
        failures.extend(validate_file(host, input)?);
    }
    Ok(failures)
}

struct FileValidation<'a> {
    context: &'a VerificationContext,
    path: &'a Path,
    modified: &'a str,
    language: Language,
}

fn validate_file(
    host: &mut LspHost,
    input: FileValidation<'_>,
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

    let result = validate_open_document(host, &input, uri.clone());
    let close_result = host
        .did_close(input.language, did_close_params(uri))
        .map_err(|e| lsp_error("did_close", e));

    match (result, close_result) {
        (Err(err), _) => Err(err),
        (Ok(_), Err(err)) => Err(err),
        (Ok(failures), Ok(())) => Ok(failures),
    }
}

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

    Ok(filter_new_failures(input.path, baseline, updated))
}

fn initialise_lsp(host: &mut LspHost, language: Language) -> Result<(), SafetyHarnessError> {
    host.initialize(language)
        .map(|_| ())
        .map_err(|e| lsp_error("initialise", e))
}

fn fetch_diagnostics(
    host: &mut LspHost,
    language: Language,
    uri: lsp_types::Uri,
) -> Result<Vec<lsp_types::Diagnostic>, SafetyHarnessError> {
    host.diagnostics(language, uri)
        .map_err(|e| lsp_error("diagnostics", e))
}

fn filter_new_failures(
    path: &Path,
    baseline: Vec<lsp_types::Diagnostic>,
    updated: Vec<lsp_types::Diagnostic>,
) -> Vec<VerificationFailure> {
    let baseline_set = diagnostics_signature_set(&baseline);
    updated
        .into_iter()
        .filter(|diagnostic| is_high_severity(&diagnostic.severity))
        .filter(|diagnostic| !baseline_set.contains(&DiagnosticSignature::from(diagnostic)))
        .map(|diagnostic| {
            let position = diagnostic.range.start;
            VerificationFailure::new(path.to_path_buf(), diagnostic.message)
                .at_location(position.line + 1, position.character + 1)
        })
        .collect()
}

fn lsp_error(action: &str, error: impl std::fmt::Display) -> SafetyHarnessError {
    SafetyHarnessError::SemanticBackendUnavailable {
        message: format!("LSP {action} failed: {error}"),
    }
}

fn did_change_params(uri: lsp_types::Uri, text: &str) -> lsp_types::DidChangeTextDocumentParams {
    lsp_types::DidChangeTextDocumentParams {
        text_document: lsp_types::VersionedTextDocumentIdentifier { uri, version: 2 },
        content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.to_string(),
        }],
    }
}

fn did_close_params(uri: lsp_types::Uri) -> lsp_types::DidCloseTextDocumentParams {
    lsp_types::DidCloseTextDocumentParams {
        text_document: lsp_types::TextDocumentIdentifier { uri },
    }
}

fn is_high_severity(severity: &Option<lsp_types::DiagnosticSeverity>) -> bool {
    matches!(
        severity,
        None | Some(lsp_types::DiagnosticSeverity::ERROR | lsp_types::DiagnosticSeverity::WARNING)
    )
}

fn diagnostics_signature_set(
    diagnostics: &[lsp_types::Diagnostic],
) -> HashSet<DiagnosticSignature> {
    diagnostics
        .iter()
        .filter(|diag| is_high_severity(&diag.severity))
        .map(DiagnosticSignature::from)
        .collect()
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct DiagnosticSignature {
    line: u32,
    character: u32,
    severity: Option<u32>,
    message: String,
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

fn severity_code(severity: lsp_types::DiagnosticSeverity) -> u32 {
    match severity {
        lsp_types::DiagnosticSeverity::ERROR => 1,
        lsp_types::DiagnosticSeverity::WARNING => 2,
        lsp_types::DiagnosticSeverity::INFORMATION => 3,
        lsp_types::DiagnosticSeverity::HINT => 4,
        _ => 0,
    }
}
