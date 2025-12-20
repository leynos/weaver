# Examples

```rust,no_run
use std::str::FromStr;

use lsp_types::{DidChangeTextDocumentParams, TextDocumentContentChangeEvent};
use lsp_types::{Uri, VersionedTextDocumentIdentifier};
use weaver_lsp_host::{
    Language, LanguageServer, LanguageServerError, LspHost, ServerCapabilitySet,
};

struct StubServer;

impl LanguageServer for StubServer {
    fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
        Ok(ServerCapabilitySet::new(false, false, false))
    }

    fn goto_definition(
        &mut self,
        _params: lsp_types::GotoDefinitionParams,
    ) -> Result<lsp_types::GotoDefinitionResponse, LanguageServerError> {
        Ok(lsp_types::GotoDefinitionResponse::Array(Vec::new()))
    }

    fn references(
        &mut self,
        _params: lsp_types::ReferenceParams,
    ) -> Result<Vec<lsp_types::Location>, LanguageServerError> {
        Ok(Vec::new())
    }

    fn diagnostics(
        &mut self,
        _uri: lsp_types::Uri,
    ) -> Result<Vec<lsp_types::Diagnostic>, LanguageServerError> {
        Ok(Vec::new())
    }

    fn did_open(
        &mut self,
        _params: lsp_types::DidOpenTextDocumentParams,
    ) -> Result<(), LanguageServerError> {
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
        _params: lsp_types::DidCloseTextDocumentParams,
    ) -> Result<(), LanguageServerError> {
        Ok(())
    }
}

let mut host = LspHost::new(weaver_config::CapabilityMatrix::default());
host.register_language(Language::Rust, Box::new(StubServer))?;

let uri = Uri::from_str("file:///workspace/main.rs")?;
let params = DidChangeTextDocumentParams {
    text_document: VersionedTextDocumentIdentifier { uri, version: 2 },
    content_changes: vec![TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text: "fn main() { println!(\"hi\"); }".to_string(),
    }],
};

host.did_change(Language::Rust, params)?;
Ok::<(), Box<dyn std::error::Error>>(())
```
