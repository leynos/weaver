# Examples

```rust,no_run
# use std::str::FromStr;
# use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem, Uri};
# use weaver_lsp_host::doc_support::DocStubServer;
# use weaver_lsp_host::LanguageServer;
# let mut server = DocStubServer::default();
let uri = Uri::from_str("file:///workspace/main.rs")?;
let params = DidOpenTextDocumentParams {
    text_document: TextDocumentItem {
        uri,
        language_id: "rust".to_string(),
        version: 1,
        text: "fn main() {}".to_string(),
    },
};

server.did_open(params)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
