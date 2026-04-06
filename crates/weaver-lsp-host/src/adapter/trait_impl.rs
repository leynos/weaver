//! Implementation of [`LanguageServer`] trait for [`ProcessLanguageServer`].

use lsp_types::{
    CallHierarchyClientCapabilities,
    CallHierarchyIncomingCall,
    CallHierarchyIncomingCallsParams,
    CallHierarchyItem,
    CallHierarchyOutgoingCall,
    CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams,
    ClientCapabilities,
    Diagnostic,
    DidChangeTextDocumentParams,
    DidCloseTextDocumentParams,
    DidOpenTextDocumentParams,
    DocumentDiagnosticParams,
    DocumentDiagnosticReport,
    GeneralClientCapabilities,
    GotoDefinitionParams,
    GotoDefinitionResponse,
    Hover,
    HoverParams,
    HoverProviderCapability,
    InitializeParams,
    InitializeResult,
    InitializedParams,
    PositionEncodingKind,
    ReferenceParams,
    TextDocumentClientCapabilities,
    TextDocumentIdentifier,
    Uri,
};
use tracing::debug;

use super::{lifecycle::ADAPTER_TARGET, process::ProcessLanguageServer};
use crate::server::{LanguageServer, LanguageServerError, ServerCapabilitySet};

