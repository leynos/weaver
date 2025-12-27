//! Doc-only helpers for rustdoc examples.
//!
//! This module provides a no-op [`LanguageServer`] implementation
//! ([`DocStubServer`]) and a helper constructor ([`doc_host`]) so documentation
//! examples can focus on the API surface without repeating boilerplate.
//! `DocStubServer` advertises no capabilities and returns empty responses,
//! while `doc_host` registers it with an [`LspHost`] for convenience in
//! user-facing docs and doctests.

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams as PrepareParams, Diagnostic,
    DidChangeTextDocumentParams as DidChangeParams, DidCloseTextDocumentParams as DidCloseParams,
    DidOpenTextDocumentParams as DidOpenParams, GotoDefinitionParams, GotoDefinitionResponse,
    Location, ReferenceParams, Uri,
};

use crate::LspHost;
use crate::language::Language;
use crate::server::{LanguageServer, LanguageServerError, ServerCapabilitySet};

/// Stub server used in rustdoc examples.
#[derive(Default)]
pub struct DocStubServer;

impl LanguageServer for DocStubServer {
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
        Ok(ServerCapabilitySet {
            definition: false,
            references: false,
            diagnostics: false,
            call_hierarchy: false,
        })
    }

    fn goto_definition(
        &mut self,
        _params: GotoDefinitionParams,
    ) -> Result<GotoDefinitionResponse, LanguageServerError> {
        Ok(GotoDefinitionResponse::Array(Vec::new()))
    }

    fn references(
        &mut self,
        _params: ReferenceParams,
    ) -> Result<Vec<Location>, LanguageServerError> {
        Ok(Vec::new())
    }

    fn diagnostics(&mut self, _uri: Uri) -> Result<Vec<Diagnostic>, LanguageServerError> {
        Ok(Vec::new())
    }

    // Keep single-line no-op methods for doc stub readability.
    #[rustfmt::skip]
    fn did_open(&mut self, _params: DidOpenParams) -> Result<(), LanguageServerError> { Ok(()) }

    #[rustfmt::skip]
    fn did_change(&mut self, _params: DidChangeParams) -> Result<(), LanguageServerError> { Ok(()) }

    #[rustfmt::skip]
    fn did_close(&mut self, _params: DidCloseParams) -> Result<(), LanguageServerError> { Ok(()) }

    fn prepare_call_hierarchy(
        &mut self,
        _params: PrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>, LanguageServerError> {
        Ok(None)
    }

    fn incoming_calls(
        &mut self,
        _params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>, LanguageServerError> {
        Ok(None)
    }

    fn outgoing_calls(
        &mut self,
        _params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>, LanguageServerError> {
        Ok(None)
    }
}

/// Builds an [`LspHost`] with a registered Rust stub server.
#[must_use]
pub fn doc_host() -> LspHost {
    let mut host = LspHost::new(weaver_config::CapabilityMatrix::default());
    host.register_language(Language::Rust, Box::new(DocStubServer))
        .expect("doc host registration failed");
    host
}
