# Phase 1: JSONL Request Dispatch Loop

## Goal

Implement the JSONL request dispatch loop in `weaverd` that reads `CommandRequest`
messages from connected clients, routes them to the appropriate domain handler,
and streams `CommandResponse` messages back.

## Acceptance Criteria

- Request parsing rejects malformed JSONL with structured errors
- Domain routing covers `observe` and `act` commands
- Responses include the terminal `exit` message with appropriate status codes

## Design Decisions

1. **Modular dispatch architecture**: Create a new `dispatch` module with clear
   separation of concerns (errors, request parsing, response writing, routing,
   and connection handling). Each file remains under 400 lines per AGENTS.md.

2. **MVP placeholder handlers**: Known operations return "not implemented"
   responses with exit status 1. This establishes the routing infrastructure
   without requiring full backend integration.

3. **Structured error responses**: All errors (malformed JSONL, unknown domain,
   unknown operation) are written to stderr as structured messages before the
   terminal exit message.

4. **Case-insensitive domain matching**: Domain names (`observe`, `act`,
   `verify`) are matched case-insensitively for robustness.

5. **Re-use existing transport infrastructure**: Build on the existing
   `ConnectionHandler` trait and `read_request_line` helper from the transport
   module.

## Module Structure

```
crates/weaverd/src/
  dispatch/
    mod.rs           # Module exports (~30 lines)
    errors.rs        # DispatchError enum (~90 lines)
    request.rs       # CommandRequest deserialization (~130 lines)
    response.rs      # DaemonMessage serialization (~130 lines)
    router.rs        # DomainRouter and operation routing (~220 lines)
    handler.rs       # DispatchConnectionHandler (~180 lines)
```

## Implementation Steps

### Step 1: Create dispatch/errors.rs

Define `DispatchError` enum with variants:
- `MalformedJsonl { message, source }` - Invalid JSON
- `InvalidStructure { message }` - Missing/empty fields
- `UnknownDomain { domain }` - Unrecognised domain
- `UnknownOperation { domain, operation }` - Unrecognised operation
- `Io(io::Error)` - IO failures
- `SerializeResponse(serde_json::Error)` - Response serialization

Implement `exit_status()` returning non-zero codes for all variants.

### Step 2: Create dispatch/request.rs

Define `CommandRequest` and `CommandDescriptor` structs with serde derive.
Implement:
- `CommandRequest::parse(line: &[u8]) -> Result<Self, DispatchError>`
- `CommandRequest::validate() -> Result<(), DispatchError>`
- Helper `trim_trailing_whitespace(bytes: &[u8]) -> &[u8]`

### Step 3: Create dispatch/response.rs

Define `DaemonMessage` enum:
```rust
#[serde(tag = "kind", rename_all = "snake_case")]
enum DaemonMessage {
    Stream { stream: StreamTarget, data: String },
    Exit { status: i32 },
}
```

Implement `ResponseWriter<W: Write>` with methods:
- `write_message(&mut self, msg: &DaemonMessage)`
- `write_stdout(&mut self, data: impl Into<String>)`
- `write_stderr(&mut self, data: impl Into<String>)`
- `write_exit(&mut self, status: i32)`
- `write_error(&mut self, error: &DispatchError)`

### Step 4: Create dispatch/router.rs

Define `Domain` enum (`Observe`, `Act`, `Verify`) with `parse()` method.

Implement `DomainRouter` with:
- `route(&self, request, writer) -> Result<DispatchResult, DispatchError>`
- `route_observe()` - Routes known observe operations
- `route_act()` - Routes known act operations
- `route_verify()` - Routes known verify operations

Known operations (MVP returns "not implemented"):
- **observe**: `get-definition`, `find-references`, `grep`, `diagnostics`,
  `call-hierarchy`
- **act**: `rename-symbol`, `apply-edits`, `apply-patch`, `apply-rewrite`,
  `refactor`
- **verify**: `diagnostics`, `syntax`

### Step 5: Create dispatch/handler.rs

Implement `DispatchConnectionHandler`:
```rust
impl ConnectionHandler for DispatchConnectionHandler {
    fn handle(&self, stream: ConnectionStream) {
        // 1. Read request line
        // 2. Parse into CommandRequest
        // 3. Validate request
        // 4. Route to domain handler
        // 5. Write exit message
    }
}
```

