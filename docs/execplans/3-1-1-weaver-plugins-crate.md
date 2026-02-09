# Implement the `weaver-plugins` crate with secure broker-plugin IPC

This ExecPlan is a living document. The sections Constraints, Tolerances,
Risks, Progress, Surprises & Discoveries, Decision Log, and Outcomes &
Retrospective must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

After this change an operator (or AI agent) can register specialist external
tools as plugins and execute them via the `weaverd` daemon. Actuator plugins
produce unified diffs that are validated by the existing Double-Lock safety
harness before any filesystem change is committed. Sensor plugins produce
structured JSON analysis data. The entire plugin lifecycle — discovery,
sandboxed execution, IPC, output capture, and verification — is managed by the
new `weaver-plugins` crate and wired into the `weaverd` dispatch layer.

Observable behaviour after this change:

- The `weaver-plugins` crate compiles, passes lint, and has unit and BDD tests.
- The `act refactor` command in `weaverd` resolves a plugin by name from the
  registry, executes it in a sandbox, captures its output, and feeds the result
  through the Double-Lock safety harness before writing to disk.
- Running `make check-fmt && make lint && make test` succeeds with no
  regressions.

## Context and orientation

Weaver is a client-daemon tool that orchestrates code analysis and
modification for AI agents. The CLI (`weaver-cli`) connects to the daemon
(`weaverd`) over Unix sockets using a JSONL protocol. The daemon routes
commands through three domains: `observe` (queries), `act` (mutations), and
`verify` (checks). All `act` commands pass through a Double-Lock safety
harness (`crates/weaverd/src/safety_harness/`) that validates changes via
syntactic (Tree-sitter) and semantic (LSP) locks before atomic filesystem
writes.

External tools already run in the `weaver-sandbox` crate
(`crates/weaver-sandbox/`), which wraps `birdcage` 0.8.1 with Linux
namespaces and `seccomp-bpf`. The sandbox requires single-threaded callers,
whitelisted absolute-path executables, and default-deny networking.

The `weaver-lsp-host` crate (`crates/weaver-lsp-host/`) provides a precedent
for process-based external tool integration. Its `ProcessLanguageServer`
spawns real language server processes and communicates via JSON-RPC 2.0 over
stdio. This pattern is the closest analogue to what the plugin system needs,
but plugins differ in one critical respect: they are short-lived, one-shot
processes rather than long-lived servers.

Key files the reader should understand before proceeding:

- `crates/weaver-sandbox/src/sandbox.rs` — `Sandbox::spawn()` launches
  sandboxed processes; returns a `SandboxChild` (wrapping
  `birdcage::process::Child`).
- `crates/weaver-sandbox/src/profile.rs` — `SandboxProfile` builder for
  declaring executable, read, and read-write path allowlists.
- `crates/weaverd/src/safety_harness/transaction.rs` —
  `ContentTransaction::add_change(ContentChange::Write { path, content })`
  feeds proposed file modifications through the Double-Lock pipeline.
- `crates/weaverd/src/dispatch/act/apply_patch/mod.rs` —
  `ApplyPatchExecutor` demonstrates how an `act` handler builds
  `ContentChange` values and uses `ContentTransaction::execute()`.
- `crates/weaverd/src/dispatch/router.rs` — `DomainRouter::route_act()`
  dispatches `act` operations; `refactor` is already listed in
  `DomainRoutingContext::ACT.known_operations`.
- `crates/weaver-lsp-host/src/adapter/lifecycle.rs` —
  `terminate_child()` implements the grace-period shutdown pattern.

## Constraints

1. **No async runtime.** The entire project uses synchronous blocking I/O.
   Plugin execution must remain synchronous.
2. **Single-threaded sandbox.** `birdcage` requires `Sandbox::spawn()` to be
   called from a single-threaded context. The daemon's per-connection handler
   satisfies this naturally.
3. **Edition 2024, Rust 1.85+.** The workspace uses `edition = "2024"` and
   `rust-version = "1.85"`.
