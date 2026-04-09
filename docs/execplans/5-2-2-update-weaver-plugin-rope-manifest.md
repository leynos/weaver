# Update weaver-plugin-rope manifest and runtime handshake for rename-symbol

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

After this change, the `weaver-plugin-rope` actuator plugin declares
`rename-symbol` in its manifest capabilities and serves rename requests through
the capability contract interface defined in roadmap item 5.2.1. The daemon
handler constructs contract-conforming requests, and the plugin returns
structured failure diagnostics with stable reason codes. Legacy provider
routing through the old `"rename"` operation name is retired for Python rename
flows.

Observable behaviour after this change:

- Running `make check-fmt && make lint && make test` passes with no
  regressions.
- The rope plugin manifest includes `CapabilityId::RenameSymbol` in its
  capabilities. The registry method
  `find_for_language_and_capability("python", CapabilityId::RenameSymbol)`
  returns the rope manifest.
- The rope plugin accepts `"rename-symbol"` operation requests with `uri`,
  `position`, and `new_name` arguments conforming to the `RenameSymbolContract`
  schema.
- The rope plugin rejects the old `"rename"` operation name with
  `ReasonCode::OperationNotSupported`.
- Failure diagnostics include stable `ReasonCode` values (e.g.,
  `IncompletePayload` for missing arguments, `SymbolNotFound` for adapter
  failures).
- The `weaverd` handler maps the user-facing `--refactoring rename` to the
  contract operation `"rename-symbol"` and translates `offset` to `position`
  internally, preserving command-line interface (CLI) backward compatibility.
- Behaviour-driven development (BDD) scenarios cover happy path,
  missing arguments, unsupported operation, adapter failure, unchanged output,
  and reason code verification.

## Constraints

1. **No async runtime.** The entire project uses synchronous blocking I/O.
2. **Edition 2024, Rust 1.85+.** The workspace uses `edition = "2024"`.
3. **Strict Clippy.** Over 60 denied lint categories including `unwrap_used`,
   `expect_used`, `indexing_slicing`, `string_slice`, `missing_docs`,
   `cognitive_complexity`, and `allow_attributes`. Both `weaver-plugin-rope`
   and `weaverd` opt into workspace lints. All code must pass
   `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
4. **400-line file limit.** No single source file may exceed 400 lines.
   `crates/weaver-plugin-rope/src/lib.rs` starts at 384 lines (16 lines of
   headroom). `crates/weaverd/src/dispatch/act/refactor/mod.rs` starts at 375
   lines (25 lines of headroom).
5. **Error handling.** Library crates use `thiserror`-derived error enums.
6. **Documentation.** Every module begins with `//!` doc comments. All public
   items have `///` rustdoc comments.
7. **en-GB-oxendict spelling.** Comments and documentation use British English
   with Oxford "-ize" / "-yse" / "-our" spelling.
8. **rstest-bdd v0.5.0.** BDD tests use v0.5.0 with mutable world fixtures
   (`&mut`). The fixture parameter must be named exactly `world` (not
   `_world`). Use `let _ = world;` to suppress unused warnings.
9. **Lint suppressions must use `#[expect]` with reason**, not `#[allow]`.
10. **Do not modify `crates/weaver-plugins/`.** The capability contract types
   from roadmap item 5.2.1 are complete and must not be changed.
11. **CLI backward compatibility.** Users still pass `--refactoring rename`,
   `offset=N`, and `new_name=X`. The handler does the translation internally.
12. **`str_to_string` denied.** Use `String::from()` or `.into()`, not
   `.to_string()` on `&str`.

## Tolerances (exception triggers)

- **Scope:** If implementation requires changes to more than 12 files or more
  than 400 net lines of code, stop and escalate.
- **Interface:** If the `RopeAdapter` trait's public `rename()` method
  signature must change in a way that breaks the `PythonRopeAdapter`, stop and
  escalate.
- **Dependencies:** If a new external crate dependency is required, stop and
  escalate.
