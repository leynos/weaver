# ExecPlan: Process-Based Language Server Adapters

## Metadata

| Field             | Value                        |
| ----------------- | ---------------------------- |
| **Status**        | Complete                     |
| **Created**       | 2026-01-21                   |
| **Target**        | Phase 1 Minimum Viable Product (MVP) |
| **Roadmap Entry** | `docs/roadmap.md` line 79-87 |

## Big Picture

Implement process-based language server adapters for `weaver-lsp-host` that
spawn real language server processes (`rust-analyzer`, `pyrefly lsp`,
`tsgo --lsp`) and communicate via JavaScript Object Notation – Remote Procedure
Call (JSON-RPC) 2.0 over stdio. This replaces the current `StubLanguageServer`
with production-ready adapters.

## Constraints

1. **Must implement `Send`**: Required by `LanguageServer` trait bound
2. **Synchronous interface**: All trait methods block on process input/output
   (I/O)
3. **Single initialization**: `initialize()` called once per language, result
   cached by `LspHost`
4. **Graceful degradation**: Missing binaries produce clear diagnostic errors,
   not crashes
5. **Zero-trust sandbox**: Language servers execute in restricted sandbox
   (future integration)

## Acceptance Criteria

1. `SemanticBackendProvider::start_backend()` registers adapters for configured
   languages
2. Adapters spawn server processes and communicate via stdio
3. Server shutdown is handled gracefully on daemon stop
4. Missing server binaries produce clear diagnostic errors

## Implementation Tasks

### Task 1: Create adapter module structure

**Files to create:**

- `crates/weaver-lsp-host/src/adapter/mod.rs`
- `crates/weaver-lsp-host/src/adapter/error.rs`
- `crates/weaver-lsp-host/src/adapter/jsonrpc.rs`
- `crates/weaver-lsp-host/src/adapter/transport.rs`
- `crates/weaver-lsp-host/src/adapter/config.rs`
- `crates/weaver-lsp-host/src/adapter/process.rs`

**Files to modify:**

- `crates/weaver-lsp-host/src/lib.rs` - Add `pub mod adapter;`
- `crates/weaver-lsp-host/Cargo.toml` - Add `serde`, `serde_json`, `tracing`
  dependencies

**Verification:** `make check-fmt && make lint`

______________________________________________________________________

### Task 2: Implement adapter error types (`adapter/error.rs`)

Define error hierarchy for adapter operations.

**Verification:** Unit tests compile and pass

______________________________________________________________________

### Task 3: Implement JSON-RPC codec (`adapter/jsonrpc.rs`)

Implement JSON-RPC 2.0 message types.

**Verification:** `make test` passes JSON-RPC unit tests

______________________________________________________________________

### Task 4: Implement stdio transport (`adapter/transport.rs`)

Implement Language Server Protocol (LSP) header-framed transport.

**Verification:** `make test` passes transport unit tests

______________________________________________________________________

### Task 5: Implement server configuration (`adapter/config.rs`)

**Design decision:** Default configurations are hardcoded. Future work may add
config file support.

**Verification:** `make check-fmt && make lint`

______________________________________________________________________

### Task 6: Implement ProcessLanguageServer (`adapter/process.rs`)

Core adapter implementing `LanguageServer` trait.

**Verification:** Compiles with `LanguageServer` trait bounds satisfied

______________________________________________________________________

### Task 7: Implement dependency injection for testing

Create `ProcessSpawner` trait for testability.

**Verification:** Mock spawner can inject responses for testing

______________________________________________________________________

### Task 8: Write Behaviour-Driven Development (BDD) tests for adapter lifecycle

**Feature file:**
`crates/weaver-lsp-host/tests/features/process_adapter.feature`

**Verification:** `make test` passes all BDD scenarios

______________________________________________________________________

### Task 9: Write unit tests for edge cases

**Verification:** `make test` passes all unit tests

______________________________________________________________________

### Task 10: Integrate with SemanticBackendProvider

Replace `StubLanguageServer` with `ProcessLanguageServer`.

**Verification:** `make test` passes all existing daemon tests

______________________________________________________________________

### Task 11: Implement graceful shutdown integration

**Verification:** Manual test - start daemon, stop with the POSIX termination
signal SIGTERM, verify child processes exit

______________________________________________________________________

### Task 12: Update documentation

**File:** `docs/users-guide.md`

**Verification:** Documentation renders correctly

______________________________________________________________________

### Task 13: Mark roadmap entry complete

**File:** `docs/roadmap.md`

**Verification:** Roadmap reflects completion

______________________________________________________________________

### Task 14: Final verification

```bash
make check-fmt
make lint
make test
```

All must pass before committing.

## Critical Files

| File                                                          | Purpose                    |
| ------------------------------------------------------------- | --------------------------- |
| `crates/weaver-lsp-host/src/server.rs`                          | `LanguageServer` trait definition |
| `crates/weaver-lsp-host/src/adapter/process.rs`                 | Main adapter implementation |
| `crates/weaverd/src/semantic_provider/mod.rs`                   | Integration point |
| `crates/weaver-lsp-host/tests/features/process_adapter.feature` | BDD scenarios |
| `docs/users-guide.md`                                           | User documentation |

## Design Decisions

### D1: Synchronous blocking I/O

**Decision:** Use blocking I/O for JSON-RPC communication rather than async.

**Rationale:** The `LanguageServer` trait methods are synchronous
(`&mut self`). Introducing async would require significant trait redesign.
Blocking is acceptable because:

- Each language has one server instance
- `LspHost` serializes access via session management
- Request latency is dominated by server processing, not I/O

### D2: Single process per language

**Decision:** One language server process per `Language` variant.

**Rationale:** Matches existing `LspHost` architecture where each language has
one registered server.

### D3: Hardcoded default configurations

**Decision:** Default server commands are hardcoded in
`LspServerConfig::for_language()`.

**Rationale:** Simplifies MVP. Configuration file support for custom paths is
future work.

### D4: Best-effort shutdown in Drop

**Decision:** `Drop` implementation attempts graceful shutdown but ignores
errors.

**Rationale:** Destructors cannot propagate errors. Tracing captures failures.
Process will be killed on timeout regardless.

### D5: Pull-based diagnostics

**Decision:** Implement `diagnostics()` using `textDocument/diagnostic` pull
model.

**Rationale:** Push-based `publishDiagnostics` requires notification handling
infrastructure. Pull model is simpler and sufficient for MVP.

## Risks and Mitigations

| Risk                          | Mitigation                                              |
| ----------------------------- | ------------------------------------------------------- |
| Language server not installed | Clear error message with installation hint              |
| Server crashes mid-session    | `ProcessExited` error propagates; daemon remains stable |
| Slow server initialization    | Configurable timeout; default 30s                       |
| JSON-RPC protocol mismatch    | Comprehensive unit tests for codec                      |

## Progress Log

| Date       | Update                                                                                                                                                                                                                      |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2026-01-21 | Plan created                                                                                                                                                                                                                |
| 2026-01-21 | Implementation started                                                                                                                                                                                                      |
| 2026-01-21 | All tasks complete: adapter module with error types, JSON-RPC codec, transport, config, ProcessLanguageServer; integrated with SemanticBackendProvider; BDD tests added; documentation and roadmap updated; all checks pass |