Use tracing for structured logging at `debug!` and `warn!` levels.

### Step 6: Create dispatch/mod.rs

Export public types and declare the module structure.

### Step 7: Wire into launch sequence

Modify `crates/weaverd/src/process/launch.rs`:
- Add `use crate::dispatch::DispatchConnectionHandler;`
- Replace: `let handler = Arc::new(NoopConnectionHandler);`
- With: `let handler = Arc::new(DispatchConnectionHandler::new());`

Modify `crates/weaverd/src/lib.rs`:
- Add `mod dispatch;`

### Step 8: Create BDD feature file

File: `crates/weaverd/tests/features/daemon_dispatch.feature`

```gherkin
Feature: Daemon JSONL request dispatch

  Scenario: Dispatching a valid observe command
    Given a daemon connection is established
    When a valid observe get-definition request is sent
    Then the response includes an exit message with status 1
    And the response includes a not implemented message

  Scenario: Rejecting malformed JSONL
    Given a daemon connection is established
    When a malformed JSONL request is sent
    Then the response includes an error message
    And the response includes an exit message with status 1

  Scenario: Rejecting unknown domain
    Given a daemon connection is established
    When a request with unknown domain "bogus" is sent
    Then the response includes an unknown domain error
    And the response includes an exit message with status 1

  Scenario: Rejecting unknown operation
    Given a daemon connection is established
    When a request with unknown operation "nonexistent" in domain "observe" is sent
    Then the response includes an unknown operation error
    And the response includes an exit message with status 1

  Scenario: Dispatching a valid act command
    Given a daemon connection is established
    When a valid act apply-patch request is sent
    Then the response includes an exit message with status 1
    And the response includes a not implemented message
```

### Step 9: Create BDD step definitions

File: `crates/weaverd/src/tests/dispatch_behaviour.rs`

Implement:
- `DispatchWorld` struct with listener, address, response storage
- Fixture: `world() -> RefCell<DispatchWorld>`
- Given steps: connection establishment
- When steps: send various request types
- Then steps: verify response content and exit codes
- Scenario binding with `#[scenario]` macro

Update `crates/weaverd/src/tests/mod.rs` to include `dispatch_behaviour`.

### Step 10: Update user's guide

Modify `docs/users-guide.md` to update the description of the daemon request
handling:
- Request parsing validates JSONL structure
- Domain routing supports `observe`, `act`, `verify`
- Unknown domains/operations return structured errors
- Known operations return "not implemented" status (pending backend wiring)

### Step 11: Mark roadmap entry as done

Update `docs/roadmap.md`, changing:
```
- [ ] Implement the JSONL request dispatch loop...
```
To:
```
- [x] Implement the JSONL request dispatch loop...
```

### Step 12: Run quality gates

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

## Files Modified

| File | Action |
|------|--------|
| `crates/weaverd/src/dispatch/mod.rs` | Created |
| `crates/weaverd/src/dispatch/errors.rs` | Created |
| `crates/weaverd/src/dispatch/request.rs` | Created |
| `crates/weaverd/src/dispatch/response.rs` | Created |
| `crates/weaverd/src/dispatch/router.rs` | Created |
| `crates/weaverd/src/dispatch/handler.rs` | Created |
| `crates/weaverd/src/lib.rs` | Modified (add `mod dispatch;`) |
| `crates/weaverd/src/process/launch.rs` | Modified (use DispatchConnectionHandler) |
| `crates/weaverd/tests/features/daemon_dispatch.feature` | Created |
| `crates/weaverd/src/tests/dispatch_behaviour.rs` | Created |
| `crates/weaverd/src/tests/mod.rs` | Modified (add `mod dispatch_behaviour;`) |
| `docs/users-guide.md` | Modified (update daemon description) |
| `docs/roadmap.md` | Modified (mark entry done) |
| `docs/execplans/phase-1-jsonl-request-dispatch-loop.md` | Created |

## Dependencies

- `serde` + `serde_json` - Already in workspace
- `thiserror` - Already in workspace
- `tracing` - Already in workspace
- `rstest` - Already in dev-dependencies
- `rstest-bdd-macros` v0.3.2 - Already in dev-dependencies