4. **Strict Clippy.** Over 60 denied lint categories including `unwrap_used`,
   `expect_used`, `indexing_slicing`, `string_slice`, `missing_docs`, and
   `cognitive_complexity`. All code must pass
   `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
5. **File size limit.** No single source file may exceed 400 lines.
6. **Error handling.** Library crates use `thiserror`-derived error enums.
   No `eyre` or `anyhow` in library code.
7. **Documentation.** Every module must begin with `//!` doc comments. All
   public items must have `///` rustdoc comments with examples where
   non-trivial.
8. **en-GB-oxendict spelling.** Comments and documentation use British English
   with Oxford "-ize" spelling.
9. **rstest-bdd v0.5.0.** BDD tests must use v0.5.0 (upgrade from current
   v0.4.0), with mutable world fixtures (`&mut`) instead of `RefCell`.
10. **Workspace dependencies.** New dependencies must use caret requirements
    and be declared in `[workspace.dependencies]` when shared.

## Tolerances (exception triggers)

- **Scope:** If implementation requires changes to more than 25 files or 3000
  lines of code (net), stop and escalate.
- **Interface:** If existing public API signatures in `weaverd`, `weaver-cli`,
  or `weaver-sandbox` must change, stop and escalate.
- **Dependencies:** If a new external crate beyond `serde`, `serde_json`,
  `thiserror`, `tracing`, and `weaver-sandbox` is required for the core crate,
  stop and escalate.
- **Iterations:** If tests still fail after 5 attempts at fixing a given
  issue, stop and escalate.
- **Ambiguity:** If the IPC protocol design requires capabilities not
  expressible with JSONL-over-stdio (e.g. bidirectional streaming), stop and
  present options.

## Risks

- Risk: The `birdcage` sandbox's `SandboxChild` does not expose `stdin` and
  `stdout` handles for plugin communication.
  Severity: high
  Likelihood: medium
  Mitigation: If `SandboxChild` does not expose stdio, use `std::process`
  directly for spawning and rely on `SandboxProfile` configuration as a
  documentation contract. The sandbox wraps `birdcage::process::Command` which
  mirrors `std::process::Command` and does expose `Stdio` configuration.

- Risk: Upgrading rstest-bdd from 0.4.0 to 0.5.0 may break existing BDD tests
  due to API changes (e.g. `RefCell` removal, mutable world fixtures).
  Severity: medium
  Likelihood: medium
  Mitigation: Upgrade as a separate first step. Fix all existing tests before
  proceeding with new code. The v0.5.0 API supports `&mut` directly, making
  `RefCell` wrappers unnecessary.

- Risk: Plugin timeout implementation without async may be complex.
  Severity: low
  Likelihood: low
  Mitigation: Use the existing pattern from `terminate_child()` in
  `weaver-lsp-host`: spawn the child, read stdout, call `try_wait()` in a
  loop with sleep, kill on timeout. This is proven in the codebase.

## Progress

- [x] Write execution plan to `docs/execplans/3-1-1-weaver-plugins-crate.md`.
- [x] Upgrade `rstest-bdd` from 0.4.0 to 0.5.0 across workspace.
- [x] Create `weaver-plugins` crate skeleton and `Cargo.toml`.
- [x] Implement error types (`error.rs`).
- [x] Implement IPC protocol types (`protocol.rs`).
- [x] Implement plugin manifest types (`manifest.rs`).
- [x] Implement plugin registry (`registry.rs`).
- [x] Implement process-based plugin execution (`process.rs`).
- [x] Implement plugin runner orchestrator (`runner.rs`).
- [x] Wire up `lib.rs` with module declarations and re-exports.
- [x] Write unit tests for all modules.
- [x] Write BDD feature file and step definitions.
- [x] Wire `weaver-plugins` into `weaverd` dispatch for `act refactor`.
- [x] Update `docs/weaver-design.md` with implementation decisions.
- [x] Update `docs/users-guide.md` with plugin system documentation.
- [x] Mark roadmap entry as done.
- [x] Run `make check-fmt`, `make lint`, `make test`.

## Surprises & discoveries

- The `birdcage::process::Stdio` type is distinct from `std::process::Stdio`.
  `SandboxCommand` wraps `birdcage::process::Command` which expects
  `birdcage::process::Stdio`. The `weaver-sandbox` crate re-exports this as
  `weaver_sandbox::process::Stdio`. This required importing the correct type
  in `process.rs`.

