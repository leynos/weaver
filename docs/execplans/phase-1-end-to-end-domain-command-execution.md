# ExecPlan: End-to-End Domain Command Execution

## Summary

Wire End-to-End (E2E) domain command execution from command-line interface (CLI)
through daemon to backend, starting with `observe get-definition` as the first
complete path.

**Acceptance Criteria:**

- `weaver observe get-definition` with a running daemon returns Language Server
  Protocol (LSP) definition results
- Errors propagate with structured messages
- CLI exits with the daemon-provided status code
- E2E tests exercise the client/daemon pipeline
- Behaviour-Driven Development (BDD) tests cover happy and unhappy paths using
  rstest-bdd v0.3.2
- `docs/users-guide.md` updated with behaviour changes
- `make check-fmt`, `make lint`, `make test` all pass
- Roadmap entry marked as done

______________________________________________________________________

## Design Decisions

1. **Response stream target**: Add `Stdout` variant to `StreamTarget` for
   success responses. The CLI already handles both streams; daemon currently
   only uses stderr.

2. **Backend wiring**: The `DispatchConnectionHandler` will receive a shared
   reference to `FusionBackends` containing a `SemanticBackendProvider` that
   manages the `LspHost`. This avoids tight coupling between router and LSP.

3. **Argument format**: Follow users-guide.md convention:
   `--uri file:///path.rs --position 10:5`. Position uses `LINE:COL` format
   (1-indexed for user-facing, converted to 0-indexed for LSP).

4. **Language inference**: Derive language from URI file extension:
   `.rs` → Rust, `.py` → Python, `.ts`/`.tsx` → TypeScript. Unknown extensions
   return a structured error.

5. **Response format**: JSON payload per users-guide.md:
   `{"uri":"<URI>","line":42,"column":17}` for each definition location.

6. **Error propagation**: New `DispatchError` variants for argument validation,
   backend startup failures, and LSP host errors. All return exit status 1.

______________________________________________________________________

## Critical Files

| File                                                    | Purpose                      |
| ------------------------------------------------------- | ---------------------------- |
| `crates/weaverd/src/dispatch/response.rs`               | Add `Stdout` stream target   |
| `crates/weaverd/src/dispatch/router.rs`                 | Wire observe ops to handlers |
| `crates/weaverd/src/dispatch/handler.rs`                | Pass backends to router      |
| `crates/weaverd/src/dispatch/errors.rs`                 | New error variants           |
| `crates/weaverd/src/dispatch/observe/mod.rs`            | New observe handler module   |
| `crates/weaverd/src/dispatch/observe/get_definition.rs` | Handler implementation       |
| `crates/weaverd/src/dispatch/observe/arguments.rs`      | Argument parsing             |
| `crates/weaverd/src/dispatch/observe/responses.rs`      | Response serialization       |
| `crates/weaverd/src/semantic_provider.rs`               | LSP host backend provider    |
| `crates/weaverd/tests/features/daemon_dispatch.feature` | BDD scenarios                |
| `crates/weaverd/src/tests/dispatch_behaviour.rs`        | Step definitions             |
| `docs/users-guide.md`                                   | Documentation updates        |
| `docs/roadmap.md`                                       | Mark task complete           |

______________________________________________________________________

## Implementation Steps

### Step 1: Add stdout stream target

**File:** `crates/weaverd/src/dispatch/response.rs`

Add `Stdout` variant to `StreamTarget` enum and corresponding helper methods:

```rust
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamTarget {
    Stdout,
    Stderr,
}

impl DaemonMessage {
    pub fn stdout(data: impl Into<String>) -> Self {
        Self::Stream {
            stream: StreamTarget::Stdout,
            data: data.into(),
        }
    }
}

impl<W: Write> ResponseWriter<W> {
    pub fn write_stdout(&mut self, data: impl Into<String>) -> Result<(), DispatchError> {
        self.write_message(&DaemonMessage::stdout(data))
    }
}
```

