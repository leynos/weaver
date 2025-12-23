# Examples

```rust,no_run
# use std::str::FromStr;
# use lsp_types::{DidChangeTextDocumentParams, TextDocumentContentChangeEvent};
# use lsp_types::{Uri, VersionedTextDocumentIdentifier};
# use weaver_lsp_host::doc_support::DocStubServer;
# use weaver_lsp_host::LanguageServer;
# let mut server = DocStubServer::default();
let uri = Uri::from_str("file:///workspace/main.rs")?;
let params = DidChangeTextDocumentParams {
    text_document: VersionedTextDocumentIdentifier { uri, version: 2 },
    content_changes: vec![TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text: "fn main() { println!(\"hi\"); }".to_string(),
    }],
};

server.did_change(params)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