- **Iterations:** If tests still fail after 5 attempts at fixing, stop and
  escalate.
- **Line budget:** If any file cannot fit within 400 lines without extracting a
  new submodule, proceed with the extraction, but if it requires more than 2
  new files, stop and escalate.
- **Ambiguity:** The acceptance criteria states "Legacy provider routing is not
  required for Python rename flows." If this interpretation (retire `"rename"`
  operation name in the plugin, update the `weaverd` handler to send
  `"rename-symbol"` with contract arguments) is wrong, stop and escalate.

## Risks

- Risk: `lib.rs` at 384 lines has only 16 lines of headroom; adding imports
  and modifying the dispatch logic could push it over 400. Severity: medium
  Likelihood: high Mitigation: Extract `parse_rename_arguments()` and
  `json_value_to_string()` into a new
  `crates/weaver-plugin-rope/src/arguments.rs` submodule. This frees
  approximately 30 lines of budget.

- Risk: The `weaverd` refactor handler at 375 lines needs argument
  translation logic that could push it over 400. Severity: low Likelihood:
  medium Mitigation: The change replaces existing argument-forwarding code with
  slightly longer translation code. Net delta is approximately +12 lines,
  arriving at ~387 lines, within budget.

- Risk: BDD step definitions reference the old operation name `"rename"` and
  argument keys (`offset`, `new_name`). Updating them could introduce
  regressions. Severity: medium Likelihood: low Mitigation: Update all step
  definitions and feature files atomically. Run `make test` after every change.

- Risk: Changing `execute_request` return semantics from `Err(String)` to
  `Err(PluginFailure)` requires updating all test assertions. Severity: medium
  Likelihood: certain Mitigation: The `PluginFailure` type provides `Display`
  and `message()` accessor, so test assertions need minimal rewording. Update
  tests in the same stage as the type change.

## Progress

- [x] (2026-03-05) Write execution plan.
- [x] (2026-03-05) Create `crates/weaver-plugin-rope/src/arguments.rs` with
  `parse_rename_symbol_arguments()`.
- [x] (2026-03-05) Refactor `crates/weaver-plugin-rope/src/lib.rs`: add
  `PluginFailure` type, update dispatch to `"rename-symbol"`, integrate reason
  codes, remove extracted functions.
- [x] (2026-03-05) Update `crates/weaverd/src/dispatch/act/refactor/mod.rs`:
  add `.with_capabilities()` to rope manifest, translate arguments in handler.
- [x] (2026-03-05) Update rope plugin unit tests in `src/tests/mod.rs`.
- [x] (2026-03-05) Update rope plugin BDD step definitions in
  `src/tests/behaviour.rs`.
- [x] (2026-03-05) Update rope plugin feature file
  `tests/features/rope_plugin.feature`.
- [x] (2026-03-05) Add weaverd contract conformance test in
  `src/dispatch/act/refactor/tests.rs`.
- [x] (2026-03-05) Update `docs/users-guide.md`.
- [x] (2026-03-05) Mark `docs/roadmap.md` entry 5.2.2 as done.
- [x] (2026-03-05) Run `make check-fmt`, `make lint`, `make test` — all pass
  (152 tests, 0 failures).

## Surprises & discoveries

- Observation: `PluginFailure` needed `#[derive(Debug)]` because tests use
  `.expect()` on `Result<_, PluginFailure>`, which requires `Debug`. Evidence:
  rustc error `E0277: PluginFailure doesn't implement Debug`. Impact: Added one
  derive line; file stayed within 400-line budget.

## Decision log

- Decision: Retire the `"rename"` operation name in the plugin entirely, not
  keep dual support. Rationale: The acceptance criteria say "Legacy provider
  routing is not required for Python rename flows." Adding dual operation
  support increases complexity and line count, and the `"rename"` name was
  never part of a stable public contract. The weaverd handler maps the
  user-facing `--refactoring rename` to `"rename-symbol"` before sending to the
  plugin. Date: 2026-03-05.