Add unit tests for stdout message serialization.

______________________________________________________________________

### Step 2: Extend dispatch errors

**File:** `crates/weaverd/src/dispatch/errors.rs`

Add new error variants:

```rust
#[derive(Debug, Error)]
pub enum DispatchError {
    // ... existing variants ...

    #[error("invalid arguments: {message}")]
    InvalidArguments { message: String },

    #[error("backend startup failed: {0}")]
    BackendStartup(#[from] BackendStartupError),

    #[error("LSP error for {language}: {message}")]
    LspHost { language: String, message: String },

    #[error("unsupported language for extension: {extension}")]
    UnsupportedLanguage { extension: String },
}
```

Implement constructors and ensure `exit_status()` returns 1 for all new
variants.

______________________________________________________________________

### Step 3: Create argument parsing module

**File:** `crates/weaverd/src/dispatch/observe/arguments.rs`

```rust
//! Argument parsing for observe domain operations.

use lsp_types::{GotoDefinitionParams, Position, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri};
use weaver_lsp_host::Language;

use crate::dispatch::errors::DispatchError;

/// Parsed arguments for `get-definition` operation.
#[derive(Debug, Clone)]
pub struct GetDefinitionArgs {
    pub uri: Uri,
    pub line: u32,
    pub column: u32,
}

impl GetDefinitionArgs {
    /// Parses arguments from CLI argument list.
    ///
    /// Expected format: `--uri <URI> --position <LINE:COL>`
    pub fn parse(arguments: &[String]) -> Result<Self, DispatchError> {
        // Implementation: iterate through arguments, extract --uri and --position
        // Return InvalidArguments error for missing/malformed arguments
    }

    /// Infers language from the URI's file extension.
    pub fn language(&self) -> Result<Language, DispatchError> {
        // Extract extension from URI path
        // Map .rs -> Rust, .py -> Python, .ts/.tsx -> TypeScript
        // Return UnsupportedLanguage for unknown extensions
    }

    /// Converts to LSP GotoDefinitionParams.
    pub fn into_params(self) -> GotoDefinitionParams {
        GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: self.uri },
                position: Position {
                    line: self.line.saturating_sub(1),  // 1-indexed to 0-indexed
                    character: self.column.saturating_sub(1),
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }
    }
}
```

Add unit tests for:

- Valid argument parsing
- Missing `--uri` error
- Missing `--position` error
- Malformed position format (e.g., `10` instead of `10:5`)
- Language inference for each supported extension
- Unsupported extension error

______________________________________________________________________

### Step 4: Create response serialization module

**File:** `crates/weaverd/src/dispatch/observe/responses.rs`

```rust
//! Response types for observe domain operations.

use lsp_types::{GotoDefinitionResponse, Location, LocationLink};
use serde::Serialize;

/// A definition location in the response format.
#[derive(Debug, Clone, Serialize)]
pub struct DefinitionLocation {
    pub uri: String,
    pub line: u32,
    pub column: u32,
}

impl From<&Location> for DefinitionLocation {
    fn from(loc: &Location) -> Self {
        Self {
            uri: loc.uri.to_string(),
            line: loc.range.start.line + 1,      // 0-indexed to 1-indexed
            column: loc.range.start.character + 1,
        }
    }
}

impl From<&LocationLink> for DefinitionLocation {
    fn from(link: &LocationLink) -> Self {
        Self {
            uri: link.target_uri.to_string(),
            line: link.target_selection_range.start.line + 1,
            column: link.target_selection_range.start.character + 1,
        }
    }
}

/// Extracts definition locations from an LSP response.
pub fn extract_locations(response: GotoDefinitionResponse) -> Vec<DefinitionLocation> {
    match response {
        GotoDefinitionResponse::Scalar(loc) => vec![DefinitionLocation::from(&loc)],
        GotoDefinitionResponse::Array(locs) => {
            locs.iter().map(DefinitionLocation::from).collect()
        }
        GotoDefinitionResponse::Link(links) => {
            links.iter().map(DefinitionLocation::from).collect()
        }
    }
}
```