impl ProcessLanguageServer {
    fn send_initialize_handshake(&mut self) -> Result<InitializeResult, LanguageServerError> {
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            capabilities: ClientCapabilities {
                general: Some(GeneralClientCapabilities {
                    position_encodings: Some(vec![PositionEncodingKind::UTF8]),
                    ..Default::default()
                }),
                text_document: Some(TextDocumentClientCapabilities {
                    call_hierarchy: Some(CallHierarchyClientCapabilities::default()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let result: InitializeResult = self
            .send_request("initialize", params)
            .map_err(|e| LanguageServerError::with_source("initialization handshake failed", e))?;

        self.send_notification("initialized", InitializedParams {})
            .map_err(|e| {
                LanguageServerError::with_source("failed to send initialized notification", e)
            })?;

        Ok(result)
    }

    fn negotiate_position_encoding<'a>(
        &self,
        caps: &'a lsp_types::ServerCapabilities,
    ) -> Option<&'a PositionEncodingKind> {
        let negotiated = caps.position_encoding.as_ref();
        if negotiated != Some(&PositionEncodingKind::UTF8) {
            debug!(
                target: ADAPTER_TARGET,
                language = %self.language(),
                negotiated = ?negotiated,
                "server did not agree to UTF-8 position encoding; LSP features requiring \
                 character offsets will be degraded"
            );
        }
        negotiated
    }

    fn build_capability_set(
        &self,
        caps: &lsp_types::ServerCapabilities,
        position_encoding: Option<&PositionEncodingKind>,
    ) -> ServerCapabilitySet {
        let definition_supported = caps.definition_provider.is_some();
        let references_supported = caps.references_provider.is_some();
        let diagnostics_supported = caps.diagnostic_provider.is_some();
        let call_hierarchy_supported = caps.call_hierarchy_provider.is_some();
        let hover_supported = supports_hover(&caps.hover_provider);

        debug!(
            target: ADAPTER_TARGET,
            language = %self.language(),
            definition = definition_supported,
            references = references_supported,
            diagnostics = diagnostics_supported,
            call_hierarchy = call_hierarchy_supported,
            hover = hover_supported,
            "language server initialized with capabilities"
        );

        ServerCapabilitySet::new(
            definition_supported,
            references_supported,
            diagnostics_supported,
        )
        .with_call_hierarchy(call_hierarchy_supported)
        .with_hover(hover_supported)
        .with_position_encoding(position_encoding.cloned())
    }
}

impl LanguageServer for ProcessLanguageServer {
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
        debug!(
            target: ADAPTER_TARGET,
            language = %self.language(),
            "initializing language server"
        );

        let (child, transport) = self.spawn_process().map_err(|e| {
            LanguageServerError::with_source(
                format!("failed to spawn {} language server", self.language()),
                e,
            )
        })?;

        self.set_running_state(child, transport);

        let result = self.send_initialize_handshake()?;
        let caps = &result.capabilities;
        let encoding = self.negotiate_position_encoding(caps);

        Ok(self.build_capability_set(caps, encoding))
    }

    fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> Result<GotoDefinitionResponse, LanguageServerError> {
        self.send_request_optional("textDocument/definition", params)
            .map(|opt| opt.unwrap_or(GotoDefinitionResponse::Array(vec![])))
            .map_err(|e| LanguageServerError::with_source("definition request failed", e))
    }

    fn references(
        &mut self,
        params: ReferenceParams,
    ) -> Result<Vec<lsp_types::Location>, LanguageServerError> {
        self.send_request_optional("textDocument/references", params)
            .map(|opt| opt.unwrap_or_default())
            .map_err(|e| LanguageServerError::with_source("references request failed", e))
    }

    fn diagnostics(&mut self, uri: Uri) -> Result<Vec<Diagnostic>, LanguageServerError> {
        // Use pull-based diagnostics (textDocument/diagnostic)
        let params = DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result: DocumentDiagnosticReport = self
            .send_request("textDocument/diagnostic", params)
            .map_err(|e| LanguageServerError::with_source("diagnostics request failed", e))?;

        // Extract diagnostics from report
        let diagnostics = match result {
            DocumentDiagnosticReport::Full(full) => full.full_document_diagnostic_report.items,
            DocumentDiagnosticReport::Unchanged(_) => {
                debug!(
                    target: ADAPTER_TARGET,
                    language = %self.language(),
                    ?uri,
                    previous_result_id = ?None::<String>,
                    "DocumentDiagnosticReport::Unchanged (unexpected: previous_result_id is None)"
                );
                Vec::new()
            }
        };

        Ok(diagnostics)
    }

    fn did_open(&mut self, params: DidOpenTextDocumentParams) -> Result<(), LanguageServerError> {
        self.send_notification("textDocument/didOpen", params)
            .map_err(|e| LanguageServerError::with_source("didOpen notification failed", e))
    }

    fn did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> Result<(), LanguageServerError> {
        self.send_notification("textDocument/didChange", params)
            .map_err(|e| LanguageServerError::with_source("didChange notification failed", e))
    }

    fn did_close(&mut self, params: DidCloseTextDocumentParams) -> Result<(), LanguageServerError> {
        self.send_notification("textDocument/didClose", params)
            .map_err(|e| LanguageServerError::with_source("didClose notification failed", e))
    }

    fn prepare_call_hierarchy(
        &mut self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, LanguageServerError> {
        self.send_request_optional("textDocument/prepareCallHierarchy", params)
            .map_err(|e| LanguageServerError::with_source("prepareCallHierarchy request failed", e))
    }

    fn incoming_calls(
        &mut self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, LanguageServerError> {
        self.send_request_optional("callHierarchy/incomingCalls", params)
            .map_err(|e| LanguageServerError::with_source("incomingCalls request failed", e))
    }

    fn outgoing_calls(
        &mut self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, LanguageServerError> {
        self.send_request_optional("callHierarchy/outgoingCalls", params)
            .map_err(|e| LanguageServerError::with_source("outgoingCalls request failed", e))
    }

    fn hover(&mut self, params: HoverParams) -> Result<Option<Hover>, LanguageServerError> {
        self.send_request_optional("textDocument/hover", params)
            .map_err(|e| LanguageServerError::with_source("hover request failed", e))
    }
}

fn supports_hover(capability: &Option<HoverProviderCapability>) -> bool {
    matches!(
        capability,
        Some(HoverProviderCapability::Simple(true)) | Some(HoverProviderCapability::Options(_))
    )
}

#[cfg(test)]
mod tests {
    //! Unit tests for LSP capability detection and trait implementations.

    use lsp_types::{HoverOptions, WorkDoneProgressOptions};

    use super::*;

    #[test]
    fn explicit_false_hover_capability_is_not_treated_as_supported() {
        assert!(!supports_hover(&Some(HoverProviderCapability::Simple(
            false
        ))));
    }

    #[test]
    fn explicit_true_hover_capability_is_treated_as_supported() {
        assert!(supports_hover(&Some(HoverProviderCapability::Simple(true))));
    }

    #[test]
    fn hover_options_are_treated_as_supported() {
        assert!(supports_hover(&Some(HoverProviderCapability::Options(
            HoverOptions {
                work_done_progress_options: WorkDoneProgressOptions {
                    work_done_progress: Some(true),
                },
            },
        ))));
    }
}