- Decision: Extract an `arguments.rs` submodule from `lib.rs` to create line
  budget. Rationale: `lib.rs` is at 384/400 lines. Moving argument parsing into
  a dedicated module frees ~30 lines of budget for the new imports and
  `PluginFailure` type. This follows the project pattern of small, focused
  modules. Date: 2026-03-05.

- Decision: Map `position` as a byte offset string.
  Rationale: The rope adapter needs a `usize` byte offset. The contract's
  `position` field is a string. The handler passes the user's `offset=N` value
  as the `position` string. The plugin parses it back to `usize`. This is the
  simplest conforming implementation. Date: 2026-03-05.

- Decision: Introduce `PluginFailure` struct to carry
  `(message, Option<ReasonCode>)`. Rationale: `execute_request()` currently
  returns `Result<PluginResponse, String>`. To attach reason codes to failures,
  the error type must carry an optional `ReasonCode`. A small struct is cleaner
  than a tuple and provides `Display` for backward-compatible test assertions.
  The `Err` path is for business-logic failures; infrastructure failures
  (stdin/JSON parse errors) also use `PluginFailure` but without a reason code.
  Date: 2026-03-05.

- Decision: Map reason codes as follows: unsupported operation →
  `OperationNotSupported`; missing/invalid arguments → `IncompletePayload`;
  adapter engine failure → `SymbolNotFound`; no content changes →
  `SymbolNotFound`; invalid file path → `IncompletePayload`. Rationale: These
  mappings align with the `ReasonCode` semantics defined in
  `crates/weaver-plugins/src/capability/reason_code.rs`. Engine failures and
  unchanged output both indicate the symbol could not be effectively renamed.
  Date: 2026-03-05.

## Outcomes & retrospective

All deliverables are complete. The rope plugin now declares `rename-symbol` in
its manifest, accepts contract-conforming requests with `uri`/`position`/
`new_name` arguments, and returns structured failure diagnostics with stable
`ReasonCode` values. The weaverd handler maps the user-facing
`--refactoring rename` to the internal `rename-symbol` operation and translates
`offset` to `position`, maintaining CLI backward compatibility.

Files created: `crates/weaver-plugin-rope/src/arguments.rs` (98 lines).
Implementation artefacts: `crates/weaver-plugin-rope/src/lib.rs` (400 lines,
exactly at budget), `crates/weaverd/src/dispatch/act/refactor/mod.rs` (396
lines), `crates/weaver-plugin-rope/src/tests/mod.rs`,
`crates/weaver-plugin-rope/src/tests/behaviour.rs`,
`crates/weaver-plugin-rope/tests/features/rope_plugin.feature`,
`crates/weaverd/src/dispatch/act/refactor/tests.rs`, `docs/users-guide.md`,
`docs/roadmap.md`, and this ExecPlan
(`docs/execplans/5-2-2-update-weaver-plugin-rope-manifest.md`).

Lessons: Always derive `Debug` on error types that might be used with
`.expect()` in tests. The 400-line budget required careful extraction of the
arguments module to make room for the `PluginFailure` struct.

## Context and orientation

### Repository structure

The Weaver project is a Rust workspace implementing a client-daemon tool for
code analysis and modification. The key crates for this task are:

- `crates/weaver-plugins/` — Plugin framework. Defines `PluginRequest`,
  `PluginResponse`, `PluginManifest`, `PluginRegistry`, `CapabilityId`,
  `ReasonCode`, `RenameSymbolRequest`, `RenameSymbolContract`. This crate was
  updated in roadmap item 5.2.1 and must NOT be modified in 5.2.2.

- `crates/weaver-plugin-rope/` — The Python rope-backed actuator plugin. A
  standalone binary crate that reads one JSON Lines (JSONL) request from stdin
  and writes one JSONL response to stdout. Baseline-only line counts before
  implementation were: `src/lib.rs` (384 lines), `src/tests/mod.rs` (224
  lines), `src/tests/behaviour.rs` (176 lines), and
  `tests/features/rope_plugin.feature` (33 lines).