Add unit tests for each `GotoDefinitionResponse` variant.

______________________________________________________________________

### Step 5: Create semantic backend provider

**File:** `crates/weaverd/src/semantic_provider.rs`

```rust
//! Semantic backend provider managing the LSP host.

use std::sync::{Arc, Mutex};

use weaver_config::{CapabilityMatrix, Config};
use weaver_lsp_host::{Language, LspHost};

use crate::backends::{BackendKind, BackendProvider, BackendStartupError};

/// Backend provider that manages the LSP host for semantic operations.
pub struct SemanticBackendProvider {
    capability_matrix: CapabilityMatrix,
    lsp_host: Arc<Mutex<Option<LspHost>>>,
}

impl SemanticBackendProvider {
    pub fn new(capability_matrix: CapabilityMatrix) -> Self {
        Self {
            capability_matrix,
            lsp_host: Arc::new(Mutex::new(None)),
        }
    }

    /// Returns access to the LSP host, if initialised.
    pub fn lsp_host(&self) -> Arc<Mutex<Option<LspHost>>> {
        Arc::clone(&self.lsp_host)
    }
}

impl BackendProvider for SemanticBackendProvider {
    fn start_backend(&self, kind: BackendKind, _config: &Config)
        -> Result<(), BackendStartupError>
    {
        match kind {
            BackendKind::Semantic => {
                let mut guard = self.lsp_host.lock()
                    .map_err(|_| BackendStartupError::new(kind, "lock poisoned"))?;
                if guard.is_none() {
                    *guard = Some(LspHost::new(self.capability_matrix.clone()));
                }
                Ok(())
            }
            _ => Ok(()), // Other backends not yet implemented
        }
    }
}
```

______________________________________________________________________

### Step 6: Create get-definition handler

**File:** `crates/weaverd/src/dispatch/observe/get_definition.rs`

```rust
//! Handler for the `observe get-definition` operation.

use std::io::Write;

use tracing::debug;

use crate::backends::{BackendKind, BackendProvider, FusionBackends};
use crate::dispatch::errors::DispatchError;
use crate::dispatch::request::CommandRequest;
use crate::dispatch::response::ResponseWriter;
use crate::dispatch::router::{DISPATCH_TARGET, DispatchResult};
use crate::semantic_provider::SemanticBackendProvider;

use super::arguments::GetDefinitionArgs;
use super::responses::extract_locations;

/// Handles the `observe get-definition` command.
pub fn handle<W: Write>(
    request: &CommandRequest,
    writer: &mut ResponseWriter<W>,
    backends: &mut FusionBackends<SemanticBackendProvider>,
) -> Result<DispatchResult, DispatchError> {
    // 1. Parse arguments
    let args = GetDefinitionArgs::parse(request.arguments())?;
    let language = args.language()?;

    debug!(
        target: DISPATCH_TARGET,
        uri = %args.uri,
        line = args.line,
        column = args.column,
        language = %language,
        "handling get-definition"
    );

    // 2. Ensure semantic backend is started
    backends.ensure_started(BackendKind::Semantic)?;

    // 3. Get LSP host and call goto_definition
    let lsp_host_arc = backends.provider_mut().lsp_host();
    let mut lsp_guard = lsp_host_arc.lock()
        .map_err(|_| DispatchError::lsp_host(language, "lock poisoned"))?;
    let lsp_host = lsp_guard.as_mut()
        .ok_or_else(|| DispatchError::lsp_host(language, "host not initialised"))?;

    // 4. Ensure language is initialised
    lsp_host.initialise(language)
        .map_err(|e| DispatchError::lsp_host_from(language, e))?;

    // 5. Call goto_definition
    let params = args.into_params();
    let response = lsp_host.goto_definition(language, params)
        .map_err(|e| DispatchError::lsp_host_from(language, e))?;

    // 6. Serialize response
    let locations = extract_locations(response);
    let json = serde_json::to_string(&locations)?;
    writer.write_stdout(json)?;

    Ok(DispatchResult::success())
}
```

