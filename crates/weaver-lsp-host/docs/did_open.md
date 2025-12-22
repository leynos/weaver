# Examples

```rust,no_run
use std::str::FromStr;

use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem, Uri};
use weaver_lsp_host::Language;
# use weaver_lsp_host::doc_support::doc_host;
# let mut host = doc_host();

let uri = Uri::from_str("file:///workspace/main.rs")?;
let params = DidOpenTextDocumentParams {
    text_document: TextDocumentItem {
        uri,
        language_id: "rust".to_string(),
        version: 1,
        text: "fn main() {}".to_string(),
    },
};

host.did_open(Language::Rust, params)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
