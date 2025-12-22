# Examples

```rust,no_run
# use std::str::FromStr;
# use lsp_types::{DidCloseTextDocumentParams, TextDocumentIdentifier, Uri};
# use weaver_lsp_host::{LanguageServer, LanguageServerError, ServerCapabilitySet};
# struct StubServer;
# impl LanguageServer for StubServer {
#     fn initialize(&mut self) -> Result<ServerCapabilitySet, LanguageServerError> {
#         Ok(ServerCapabilitySet::new(false, false, false))
#     }
#     fn goto_definition(
#         &mut self,
#         _params: lsp_types::GotoDefinitionParams,
#     ) -> Result<lsp_types::GotoDefinitionResponse, LanguageServerError> {
#         unimplemented!("see trait-level example")
#     }
#     fn references(
#         &mut self,
#         _params: lsp_types::ReferenceParams,
#     ) -> Result<Vec<lsp_types::Location>, LanguageServerError> {
#         unimplemented!("see trait-level example")
#     }
#     fn diagnostics(
#         &mut self,
#         _uri: lsp_types::Uri,
#     ) -> Result<Vec<lsp_types::Diagnostic>, LanguageServerError> {
#         unimplemented!("see trait-level example")
#     }
#     fn did_open(
#         &mut self,
#         _params: lsp_types::DidOpenTextDocumentParams,
#     ) -> Result<(), LanguageServerError> {
#         unimplemented!("see trait-level example")
#     }
#     fn did_change(
#         &mut self,
#         _params: lsp_types::DidChangeTextDocumentParams,
#     ) -> Result<(), LanguageServerError> {
#         unimplemented!("see trait-level example")
#     }
#     fn did_close(
#         &mut self,
#         _params: DidCloseTextDocumentParams,
#     ) -> Result<(), LanguageServerError> {
#         Ok(())
#     }
# }
# let mut server = StubServer;
let uri = Uri::from_str("file:///workspace/main.rs")?;
let params = DidCloseTextDocumentParams {
    text_document: TextDocumentIdentifier { uri },
};

server.did_close(params)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