- `crates/weaverd/` — The Weaver daemon. The refactor handler at
  `src/dispatch/act/refactor/mod.rs` (375 lines) registers the rope plugin
  manifest and constructs `PluginRequest` payloads for plugin execution.

### Current rope plugin protocol

The rope plugin currently:

1. Accepts operation `"rename"` (not `"rename-symbol"`).
2. Expects arguments: `offset` (string or number, parsed to `usize`) and
   `new_name` (string).
3. Returns `PluginOutput::Diff` on success.
4. Returns failure via `PluginResponse::failure` with a single
   `PluginDiagnostic` that has no `reason_code`.
5. The function `failure_response(message: String)` constructs failures.

### Current manifest registration

In `crates/weaverd/src/dispatch/act/refactor/mod.rs` lines 67–68, the rope
manifest is constructed without capability declarations:

```rust
let rope_manifest =
    PluginManifest::new(rope_metadata, vec![String::from("python")], rope_executable);
```

### Rename-symbol contract (from 5.2.1)

The contract (defined in
`crates/weaver-plugins/src/capability/rename_symbol.rs`) requires:

- Operation: `"rename-symbol"`
- Arguments: `uri` (non-empty string), `position` (non-empty string),
  `new_name` (non-empty string)
- Success response: `PluginOutput::Diff`
- Failure response: diagnostics with optional `ReasonCode`

### Reason codes

Defined in `crates/weaver-plugins/src/capability/reason_code.rs` as a 7-
variant enum: `SymbolNotFound`, `MacroGenerated`, `AmbiguousReferences`,
`UnsupportedLanguage`, `IncompletePayload`, `NameConflict`,
`OperationNotSupported`. Serialized as `snake_case` strings.

## Plan of work

### Stage A: Extract arguments module (create line budget)

Create `crates/weaver-plugin-rope/src/arguments.rs` containing:

- `RenameSymbolArgs` struct with `offset: usize` and `new_name: String`, plus
  accessor methods.
- `parse_rename_symbol_arguments()` function that validates `uri` (present,
  non-empty string), `position` (parseable as `usize`), and `new_name`
  (non-empty string) from the arguments `HashMap`. Returns
  `Result<RenameSymbolArgs, String>`.
- `json_value_to_string()` helper (moved from `lib.rs`).

In `lib.rs`, add `mod arguments;`, replace the inline
`parse_rename_arguments()` and `json_value_to_string()` with the new module's
function, and remove the old implementations.

Validation: `cargo check -p weaver-plugin-rope` compiles.

### Stage B: Update rope plugin dispatch and failure responses

In `crates/weaver-plugin-rope/src/lib.rs`:

1. Add import for `ReasonCode` from `weaver_plugins`.
2. Define `PluginFailure` struct carrying `message: String` and
   `reason_code: Option<ReasonCode>`, with `Display` impl and constructors.
3. Change `execute_request()` match arm from `"rename"` to
   `"rename-symbol"`. The `Err` case for unsupported operations uses
   `PluginFailure::with_reason(message, ReasonCode::OperationNotSupported)`.
4. Update `execute_rename()` to use `parse_rename_symbol_arguments()` and
   return `PluginFailure` errors with appropriate reason codes.
5. Update `failure_response()` to accept `PluginFailure` and attach the
   reason code to the diagnostic when present.
6. Update `run_with_adapter()` and `read_request()` to use `PluginFailure`
   instead of `String`.

Validation: `cargo check -p weaver-plugin-rope` compiles.

### Stage C: Update weaverd manifest and handler

In `crates/weaverd/src/dispatch/act/refactor/mod.rs`:

1. Add `use weaver_plugins::CapabilityId;` import.
2. Add `.with_capabilities(vec![CapabilityId::RenameSymbol])` to the rope
   manifest construction.
3. In `handle()`, after building `plugin_args` from the extras loop:
   - If the refactoring is `"rename"`, set the effective operation to
     `"rename-symbol"`.
   - Inject `uri` from `args.file` if not already present.
   - Map `offset` to `position` if `offset` is present and `position` is not.
