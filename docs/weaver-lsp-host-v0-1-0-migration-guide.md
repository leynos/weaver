# Weaver LSP Host v0.1.0 migration guide

## Document synchronisation methods

The `LanguageServer` trait now requires document synchronisation methods:
`did_open`, `did_change`, and `did_close`. These notifications are required for
semantic validation that operates on in-memory document contents. Any crate
implementing the trait must now provide these methods.

### Why this change?

The host must notify language servers when a document is opened, updated, or
closed so that semantic validators can work with unsaved edits. Without these
callbacks, servers can only operate on on-disk content.

### Migration steps

- Add the three new methods to your `LanguageServer` implementation.
- If your server maintains an in-memory document store, update it in these
  methods.
- If you do not need document tracking, a no-op implementation is sufficient.

### Example

```rust,no_run
use lsp_types::{DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams};
use weaver_lsp_host::{LanguageServer, LanguageServerError, ServerCapabilitySet};

struct MyServer;

impl LanguageServer for MyServer {
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
# Ok::<(), Box<dyn std::error::Error>>(())
```
