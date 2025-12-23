# Examples

```rust,no_run
use lsp_types::{
    Diagnostic, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionParams, GotoDefinitionResponse, Location, ReferenceParams, Uri,
};
use weaver_lsp_host::{LanguageServer, LanguageServerError, ServerCapabilitySet};

struct StubServer;

impl LanguageServer for StubServer {
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

    fn did_open(
        &mut self,
        _params: DidOpenTextDocumentParams,
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
        _params: DidCloseTextDocumentParams,
    ) -> Result<(), LanguageServerError> {
        Ok(())
    }
}

let mut server = StubServer;
let _capabilities = server.initialize()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