4. Use the effective operation name when constructing the `PluginRequest`.

Validation: `cargo check -p weaverd` compiles.

### Stage D: Update rope plugin tests

Update `crates/weaver-plugin-rope/src/tests/mod.rs`:

1. Update `rename_arguments()` fixture: add `"uri"` key, rename `"offset"` to
   `"position"`.
2. Update `request_with_args()`: operation `"rename"` → `"rename-symbol"`.
3. Update parameterized `rename_argument_validation` test cases for new
   argument names (`position`, `uri`) and `PluginFailure` error type.
4. Update `unsupported_operation_returns_error` for new return type.
5. Update `rename_non_mutating_or_error_returns_failure` for new return type.
6. Update `run_with_adapter_dispatch_layer` for new request JSON.
7. Add test verifying `ReasonCode` on failure diagnostics.

Update `crates/weaver-plugin-rope/src/tests/behaviour.rs`:

1. Update `build_request()`: operation `"rename-symbol"`, add `uri` argument,
   rename `offset` to `position`.
2. Update `should_invoke_rename()`: check `"rename-symbol"`, `"position"`,
   `"uri"`.
3. Update `#[given]` step annotations to match new feature file text.
4. Add `#[then("the failure has reason code {code}")]` step for reason code
   assertion.

Update `crates/weaver-plugin-rope/tests/features/rope_plugin.feature`:

1. Rename scenarios to reference "Rename-symbol".
2. Update step text: "offset" → "position".
3. Add reason code assertion step to adapter failure scenario.

Validation: `cargo test -p weaver-plugin-rope` passes.

### Stage E: Update weaverd tests

In `crates/weaverd/src/dispatch/act/refactor/tests.rs`, add a test that
verifies the `PluginRequest` sent to the plugin uses `"rename-symbol"` as the
operation and contains `uri`, `position`, and `new_name` in the arguments map.

Validation: `cargo test -p weaverd` passes.

### Stage F: Documentation and cleanup

1. Update `docs/users-guide.md` in the "act refactor" section to note that
   the handler maps `--refactoring rename` to the capability contract operation
   `"rename-symbol"` internally. Update the parameter table to note that
   `offset` is mapped to `position` in the plugin protocol. Add a note that the
   rope plugin now declares the `rename-symbol` capability.
2. Mark `docs/roadmap.md` entry 5.2.2 as done (`[x]`).
3. Run `make fmt` to format all changed files.
4. Run `make markdownlint` to validate Markdown.

Validation: `make check-fmt && make lint && make test` all pass.

## Concrete steps

All commands are run from the workspace root `/home/user/project`.

### Step 1: Verify baseline

```sh
set -o pipefail && make test 2>&1 | tee /tmp/test-baseline.log
```

Expected: all tests pass (29 test suites, 0 failures).

### Step 2: Create arguments module

Create `crates/weaver-plugin-rope/src/arguments.rs` and update `lib.rs`.

```sh
cargo check -p weaver-plugin-rope
```

Expected: compiles with no errors.

### Step 3: Update dispatch and failure responses

Edit `lib.rs` as described in Stage B.

```sh
cargo check -p weaver-plugin-rope
```

Expected: compiles (tests may fail until Stage D).

### Step 4: Update weaverd manifest and handler

Edit `crates/weaverd/src/dispatch/act/refactor/mod.rs` as described in Stage C.

```sh
cargo check -p weaverd
```

Expected: compiles.

### Step 5: Update all tests

Apply changes from Stages D and E.

```sh
set -o pipefail && make test 2>&1 | tee /tmp/test-after.log
```

Expected: all tests pass.

### Step 6: Update documentation

Apply changes from Stage F.

```sh
make fmt && make markdownlint
```

Expected: no errors.

### Step 7: Final validation

