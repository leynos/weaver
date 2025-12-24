# Examples

```rust,no_run
# use std::str::FromStr;
# use lsp_types::{DidCloseTextDocumentParams, TextDocumentIdentifier, Uri};
# use weaver_lsp_host::doc_support::DocStubServer;
# use weaver_lsp_host::LanguageServer;
# let mut server = DocStubServer::default();
let uri = Uri::from_str("file:///workspace/main.rs")?;
let params = DidCloseTextDocumentParams {
    text_document: TextDocumentIdentifier { uri },
};

server.did_close(params)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