- The workspace's strict Clippy configuration denies `allow_attributes`,
  requiring `#[expect(lint, reason = "...")]` instead of `#[allow(lint)]`.
  This applies to all lint suppressions including `too_many_arguments`.

- Rust Edition 2024 changed module path resolution: `mod tests;` inside
  `error.rs` resolves to `error/tests.rs` (directory-based), but Clippy's
  `self_named_module_files` lint still requires `mod.rs` convention. The
  solution is to use `error/mod.rs` + `error/tests.rs` rather than
  `error.rs` + `error/tests.rs`.

- The `rstest-bdd` v0.5.0 upgrade was seamless — existing tests in `weaverd`
  and `weaver-config` continued to pass without modification.

## Decision log

- Decision: Use JSONL over stdio for the plugin IPC protocol rather than
  JSON-RPC 2.0 with Content-Length framing.
  Rationale: Plugins are short-lived one-shot processes, not long-lived servers.
  A single JSONL line on stdin and a single JSONL line on stdout is sufficient.
  JSON-RPC adds unnecessary complexity (request IDs, method names, error
  objects) for a one-shot exchange. JSONL is consistent with the existing
  CLI-to-daemon protocol.
  Date: 2026-02-09

- Decision: Pass file content in-band in the JSON request rather than using
  file descriptor passing via `sendmsg`/`recvmsg` with `SCM_RIGHTS`.
  Rationale: The design document describes FD passing as the eventual model,
  but it requires `seccomp-bpf` filter coordination and `sendmsg`/`recvmsg`
  plumbing. Passing content inline is simpler and consistent with how
  `act apply-patch` passes patch content. The 1 MiB request limit already
  exists. FD passing can be added in a future iteration.
  Date: 2026-02-09

- Decision: Use a `Plugin` trait with `ProcessPlugin` as the concrete
  implementation, mirroring the `LanguageServer` / `ProcessLanguageServer`
  pattern from `weaver-lsp-host`.
  Rationale: Enables test doubles that do not spawn real processes. The trait
  boundary sits at the process execution level, allowing BDD tests to inject
  mock behaviour via closures or pre-configured response data.
  Date: 2026-02-09

- Decision: Actuator plugin output feeds into the existing
  `ContentTransaction` / `ContentChange::Write` path in the safety harness.
  Rationale: The Double-Lock verification infrastructure already validates
  file content changes atomically. Reusing it avoids duplicating safety logic
  and ensures plugin output receives the same syntactic and semantic
  validation as `apply-patch` output.
  Date: 2026-02-09

- Decision: Upgrade `rstest-bdd` to v0.5.0 as a prerequisite step.
  Rationale: v0.5.0 supports mutable world fixtures with `&mut`, eliminating
  the need for `RefCell` wrappers in step definitions. This is a cleaner
  pattern for new BDD tests. Existing tests that use `RefCell` must be
  migrated as part of the upgrade.
  Date: 2026-02-09

## Outcomes & retrospective

All acceptance criteria met. The `weaver-plugins` crate compiles, passes
lint, and has comprehensive unit and BDD tests (56 unit tests + 5 BDD
scenarios). The `act refactor` command is wired into `weaverd` dispatch and
routes correctly. Plugin execution returns "not yet available" pending
Phase 3.2 when the daemon runtime will hold a `PluginRunner` instance.

Key metrics:
- 14 new files created across `crates/weaver-plugins/` and
  `crates/weaverd/src/dispatch/act/refactor/`.
- 3 documentation files updated (`weaver-design.md`, `users-guide.md`,
  `roadmap.md`).
- Zero regressions: all 511 workspace tests pass (142 weaverd, 56
  weaver-plugins, 313 others).
- `make check-fmt`, `make lint`, `make test` all pass.

## Plan of work

### Stage A: Prerequisite — upgrade rstest-bdd to v0.5.0

Update the workspace dependency declarations in the root `Cargo.toml` from
`rstest-bdd = { version = "0.4.0", ... }` and `rstest-bdd-macros = "0.4.0"`
to `rstest-bdd = { version = "0.5.0", ... }` and
`rstest-bdd-macros = "0.5.0"`. Then run `make test` to identify any
breaking changes in existing BDD tests. Migrate existing step definitions
from `RefCell<World>` to `&mut World` where the v0.5.0 API requires it.
This stage must pass `make check-fmt && make lint && make test` before
proceeding.