```sh
set -o pipefail && make check-fmt 2>&1 | tee /tmp/fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/lint.log
set -o pipefail && make test 2>&1 | tee /tmp/test-final.log
```

Expected: all three pass with zero warnings and zero failures.

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes. All 29 pre-existing test suites pass. New unit
  tests verify reason codes on failure diagnostics. New BDD scenarios cover
  rename-symbol happy path, missing arguments, adapter failure with reason
  code, and unsupported operation.
- Lint: `make lint` passes with zero warnings.
- Format: `make check-fmt` passes.
- Markdown: `make markdownlint` passes.

Acceptance criteria from the roadmap:

1. **Plugin advertises `rename-symbol` in capability probes.** The rope
   manifest includes `CapabilityId::RenameSymbol`. Verified by a weaverd unit
   test that inspects the `PluginRequest` operation name.
2. **Request payloads conform to schema.** The rope plugin accepts
   `"rename-symbol"` with `uri`, `position`, `new_name`. Unit tests send
   conforming requests and receive `PluginOutput::Diff`.
3. **Response payloads conform to schema.** Failure responses include
   `ReasonCode` values. BDD scenario checks `reason_code` field.
4. **Legacy provider routing is not required.** The old `"rename"` operation
   is rejected with `ReasonCode::OperationNotSupported`. A unit test verifies
   this.

## Idempotence and recovery

All stages are re-runnable. If a stage fails partway, fix the issue and re-run
from the beginning of that stage. No destructive operations are involved. The
new `arguments.rs` file is additive; existing files are edited in place.

## Artefacts and notes

### Line budget analysis

Table: Line-budget projection for files modified by this plan.

| File                                                    | Current | Delta                               | Projected |
| ------------------------------------------------------- | ------- | ----------------------------------- | --------- |
| `weaver-plugin-rope/src/lib.rs`                         | 384     | -30 (extract) +15 (new logic) = -15 | 369       |
| `weaver-plugin-rope/src/arguments.rs`                   | 0 (new) | +70                                 | 70        |
| `weaverd/dispatch/act/refactor/mod.rs`                  | 375     | +12                                 | 387       |
| `weaver-plugin-rope/src/tests/mod.rs`                   | 224     | +30                                 | 254       |
| `weaver-plugin-rope/src/tests/behaviour.rs`             | 176     | +20                                 | 196       |
| `weaver-plugin-rope/tests/features/rope_plugin.feature` | 33      | +5                                  | 38        |
| `weaverd/dispatch/act/refactor/tests.rs`                | 249     | +30                                 | 279       |

All files remain within the 400-line limit.

### Dependency analysis

No new external crate dependencies are required. All types come from
`weaver-plugins` which is already a dependency of both `weaver-plugin-rope` and
`weaverd`.

## Interfaces and dependencies

### New internal types

In `crates/weaver-plugin-rope/src/arguments.rs`:

```rust
/// Validated rename-symbol arguments extracted from a plugin request.
pub(crate) struct RenameSymbolArgs {
    offset: usize,
    new_name: String,
}

/// Parses and validates rename-symbol arguments from the request map.
pub(crate) fn parse_rename_symbol_arguments(
    arguments: &HashMap<String, serde_json::Value>,
) -> Result<RenameSymbolArgs, String>
```

In `crates/weaver-plugin-rope/src/lib.rs`:

```rust
/// Structured failure carrying an optional reason code for diagnostics.
pub(crate) struct PluginFailure {
    message: String,
    reason_code: Option<ReasonCode>,
}
```

### Modified registrations

In `crates/weaverd/src/dispatch/act/refactor/mod.rs`:

```rust
let rope_manifest =
    PluginManifest::new(rope_metadata, vec![String::from("python")], rope_executable)
        .with_capabilities(vec![CapabilityId::RenameSymbol]);
```

### Imported types (from weaver-plugins, read-only)

- `CapabilityId::RenameSymbol` — capability identifier
- `ReasonCode` — 7-variant enum for failure diagnostics
- `PluginDiagnostic::with_reason_code()` — builder method
