# Weaver LSP Host v0.1.0 migration guide

## Document synchronization methods

The `LanguageServer` trait now requires document synchronization methods:
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

    fn prepare_call_hierarchy(
        &mut self,
        _params: lsp_types::CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<lsp_types::CallHierarchyItem>>, LanguageServerError> {
        Ok(None)
    }

    fn incoming_calls(
        &mut self,
        _params: lsp_types::CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<lsp_types::CallHierarchyIncomingCall>>, LanguageServerError> {
        Ok(None)
    }

    fn outgoing_calls(
        &mut self,
        _params: lsp_types::CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<lsp_types::CallHierarchyOutgoingCall>>, LanguageServerError> {
        Ok(None)
    }

    fn hover(
        &mut self,
        _params: lsp_types::HoverParams,
    ) -> Result<Option<lsp_types::Hover>, LanguageServerError> {
        Ok(None)
    }
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Capability set representation

`ServerCapabilitySet` no longer exposes one public boolean field per
capability. It holds a private `BTreeSet<CapabilityKind>` of the capabilities a
server advertised, plus the negotiated position encoding, and is built with
chained calls:

```rust
use lsp_types::PositionEncodingKind;
use weaver_lsp_host::{CapabilityKind, ServerCapabilitySet};

let capabilities = ServerCapabilitySet::new(true, true, false)
    .with_call_hierarchy(true)
    .with_hover(true)
    .with_position_encoding(Some(PositionEncodingKind::UTF8));

assert!(capabilities.supports(CapabilityKind::CallHierarchy));
assert!(capabilities.supports_hover());
assert_eq!(
    capabilities.position_encoding(),
    Some(&PositionEncodingKind::UTF8)
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Why the capability model changed

The host now negotiates five capabilities — `Definition`, `References`,
`Diagnostics`, `CallHierarchy`, and `Hover` — and will negotiate more over
time. A struct of public booleans forces a breaking change for each addition,
and makes it easy for an adaptor to leave one flag at its default by accident.
A set makes "not advertised" the only default state and keeps the supported
capabilities and their queries in one place.

### Migrating the capability model

- Replace struct literals with
  `ServerCapabilitySet::new(definition, references, diagnostics)`.
- Chain `.with_call_hierarchy(...)`, `.with_hover(...)`, and
  `.with_position_encoding(...)` for the remaining capabilities.
- Replace field reads such as `set.definition` with
  `set.supports(CapabilityKind::Definition)` or the corresponding
  `supports_*()` accessor.

Because the build-time flags are now additive, an adaptor that previously had
no way to advertise call hierarchy or hover must opt in explicitly; the host
treats an absent capability as unavailable and reports
`CapabilitySource::MissingOnServer`.
