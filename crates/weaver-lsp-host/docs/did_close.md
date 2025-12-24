# Examples

```rust,no_run
use std::str::FromStr;

use lsp_types::{DidCloseTextDocumentParams, TextDocumentIdentifier, Uri};
use weaver_lsp_host::Language;
# use weaver_lsp_host::doc_support::doc_host;
# let mut host = doc_host();

let uri = Uri::from_str("file:///workspace/main.rs")?;
let params = DidCloseTextDocumentParams {
    text_document: TextDocumentIdentifier { uri },
};

host.did_close(Language::Rust, params)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