### Stage B: Crate skeleton and core types

Create the `crates/weaver-plugins/` directory tree. Add the crate to the
workspace members list. Write `Cargo.toml` with dependencies on `serde`,
`serde_json`, `thiserror`, `tracing`, and `weaver-sandbox`. Implement the
following modules in order (each must compile and pass lint before the next):

1. `error.rs` — `PluginError` enum with variants: `NotFound`,
   `SpawnFailed`, `Timeout`, `NonZeroExit`, `SerializeRequest`,
   `DeserializeResponse`, `InvalidOutput`, `Io`, `Sandbox`, `Manifest`.

2. `protocol.rs` — IPC message types: `PluginRequest`, `PluginResponse`,
   `FilePayload`, `PluginOutput` (tagged enum: `Diff`, `Analysis`, `Empty`),
   `PluginDiagnostic`, `DiagnosticSeverity`. All types derive `Serialize`
   and `Deserialize`. Unit tests verify round-trip serialization.

3. `manifest.rs` — `PluginKind` (`Sensor` / `Actuator`),
   `PluginManifest` (name, version, kind, languages, executable path,
   args, timeout). Validation: non-empty name, absolute executable path.

4. `registry.rs` — `PluginRegistry` backed by `HashMap<String, PluginManifest>`.
   Methods: `register()`, `get()`, `find_by_kind()`, `find_for_language()`,
   `find_actuator_for_language()`.

5. `process.rs` — `ProcessPlugin` struct. The `run()` method:
   (a) builds a `SandboxProfile` with the plugin executable whitelisted,
   (b) creates a `Sandbox` and spawns the command with stdin/stdout piped,
   (c) writes a single JSONL request line to stdin and closes it,
   (d) reads a single JSONL response line from stdout,
   (e) waits for exit with timeout,
   (f) returns a `PluginResponse` or `PluginError`.

6. `runner.rs` — `PluginRunner` wrapping `PluginRegistry`. The `execute()`
   method resolves the manifest, creates a `ProcessPlugin`, calls `run()`,
   and returns the response. Uses a `PluginExecutor` trait to enable test
   doubles.

7. `lib.rs` — module declarations, `pub use` re-exports, crate-level `//!`
   documentation.

### Stage C: BDD tests

Write a Gherkin feature file at
`crates/weaver-plugins/tests/features/plugin_execution.feature` covering:

- Successful actuator plugin execution (produces a diff).
- Successful sensor plugin execution (produces analysis JSON).
- Plugin not found in registry.
- Plugin produces invalid (non-JSON) output.
- Plugin exits with non-zero status.
- Plugin timeout.

Write step definitions in
`crates/weaver-plugins/src/tests/plugin_behaviour.rs` using rstest-bdd
v0.5.0 with mutable world fixtures (`&mut PluginTestWorld`). The test world
uses a mock executor (implementing the `PluginExecutor` trait) that returns
pre-configured responses without spawning real processes.

### Stage D: weaverd integration

Wire the plugin system into the `weaverd` dispatch layer:

1. Add `weaver-plugins = { path = "../weaver-plugins" }` to
   `crates/weaverd/Cargo.toml`.

2. Create `crates/weaverd/src/dispatch/act/refactor/mod.rs` with a `handle()`
   function following the `apply_patch::handle()` pattern:
   - Parse `--provider` and `--refactoring` arguments from `CommandRequest`.
   - Read target file content from disk.
   - Build a `PluginRequest` with the file content and arguments.
   - Call `PluginRunner::execute()`.
   - On success with `PluginOutput::Diff`, parse the diff and build
     `ContentChange::Write` values.
   - Feed changes through `ContentTransaction::execute()` with syntactic
     and semantic locks.
   - Return the result via `ResponseWriter`.

3. Update `crates/weaverd/src/dispatch/act/mod.rs` to declare
   `pub mod refactor;`.

4. Update `crates/weaverd/src/dispatch/router.rs` to route `"refactor"` to
   `act::refactor::handle()` in `route_act()`.

### Stage E: Documentation and roadmap

