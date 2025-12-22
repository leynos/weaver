//! Doc-only helpers for examples in rustdoc.

use lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Location, ReferenceParams, Uri,
};

use crate::LspHost;
use crate::language::Language;
use crate::server::{LanguageServer, LanguageServerError, ServerCapabilitySet};

/// Stub server used in rustdoc examples.
pub struct DocStubServer;

impl LanguageServer for DocStubServer {
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
        Ok(ServerCapabilitySet::new(false, false, false))
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

    fn did_open(&mut self, _params: DidOpenTextDocumentParams) -> Result<(), LanguageServerError> {
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
}

/// Builds an [`LspHost`] with a registered Rust stub server.
#[must_use]
pub fn doc_host() -> LspHost {
    let mut host = LspHost::new(weaver_config::CapabilityMatrix::default());
    host.register_language(Language::Rust, Box::new(DocStubServer))
        .expect("doc host registration failed");
    host
}
