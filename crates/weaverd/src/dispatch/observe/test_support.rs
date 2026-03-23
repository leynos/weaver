//! Test doubles shared by `observe` unit tests.

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, Location, MarkupContent,
    MarkupKind, ReferenceParams, Uri,
};
use tempfile::TempDir;
use weaver_config::{CapabilityMatrix, Config, SocketEndpoint};
use weaver_lsp_host::{
    Language, LanguageServer, LanguageServerError, LspHost, ServerCapabilitySet,
};

use crate::backends::FusionBackends;
use crate::semantic_provider::SemanticBackendProvider;

pub(crate) struct StubLanguageServer {
    capabilities: ServerCapabilitySet,
    hover: Option<Hover>,
    initialize_error: Option<String>,
    hover_error: Option<String>,
}

impl StubLanguageServer {
    pub(crate) fn with_hover(capabilities: ServerCapabilitySet, hover: Hover) -> Self {
        Self {
            capabilities,
            hover: Some(hover),
            initialize_error: None,
            hover_error: None,
        }
    }

    pub(crate) fn missing_hover(capabilities: ServerCapabilitySet) -> Self {
        Self {
            capabilities,
            hover: None,
            initialize_error: None,
            hover_error: None,
        }
    }

    pub(crate) fn failing_initialize(
        capabilities: ServerCapabilitySet,
        message: impl Into<String>,
    ) -> Self {
        Self {
            capabilities,
            hover: None,
            initialize_error: Some(message.into()),
            hover_error: None,
        }
    }

    pub(crate) fn failing_hover(
        capabilities: ServerCapabilitySet,
        message: impl Into<String>,
    ) -> Self {
        Self {
            capabilities,
            hover: None,
            initialize_error: None,
            hover_error: Some(message.into()),
        }
    }
}

impl LanguageServer for StubLanguageServer {
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
        match &self.initialize_error {
            Some(message) => Err(LanguageServerError::new(message.clone())),
            None => Ok(self.capabilities.clone()),
        }
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

    fn prepare_call_hierarchy(
        &mut self,
        _params: CallHierarchyPrepareParams,
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

    fn hover(&mut self, _params: HoverParams) -> Result<Option<Hover>, LanguageServerError> {
        match &self.hover_error {
            Some(message) => Err(LanguageServerError::new(message.clone())),
            None => Ok(self.hover.clone()),
        }
    }
}

pub(crate) fn markdown_hover(value: &str) -> Hover {
    Hover {
        contents: lsp_types::HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: String::from(value),
        }),
        range: None,
    }
}

pub(crate) fn semantic_backends_with_server(
    language: Language,
    server: impl LanguageServer + 'static,
) -> (FusionBackends<SemanticBackendProvider>, TempDir) {
    let capability_matrix = CapabilityMatrix::default();
    let mut lsp_host = LspHost::new(capability_matrix.clone());
    lsp_host
        .register_language(language, Box::new(server))
        .expect("register test language server");

    let provider =
        SemanticBackendProvider::with_lsp_host_for_tests(capability_matrix.clone(), lsp_host);
    let (config, dir) = test_config();
    (FusionBackends::new(config, provider), dir)
}

fn test_config() -> (Config, TempDir) {
    let dir = TempDir::new().expect("create temp dir");
    let socket_path = dir
        .path()
        .join("socket.sock")
        .to_string_lossy()
        .into_owned();
    let config = Config {
        daemon_socket: SocketEndpoint::unix(socket_path),
        ..Config::default()
    };
    (config, dir)
}