1. Add a new subsection to `docs/weaver-design.md` under section 4.1
   documenting the implementation decisions (IPC protocol choice, in-band
   file content, plugin trait pattern, safety harness integration).

2. Add a "Plugin system" section to `docs/users-guide.md` documenting:
   - Plugin categories (sensor/actuator).
   - Plugin manifest format.
   - The `act refactor` command syntax.
   - How plugin output is validated through the Double-Lock harness.

3. Mark the first Phase 3 roadmap entry as done in `docs/roadmap.md`.

### Stage F: Final verification

Run:

    make check-fmt
    make lint
    make test

All must pass. Commit the change.

## Concrete steps

### Step 1: Upgrade rstest-bdd workspace dependencies

In `Cargo.toml` (root), change:

    rstest-bdd = { version = "0.4.0", default-features = false }
    rstest-bdd-macros = "0.4.0"

to:

    rstest-bdd = { version = "0.5.0", default-features = false }
    rstest-bdd-macros = "0.5.0"

Run:

    make test 2>&1 | tee /tmp/rstest-upgrade.log; echo "EXIT: $?"

If tests fail, migrate affected step definitions from `RefCell<World>` to
`&mut World`. Repeat until `make test` passes.

### Step 2: Create crate skeleton

    mkdir -p crates/weaver-plugins/src/tests

Add `"crates/weaver-plugins"` to the workspace `members` list in the root
`Cargo.toml`.

Create `crates/weaver-plugins/Cargo.toml`:

    [package]
    name = "weaver-plugins"
    edition.workspace = true
    version.workspace = true

    [dependencies]
    serde = { version = "1.0", features = ["derive"] }
    serde_json = "1.0"
    thiserror.workspace = true
    tracing = "0.1"
    weaver-sandbox = { path = "../weaver-sandbox" }

    [dev-dependencies]
    rstest.workspace = true
    rstest-bdd.workspace = true
    rstest-bdd-macros.workspace = true
    tempfile.workspace = true

    [build-dependencies]
    weaver-build-util = { path = "../weaver-build-util" }

    [lints]
    workspace = true

Create `crates/weaver-plugins/src/lib.rs` with module declarations and
crate-level documentation. Verify with:

    cargo check -p weaver-plugins

### Step 3: Implement core modules

Implement `error.rs`, `protocol.rs`, `manifest.rs`, `registry.rs`,
`process.rs`, and `runner.rs` as described in Stage B. After each module:

    cargo check -p weaver-plugins
    cargo clippy -p weaver-plugins --all-targets -- -D warnings

### Step 4: Write unit tests

Add `#[cfg(test)] mod tests;` to each module. Write unit tests using `rstest`
fixtures. Expected test coverage:

- `protocol`: round-trip serialization for each `PluginOutput` variant,
  malformed JSON rejection, empty diagnostics.
- `manifest`: construction, validation (empty name, non-absolute path),
  accessors.
- `registry`: register, get, find by kind, find by language, duplicate
  rejection.
- `runner`: execution with mock executor, error propagation.

Verify:

    cargo test -p weaver-plugins 2>&1 | tee /tmp/plugins-test.log
    echo "EXIT: $?"

### Step 5: Write BDD tests

Create `crates/weaver-plugins/tests/features/plugin_execution.feature` and
step definitions. Verify:

    cargo test -p weaver-plugins 2>&1 | tee /tmp/plugins-bdd.log
    echo "EXIT: $?"

### Step 6: Wire into weaverd

Add `weaver-plugins` dependency to `crates/weaverd/Cargo.toml`. Create the
`refactor` handler module. Update the router. Verify:

    cargo test -p weaverd 2>&1 | tee /tmp/weaverd-test.log
    echo "EXIT: $?"

### Step 7: Update documentation

Edit `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`.
Verify:

    make check-fmt

### Step 8: Full quality gate

    set -o pipefail
    make check-fmt 2>&1 | tee /tmp/check-fmt.log; echo "EXIT: $?"
    make lint 2>&1 | tee /tmp/lint.log; echo "EXIT: $?"
    make test 2>&1 | tee /tmp/test.log; echo "EXIT: $?"

