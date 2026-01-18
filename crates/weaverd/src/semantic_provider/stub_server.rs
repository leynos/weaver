//! Stub language server implementation for placeholder registration.
//!
//! This module provides a stub `LanguageServer` implementation that can be
//! registered with the `LspHost` until proper process-based adapters are
//! implemented. The stub advertises full capabilities but returns meaningful
//! "not yet implemented" errors when operations are invoked.

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, ReferenceParams, Uri,
};
use weaver_lsp_host::{Language, LanguageServer, LanguageServerError, ServerCapabilitySet};

/// Stub language server that returns "not yet implemented" errors.
///
/// This implementation exists to allow registration with `LspHost` so that
/// initialization succeeds, while providing clear error messages when actual
/// LSP operations are attempted. It will be replaced by proper process-based
/// adapters that spawn real language servers.
pub struct StubLanguageServer {
    language: Language,
}

impl StubLanguageServer {
    /// Creates a stub server for the given language.
    pub fn new(language: Language) -> Self {
        Self { language }
    }

    fn not_implemented(&self, operation: &str) -> LanguageServerError {
        LanguageServerError::new(format!(
            "{operation} for {language} is not yet implemented; \
             process-based language server adapters are pending",
            language = self.language
        ))
    }
}

impl LanguageServer for StubLanguageServer {
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
        // Advertise capabilities so the host doesn't reject operations at the
        // capability check level; the actual operations will return clear errors.
        Ok(ServerCapabilitySet::new(true, true, true).with_call_hierarchy(true))
    }

    fn goto_definition(
        &mut self,
        _params: GotoDefinitionParams,
    ) -> Result<GotoDefinitionResponse, LanguageServerError> {
        Err(self.not_implemented("textDocument/definition"))
    }

    fn references(
        &mut self,
        _params: ReferenceParams,
    ) -> Result<Vec<lsp_types::Location>, LanguageServerError> {
        Err(self.not_implemented("textDocument/references"))
    }

    fn diagnostics(&mut self, _uri: Uri) -> Result<Vec<Diagnostic>, LanguageServerError> {
        Err(self.not_implemented("diagnostics"))
    }

    fn did_open(&mut self, _params: DidOpenTextDocumentParams) -> Result<(), LanguageServerError> {
        // Document sync notifications succeed silently since they don't return data
        Ok(())
    }

    fn did_change(
        &mut self,
        _params: DidChangeTextDocumentParams,
    ) -> Result<(), LanguageServerError> {
        Ok(())
    }

    fn did_close(
        &mut self,
        _params: DidCloseTextDocumentParams,
    ) -> Result<(), LanguageServerError> {
        Ok(())
    }

    fn prepare_call_hierarchy(
        &mut self,
        _params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, LanguageServerError> {
        Err(self.not_implemented("textDocument/prepareCallHierarchy"))
    }

    fn incoming_calls(
        &mut self,
        _params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, LanguageServerError> {
        Err(self.not_implemented("callHierarchy/incomingCalls"))
    }

    fn outgoing_calls(
        &mut self,
        _params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, LanguageServerError> {
        Err(self.not_implemented("callHierarchy/outgoingCalls"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_succeeds_with_full_capabilities() {
        let mut server = StubLanguageServer::new(Language::Rust);
        let caps = server.initialize().expect("initialization should succeed");

        assert!(caps.supports_definition());
        assert!(caps.supports_references());
        assert!(caps.supports_diagnostics());
        assert!(caps.supports_call_hierarchy());
    }

    #[test]
    fn goto_definition_returns_not_implemented_error() {
        let mut server = StubLanguageServer::new(Language::Rust);
        let params = GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: "file:///test.rs".parse().unwrap(),
                },
                position: lsp_types::Position::new(0, 0),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let result = server.goto_definition(params);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.message().contains("not yet implemented"));
        assert!(error.message().contains("rust"));
    }

    #[test]
    fn document_sync_notifications_succeed() {
        let mut server = StubLanguageServer::new(Language::Python);

        // did_open should succeed
        let open_params = DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: "file:///test.py".parse().unwrap(),
                language_id: "python".to_string(),
                version: 1,
                text: "print('hello')".to_string(),
            },
        };
        assert!(server.did_open(open_params).is_ok());

        // did_close should succeed
        let close_params = DidCloseTextDocumentParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: "file:///test.py".parse().unwrap(),
            },
        };
        assert!(server.did_close(close_params).is_ok());
    }
}