______________________________________________________________________

### Step 7: Create observe module

**File:** `crates/weaverd/src/dispatch/observe/mod.rs`

```rust
//! Handlers for the `observe` domain.
//!
//! This module contains operation handlers for querying the codebase,
//! including definition lookup, reference finding, and structural search.

pub mod arguments;
pub mod get_definition;
pub mod responses;
```

Update `crates/weaverd/src/dispatch/mod.rs` to include the new module:

```rust
pub mod observe;
```

______________________________________________________________________

### Step 8: Wire router to observe handler

**File:** `crates/weaverd/src/dispatch/router.rs`

Modify `DomainRouter` to accept backends and dispatch to handlers:

```rust
use crate::backends::FusionBackends;
use crate::semantic_provider::SemanticBackendProvider;
use super::observe;

impl DomainRouter {
    pub fn route<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
        backends: &mut FusionBackends<SemanticBackendProvider>,
    ) -> Result<DispatchResult, DispatchError> {
        let domain = Domain::parse(request.domain())?;
        // ... existing logging ...

        match domain {
            Domain::Observe => self.route_observe(request, writer, backends),
            Domain::Act => self.route_act(request, writer),
            Domain::Verify => self.route_verify(request, writer),
        }
    }

    fn route_observe<W: Write>(
        &self,
        request: &CommandRequest,
        writer: &mut ResponseWriter<W>,
        backends: &mut FusionBackends<SemanticBackendProvider>,
    ) -> Result<DispatchResult, DispatchError> {
        let operation = request.operation().to_ascii_lowercase();
        match operation.as_str() {
            "get-definition" => observe::get_definition::handle(request, writer, backends),
            _ if DomainRoutingContext::OBSERVE.known_operations
                    .contains(&operation.as_str()) => {
                self.write_not_implemented(writer, "observe", &operation)
            }
            _ => Err(DispatchError::unknown_operation("observe", operation)),
        }
    }
}
```

______________________________________________________________________

### Step 9: Wire handler to use backends

**File:** `crates/weaverd/src/dispatch/handler.rs`

Update `DispatchConnectionHandler` to hold and pass backends:

```rust
use std::sync::{Arc, Mutex};
use crate::backends::FusionBackends;
use crate::semantic_provider::SemanticBackendProvider;

pub struct DispatchConnectionHandler {
    router: DomainRouter,
    backends: Arc<Mutex<FusionBackends<SemanticBackendProvider>>>,
}

impl DispatchConnectionHandler {
    pub fn new(backends: Arc<Mutex<FusionBackends<SemanticBackendProvider>>>) -> Self {
        Self {
            router: DomainRouter::new(),
            backends,
        }
    }

    fn dispatch(&self, mut stream: ConnectionStream) {
        // ... existing request reading ...

        // Acquire backends lock for routing
        let mut backends_guard = match self.backends.lock() {
            Ok(g) => g,
            Err(_) => {
                let mut writer = ResponseWriter::new(&mut stream);
                let _ = writer.write_error(
                    &DispatchError::internal("backends lock poisoned")
                );
                return;
            }
        };

        match self.router.route(&request, &mut writer, &mut backends_guard) {
            // ... existing result handling ...
        }
    }
}
```

______________________________________________________________________

### Step 10: Update daemon bootstrap

**File:** `crates/weaverd/src/process/launch.rs` (or appropriate bootstrap file)

Create `SemanticBackendProvider` during daemon startup:

```rust
use crate::semantic_provider::SemanticBackendProvider;

// In bootstrap function:
let provider = SemanticBackendProvider::new(config.capability_matrix().clone());
let backends = Arc::new(Mutex::new(FusionBackends::new(config.clone(), provider)));
let handler = Arc::new(DispatchConnectionHandler::new(Arc::clone(&backends)));
```

______________________________________________________________________

### Step 11: Add BDD feature scenarios

**File:** `crates/weaverd/tests/features/daemon_dispatch.feature`

Update the scenario for `observe get-definition` to test argument validation:

```gherkin
Scenario: Observe get-definition without arguments returns error
  Given a daemon connection is established
  When a valid observe get-definition request is sent
  Then the response includes an exit message with status 1
  And the response includes an invalid arguments error
```

______________________________________________________________________

### Step 12: Implement BDD step definitions

**File:** `crates/weaverd/src/tests/dispatch_behaviour.rs`

Add step definitions and update the world fixture to use the semantic backend
provider:

```rust
use crate::backends::FusionBackends;
use crate::semantic_provider::SemanticBackendProvider;

fn start_listener(&mut self) {
    let config = Config { /* ... */ };
    let provider = SemanticBackendProvider::new(CapabilityMatrix::default());
    let backends = Arc::new(Mutex::new(FusionBackends::new(config, provider)));
    let handler = Arc::new(DispatchConnectionHandler::new(backends));
    // ... rest of listener setup
}

#[then("the response includes an invalid arguments error")]
fn then_invalid_arguments_error(world: &RefCell<DispatchWorld>) {
    assert!(
        world.borrow().has_invalid_arguments_error(),
        "expected invalid arguments error, got: {:?}",
        world.borrow().response_lines
    );
}
```

______________________________________________________________________

### Step 13: Update documentation

**File:** `docs/users-guide.md`

Update the daemon lifecycle section to note that `observe get-definition` is
fully implemented. Update the `observe get-definition` command reference with:

- Exact argument requirements (`--uri` and `--position` both required)
- Response format (JSON array of locations written to stdout stream)
- Error handling for missing arguments and unsupported languages

**File:** `docs/roadmap.md`

Mark the task as complete:

```markdown
- [x] Wire end-to-end domain command execution from CLI through daemon to
      backend, starting with `observe get-definition` as the first complete
      path.
```

______________________________________________________________________

### Step 14: Run quality gates

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

Fix any issues before committing.

______________________________________________________________________

## Dependency Graph

```text
Step 1 (stdout target) ────┐
                           │
Step 2 (dispatch errors) ──┤
                           │
Step 3 (argument parsing) ─┼──> Step 6 (handler) ──> Step 8 (router wiring)
                           │         │
Step 4 (responses) ────────┤         │
                           │         v
Step 5 (semantic provider) ┴──> Step 9 (handler wiring) ──> Step 10 (bootstrap)

Step 7 (observe module) depends on Steps 3, 4, 6

Steps 11-12 (tests) can proceed after Step 10

Steps 13-14 (docs, gates) are final
```

______________________________________________________________________

## Test Plan

### Unit Tests

- `GetDefinitionArgs::parse()` with valid/invalid arguments
- `GetDefinitionArgs::language()` for each extension
- `extract_locations()` for each `GotoDefinitionResponse` variant
- `ResponseWriter::write_stdout()` message format

### BDD Tests

- Argument validation: missing `--uri` or `--position` returns error
- Unsupported language error for unknown file extensions

### E2E Tests

- Full CLI → daemon → mock LSP → response flow
- Verify CLI exit code matches daemon status

______________________________________________________________________

## Risks and Mitigations

| Risk                               | Mitigation                           |
| ---------------------------------- | ------------------------------------ |
| LSP host initialisation slow       | Lazy initialisation already in place |
| Thread safety with shared backends | Use `Arc<Mutex<>>` pattern           |
| Argument parsing edge cases        | Comprehensive unit tests             |
| Response format mismatch           | Review docs before implementation    |