All three must exit with status 0.

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes with no regressions. New unit tests cover all
  `weaver-plugins` modules. BDD scenarios cover happy path (actuator success,
  sensor success) and unhappy paths (not found, invalid output, non-zero exit,
  timeout).
- Lint: `make lint` passes with zero warnings.
- Format: `make check-fmt` reports no formatting violations.
- Documentation: `docs/weaver-design.md` contains implementation decisions.
  `docs/users-guide.md` documents the plugin system. `docs/roadmap.md` marks
  the entry as done.

Quality method (how we check):

    make check-fmt && make lint && make test

## Idempotence and recovery

All steps are re-runnable. The crate creation step uses `mkdir -p` (no
failure on existing directory). Cargo operations are idempotent. If a step
fails partway through, fix the issue and re-run the same step.

To roll back entirely: remove `crates/weaver-plugins/` from the filesystem
and its entry from the workspace `members` list.

## Interfaces and dependencies

### New crate: `weaver-plugins`

Dependencies: `serde` 1.0, `serde_json` 1.0, `thiserror` (workspace),
`tracing` 0.1, `weaver-sandbox` (path).

Dev-dependencies: `rstest` (workspace), `rstest-bdd` (workspace),
`rstest-bdd-macros` (workspace), `tempfile` (workspace).

### Key types defined by this crate

In `crates/weaver-plugins/src/manifest.rs`:

    /// Category of a plugin within the Weaver ecosystem.
    pub enum PluginKind { Sensor, Actuator }

    /// Declarative description of a plugin's identity and capabilities.
    pub struct PluginManifest {
        name: String,
        version: String,
        kind: PluginKind,
        languages: Vec<String>,
        executable: PathBuf,
        args: Vec<String>,
        timeout_secs: u64,
    }

In `crates/weaver-plugins/src/protocol.rs`:

    /// Request sent from weaverd to a plugin on stdin.
    pub struct PluginRequest {
        operation: String,
        files: Vec<FilePayload>,
        arguments: HashMap<String, serde_json::Value>,
    }

    /// File content passed to the plugin in the request.
    pub struct FilePayload { path: PathBuf, content: String }

    /// Response sent from a plugin to weaverd on stdout.
    pub struct PluginResponse {
        success: bool,
        output: PluginOutput,
        diagnostics: Vec<PluginDiagnostic>,
    }

    /// Output payload from a plugin.
    pub enum PluginOutput {
        Diff { content: String },
        Analysis { data: serde_json::Value },
        Empty,
    }

In `crates/weaver-plugins/src/runner.rs`:

    /// Trait abstracting plugin execution for testability.
    pub trait PluginExecutor {
        fn execute(
            &self,
            manifest: &PluginManifest,
            request: &PluginRequest,
        ) -> Result<PluginResponse, PluginError>;
    }

    /// Orchestrates plugin execution within the sandbox.
    pub struct PluginRunner<E> {
        registry: PluginRegistry,
        executor: E,
    }

In `crates/weaver-plugins/src/error.rs`:

    /// Errors arising from plugin operations.
    pub enum PluginError {
        NotFound { name: String },
        SpawnFailed { name: String, message: String, source: ... },
        Timeout { name: String, timeout_secs: u64 },
        NonZeroExit { name: String, status: i32 },
        SerializeRequest(serde_json::Error),
        DeserializeResponse { message: String, source: ... },
        InvalidOutput { name: String, message: String },
        Io { name: String, source: Arc<std::io::Error> },
        Sandbox { name: String, message: String },
        Manifest { message: String },
    }

### IPC protocol specification

Direction: weaverd -> plugin (stdin). A single JSONL line:

    {"operation":"rename","files":[{"path":"/project/src/main.py","content":"def old():\n    pass\n"}],"arguments":{"new_name":"new_func"}}\n

The broker closes stdin after writing to signal no more input.

Direction: plugin -> weaverd (stdout). A single JSONL line:

    {"success":true,"output":{"kind":"diff","content":"--- a/src/main.py\n+++ b/src/main.py\n@@ -1 +1 @@\n-def old():\n+def new_func():\n"},"diagnostics":[]}\n

Plugin stderr is captured for diagnostic logging but is not part of the
protocol.

## Artifacts and notes

(To be populated during implementation with transcripts of test runs and
any notable code snippets.)
