# Define the `rename-symbol` capability contract for actuator plugins

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

After this change, the `weaver-plugins` crate defines a strongly-typed,
versioned capability contract for the `rename-symbol` actuator operation.
Plugins that implement `rename-symbol` declare the capability in their
manifest, and the broker validates request and response payloads against the
contract schema before and after plugin execution. When a rename cannot
proceed, the plugin returns a structured refusal diagnostic with a stable
reason code that callers can match programmatically without parsing free-text
messages.

Observable behaviour after this change:

- Running `make check-fmt && make lint && make test` passes with no
  regressions.
- New unit tests in `crates/weaver-plugins/src/capability/` exercise schema
  validation for happy and unhappy paths.
- New BDD scenarios in
  `crates/weaver-plugins/tests/features/capability_contract.feature` exercise
  contract validation, refusal diagnostics, and manifest capability
  declarations.
- Downstream roadmap items 5.2.2 and 5.2.3 (updating rope and rust-analyzer
  plugins) can import the new types and implement the contract without further
  schema work in this crate.

## Constraints

1. **No async runtime.** The entire project uses synchronous blocking I/O.
   All new code must remain synchronous.
2. **Edition 2024, Rust 1.85+.** The workspace uses `edition = "2024"` and
   `rust-version = "1.85"`.
3. **Strict Clippy.** Over 60 denied lint categories including `unwrap_used`,
   `expect_used`, `indexing_slicing`, `string_slice`, `missing_docs`,
   `cognitive_complexity`, `self_named_module_files`, and `allow_attributes`.
   The `weaver-plugins` crate opts into workspace lints via
   `[lints] workspace = true`. All code must pass
   `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
4. **400-line file limit.** No single source file may exceed 400 lines.
5. **Error handling.** Library crates use `thiserror`-derived error enums.
   No `eyre` or `anyhow` in library code. Large errors use `Arc` to satisfy
   `result_large_err`.
6. **Documentation.** Every module begins with `//!` doc comments. All
   public items have `///` rustdoc comments with examples where non-trivial.
7. **en-GB-oxendict spelling.** Comments and documentation use British English
   with Oxford "-ize" / "-yse" / "-our" spelling.
8. **rstest-bdd v0.5.0.** BDD tests use v0.5.0 with mutable world fixtures
   (`&mut`) instead of `RefCell`.
9. **Caret version requirements.** New dependencies must use caret
   requirements and be declared in `[workspace.dependencies]` when shared.
10. **Lint suppressions must use `#[expect]` with reason**, not `#[allow]`.
11. **Existing public API stability.** All existing public types in
    `weaver-plugins` (`PluginRequest`, `PluginResponse`, `PluginOutput`,
    `PluginManifest`, `PluginKind`, `PluginError`, etc.) must retain their
    current public API signatures. New fields on serializable structs must
    use `#[serde(default)]` for backwards compatibility.
12. **Scope boundary.** This plan defines the contract types, validation
    functions, and manifest extension only. It does not wire capability
    routing into `weaverd` dispatch (that is roadmap item 5.2.4), nor does
    it modify the rope or rust-analyzer plugin crates (those are 5.2.2 and
    5.2.3).

## Tolerances (exception triggers)

- **Scope:** If implementation requires changes to more than 12 source files
  (excluding test files), stop and escalate.
- **Interface:** If any existing public API signature in `weaver-plugins` must
  change in a backwards-incompatible way, stop and escalate.
- **Dependencies:** If a new external crate dependency is required beyond
  what is already in the workspace, stop and escalate.
- **Iterations:** If tests still fail after 3 attempts at a fix, stop and
  escalate.
- **Line budget:** If any single file approaches 380 lines, split before
  continuing.
- **Ambiguity:** If the contract shape for rename-symbol requires information
  not available in the current codebase or ADR, stop and present options.

## Risks

- Risk: Backwards-incompatible serde changes to `PluginManifest` when adding
  the `capabilities` field. Severity: high Likelihood: low Mitigation: Use
  `#[serde(default)]` on the new field so existing manifest JSON without the
  field deserializes cleanly to an empty capabilities list.

- Risk: The 400-line limit forces premature splitting of the capability module.
  Severity: low Likelihood: medium Mitigation: Design the module as a directory
  (`capability/mod.rs`, `capability/rename_symbol.rs`,
  `capability/reason_code.rs`) from the start.

- Risk: The `PluginDiagnostic` struct may need a new field for reason codes,
  which could be a breaking serde change for existing plugin responses.
  Severity: medium Likelihood: medium Mitigation: Add the reason code as an
  `Option` with `#[serde(default)]` and
  `skip_serializing_if = "Option::is_none"` so existing diagnostics without
  reason codes continue to deserialize.

- Risk: Strict Clippy lints (`unwrap_used`, `expect_used`, `indexing_slicing`)
  make test code verbose; BDD step functions need careful error handling.
  Severity: low Likelihood: high Mitigation: Follow the existing pattern in
  `crates/weaver-plugins/src/tests/behaviour.rs` which uses `expect()` in
  non-Result test functions (the `expect_used` lint fires in library code but
  test modules suppress it via `#[cfg(test)]` scope). Use `assert!` and `match`
  for error discrimination.

## Progress

- [x] (2026-02-28) Write execution plan.
- [x] (2026-02-28) Create `capability/mod.rs` with `CapabilityId`,
  `ContractVersion`, `CapabilityContract` trait.
- [x] (2026-02-28) Create `capability/rename_symbol.rs` with
  `RenameSymbolContract` and `RenameSymbolRequest`.
- [x] (2026-02-28) Create `capability/reason_code.rs` with `ReasonCode` enum.
- [x] (2026-02-28) Create `capability/tests.rs` with unit tests.
- [x] (2026-02-28) Extend `PluginManifest` with `capabilities` field.
- [x] (2026-02-28) Extend `PluginDiagnostic` with optional `reason_code`
  field.
- [x] (2026-02-28) Add `find_for_capability()` and
  `find_for_language_and_capability()` to `PluginRegistry`.
- [x] (2026-02-28) Update `lib.rs` with re-exports.
- [x] (2026-02-28) Create BDD feature file and step definitions.
- [x] (2026-02-28) Update `docs/users-guide.md` with capability contract
  documentation.
- [x] (2026-02-28) Run `make check-fmt`, `make lint`, `make test` — all pass.
- [x] (2026-02-28) Mark roadmap entry 5.2.1 as done.

## Surprises & discoveries

- Observation: The workspace Clippy configuration denies `string_slice`, which
  prevents string indexing even in test code. The initial BDD key-value parser
  used byte-offset indexing into strings.
  Evidence: `make lint` failed with 6 `string_slice` errors.
  Impact: Rewrote the parser to use `split_whitespace` + `split_once` instead
  of positional indexing. No functional change, but a reminder that the strict
  lints apply uniformly to test code.

- Observation: The `doc_markdown` Clippy lint requires backticks around
  identifiers like `snake_case` in doc comments.
  Evidence: `make lint` flagged two lines in `reason_code.rs`.
  Impact: Trivial fix; wrapped `snake_case` in backticks.

## Decision log

- Decision: Use an enum for `CapabilityId` rather than a string-backed
  newtype. Rationale: ADR 001 defines exactly five first-party capability IDs
  (`rename-symbol`, `extricate-symbol`, `extract-method`, `replace-body`,
  `extract-predicate`). An enum provides compile-time exhaustiveness checking,
  reliable `match` coverage, and prevents typos. Third-party extensibility is a
  non-goal at this stage. The enum follows the precedent set by
  `CapabilityKind` in `crates/weaver-lsp-host/src/capability.rs`. Date:
  2026-02-28.

- Decision: Create a new `capability/` module directory rather than adding
  to existing protocol or manifest modules. Rationale: The capability contract
  is a cross-cutting concern that touches request validation, response
  validation, manifest metadata, and diagnostic extensions. Placing it in its
  own module keeps existing modules stable and under the 400-line limit. It
  also provides a natural home for future capability contracts
  (`extricate-symbol`, etc.). Date: 2026-02-28.

- Decision: Use major.minor integer versioning for the contract rather than
  SemVer strings. Rationale: Contract versions are internal to the
  broker-plugin protocol, not published crate versions. A simple
  `ContractVersion { major: u16, minor: u16 }` is easier to compare and
  negotiate than parsing SemVer strings. Major bumps indicate breaking changes;
  minor bumps indicate additive changes. This is sufficient for the acceptance
  criterion "capability contract is versioned". Date: 2026-02-28.

- Decision: Add an optional `reason_code` field to `PluginDiagnostic`
  rather than creating a separate `RefusalDiagnostic` type. Rationale: The
  existing `PluginResponse::failure(diagnostics)` pattern is already used by
  both plugin crates. Adding a reason code field to the existing diagnostic
  type preserves this pattern, avoids a parallel type hierarchy, and keeps the
  response schema simple. The field is `Option` for backwards compatibility.
  Date: 2026-02-28.

- Decision: Implement validation as a `CapabilityContract` trait with a
  `RenameSymbolContract` implementation, rather than free functions. Rationale:
  Each future capability (`extricate-symbol`, `extract-method`, etc.) will need
  its own validation logic. A trait provides a uniform interface
  (`validate_request`, `validate_response`) and enables the broker to dispatch
  validation by capability ID. This follows the polymorphic pattern used
  elsewhere in the codebase (e.g., `PluginExecutor` trait in `runner/mod.rs`).
  Date: 2026-02-28.

## Outcomes & retrospective

All acceptance criteria are met:

1. **Capability contract is versioned.** `ContractVersion::new(1, 0)` is the
   current `rename-symbol` contract version. `is_compatible_with()` enforces
   same-major-version compatibility.

2. **Broker validation enforces schema shape.** `RenameSymbolContract`
   implements `CapabilityContract` with `validate_request()` (checks `uri`,
   `position`, `new_name` fields) and `validate_response()` (checks successful
   responses contain `PluginOutput::Diff`). Unit and BDD tests cover happy and
   unhappy paths.

3. **Refusal diagnostics use stable reason codes.** `ReasonCode` enum with 7
   variants is serialized as `snake_case` strings. The optional `reason_code`
   field on `PluginDiagnostic` is backwards-compatible via `serde(default)`.

Deliverables: 4 new files in `capability/` module, 3 extended existing types,
1 BDD feature file with 10 scenarios, 1 BDD step definitions file, user guide
documentation, and roadmap update. All 124 `weaver-plugins` tests pass. Full
workspace `check-fmt`, `lint`, and `test` gates pass.

Lessons: Strict workspace Clippy lints (`string_slice`, `doc_markdown`) catch
issues that would be acceptable in many projects. Test code is held to the same
standard as production code, which requires more careful string handling even
in test fixtures.

## Context and orientation

### Repository structure

The Weaver project is a client-daemon tool for code analysis and modification.
The relevant crate for this work is `crates/weaver-plugins/`, which implements
the plugin orchestration layer. The crate currently has these source modules:

```plaintext
crates/weaver-plugins/src/
  lib.rs              (60 lines)   - public facade with re-exports
  error/mod.rs        (112 lines)  - PluginError enum (thiserror)
  manifest/mod.rs     (240 lines)  - PluginManifest, PluginMetadata, PluginKind
  protocol/mod.rs     (254 lines)  - PluginRequest, PluginResponse, PluginOutput,
                                     PluginDiagnostic
  registry/mod.rs     (112 lines)  - PluginRegistry with lookup methods
  runner/mod.rs       (148 lines)  - PluginExecutor trait, PluginRunner<E>
  process.rs          (282 lines)  - SandboxExecutor implementation
  tests/mod.rs        (69 lines)   - mock executor factories, integration test
  tests/behaviour.rs  (264 lines)  - BDD test steps and scenario registration
```

Each module directory also contains a `tests.rs` file with unit tests.

BDD feature files live at `crates/weaver-plugins/tests/features/`.

### Key existing types

`PluginRequest` (in `protocol/mod.rs`): has fields `operation: String`,
`files: Vec<FilePayload>`, `arguments: HashMap<String, serde_json::Value>`. The
`arguments` map is untyped; the capability contract will define the required
shape for `rename-symbol`.

`PluginResponse` (in `protocol/mod.rs`): has fields `success: bool`,
`output: PluginOutput`, `diagnostics: Vec<PluginDiagnostic>`. The `output` enum
has variants `Diff`, `Analysis`, and `Empty`.

`PluginDiagnostic` (in `protocol/mod.rs`): has fields `severity`, `message`,
optional `file`, optional `line`. The capability contract will add an optional
`reason_code` field.

`PluginManifest` (in `manifest/mod.rs`): has fields `name`, `version`, `kind`,
`languages`, `executable`, `args`, `timeout_secs`. The capability contract will
add a `capabilities` field.

`PluginKind` (in `manifest/mod.rs`): enum with `Sensor` and `Actuator`
variants. Only `Actuator` plugins can declare actuator capabilities like
`rename-symbol`.

### ADR 001 context

`docs/adr-001-plugin-capability-model-and-act-extricate.md` defines the
capability model that motivates this work. It establishes five stable
capability IDs: `rename-symbol`, `extricate-symbol`, `extract-method`,
`replace-body`, `extract-predicate`. It mandates capability-first routing where
user intent maps to a capability ID, which maps to provider resolution.

### Downstream consumers

Roadmap items 5.2.2 and 5.2.3 will update the rope and rust-analyzer plugins to
declare `rename-symbol` in their manifests and conform to the contract. Item
5.2.4 will wire capability routing into the `weaverd` daemon. This plan defines
the types and validation those items will import.

## Plan of work

### Stage A: Scaffold capability module structure and types

Create the `capability/` module directory with four files:

1. `crates/weaver-plugins/src/capability/mod.rs` — Module-level doc comment
   explaining the capability contract system. `CapabilityId` enum with five
   variants matching ADR 001, serialized as kebab-case. `ContractVersion`
   struct with `major` and `minor` fields and `is_compatible_with()` method.
   `CapabilityContract` trait defining the validation interface. Re-exports for
   public API.

2. `crates/weaver-plugins/src/capability/rename_symbol.rs` — Module-level
   doc comment for the rename-symbol contract. `RenameSymbolRequest` struct
   defining the typed request shape with `extract()` method that validates the
   untyped arguments HashMap. `RenameSymbolContract` struct implementing
   `CapabilityContract`. `RENAME_SYMBOL_CONTRACT_VERSION` constant. Validation
   logic for request and response.

3. `crates/weaver-plugins/src/capability/reason_code.rs` — Module-level doc
   comment for refusal reason codes. `ReasonCode` enum with seven stable codes
   serialized as snake_case strings.

4. `crates/weaver-plugins/src/capability/tests.rs` — Unit tests covering
   serde round-trips, validation happy/unhappy paths, version compatibility,
   and reason code serialization.

Add the `capability` module to `lib.rs` and wire in the `#[cfg(test)]` `tests`
submodule.

Validation: `cargo check -p weaver-plugins` compiles. Unit tests pass.

### Stage B: Extend existing types

Add `capabilities: Vec<CapabilityId>` to `PluginManifest` with
`#[serde(default)]`, builder method, accessor, and validation (sensor plugins
must not declare capabilities). Update manifest unit tests.

Add `reason_code: Option<ReasonCode>` to `PluginDiagnostic` with
`#[serde(default, skip_serializing_if = "Option::is_none")]`, builder method,
and accessor. Update diagnostic unit tests.

Add `find_for_capability()` and `find_for_language_and_capability()` methods to
`PluginRegistry`. Update registry unit tests.

Validation: all existing tests pass. New tests pass. Serde backwards
compatibility confirmed (existing JSON without new fields deserializes
correctly).

### Stage C: BDD scenarios

Create `crates/weaver-plugins/tests/features/capability_contract.feature` with
scenarios covering the acceptance criteria. Add corresponding BDD step
definitions in `crates/weaver-plugins/src/tests/capability_behaviour.rs` and
register from `tests/mod.rs`.

Validation: all BDD scenarios pass.

### Stage D: Documentation and cleanup

Update `docs/users-guide.md` with a section on plugin capabilities covering
capability IDs, the rename-symbol request schema, and refusal reason codes.
Update `lib.rs` re-exports and ensure all rustdoc examples compile. Run full
quality gates. Mark roadmap entry as done.

Validation: `make check-fmt && make lint && make test` all pass.

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes. New unit tests in
  `crates/weaver-plugins/src/capability/tests.rs` cover all validation paths.
  New BDD scenarios in `capability_contract.feature` exercise the contract
  end-to-end through the validation layer.
- Lint: `make lint` passes with no warnings.
- Format: `make check-fmt` passes.
- Backwards compatibility: Existing plugin manifest JSON without a
  `capabilities` field deserializes correctly. Existing diagnostic JSON without
  a `reason_code` field deserializes correctly. All pre-existing tests pass
  unchanged.

Quality method (how we check):

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

Acceptance criteria from the roadmap:

1. **Capability contract is versioned:** The `RenameSymbolContract`
   implements `CapabilityContract::version()` returning
   `ContractVersion::new(1, 0)`. Unit test verifies the version value.
   `ContractVersion::is_compatible_with()` enables negotiation.

2. **Broker validation enforces schema shape:** The `validate_request()`
   and `validate_response()` methods on `RenameSymbolContract` check field
   presence, types, and output variant. Unit and BDD tests exercise both happy
   and unhappy paths.

3. **Refusal diagnostics use stable reason codes:** The `ReasonCode` enum
   provides seven stable codes serialized as snake_case strings. The
   `PluginDiagnostic::with_reason_code()` builder attaches a code to a
   diagnostic. BDD scenarios verify round-trip serialization.

## Idempotence and recovery

All stages are re-runnable. If a stage fails partway, delete any partially
created files in `crates/weaver-plugins/src/capability/` and re-run the stage
from the beginning. The capability module is additive; no existing files are
deleted or renamed.

## Artifacts and notes

### Line budget analysis

| File                          | Current lines | Planned additions | Projected total |
| ----------------------------- | ------------- | ----------------- | --------------- |
| `protocol/mod.rs`             | 254           | +20               | 274             |
| `manifest/mod.rs`             | 240           | +20               | 260             |
| `registry/mod.rs`             | 112           | +20               | 132             |
| `error/mod.rs`                | 112           | 0                 | 112             |
| `lib.rs`                      | 60            | +10               | 70              |
| `capability/mod.rs`           | 0 (new)       | ~120              | 120             |
| `capability/rename_symbol.rs` | 0 (new)       | ~150              | 150             |
| `capability/reason_code.rs`   | 0 (new)       | ~80               | 80              |

All files remain well within the 400-line limit.

### Dependency analysis

No new external crate dependencies are required. The implementation uses only
existing dependencies: `serde`, `serde_json`, `thiserror` (production);
`rstest`, `rstest-bdd`, `rstest-bdd-macros`, `mockall` (dev).

## Interfaces and dependencies

### New public types

In `crates/weaver-plugins/src/capability/mod.rs`:

```rust
pub enum CapabilityId {
    RenameSymbol,
    ExtricateSymbol,
    ExtractMethod,
    ReplaceBody,
    ExtractPredicate,
}

pub struct ContractVersion { major: u16, minor: u16 }

pub trait CapabilityContract {
    fn capability_id(&self) -> CapabilityId;
    fn version(&self) -> ContractVersion;
    fn validate_request(&self, request: &PluginRequest) -> Result<(), PluginError>;
    fn validate_response(&self, response: &PluginResponse) -> Result<(), PluginError>;
}
```

In `crates/weaver-plugins/src/capability/rename_symbol.rs`:

```rust
pub struct RenameSymbolRequest { uri: String, position: String, new_name: String }
pub struct RenameSymbolContract;
pub const RENAME_SYMBOL_CONTRACT_VERSION: ContractVersion;
```

In `crates/weaver-plugins/src/capability/reason_code.rs`:

```rust
pub enum ReasonCode {
    SymbolNotFound,
    MacroGenerated,
    AmbiguousReferences,
    UnsupportedLanguage,
    IncompletePayload,
    NameConflict,
    OperationNotSupported,
}
```

### Modified public types

In `crates/weaver-plugins/src/manifest/mod.rs`:

```rust
// PluginManifest gains:
pub fn capabilities(&self) -> &[CapabilityId];
pub fn with_capabilities(self, capabilities: Vec<CapabilityId>) -> Self;
```

In `crates/weaver-plugins/src/protocol/mod.rs`:

```rust
// PluginDiagnostic gains:
pub fn with_reason_code(self, code: ReasonCode) -> Self;
pub fn reason_code(&self) -> Option<ReasonCode>;
```

In `crates/weaver-plugins/src/registry/mod.rs`:

```rust
// PluginRegistry gains:
pub fn find_for_capability(&self, id: CapabilityId) -> Vec<&PluginManifest>;
pub fn find_for_language_and_capability(
    &self, language: &str, id: CapabilityId,
) -> Vec<&PluginManifest>;
```

### Downstream consumers (out of scope, for reference)

- 5.2.2: `crates/weaver-plugin-rope/` will import `CapabilityId` and
  declare `RenameSymbol` in its manifest.
- 5.2.3: `crates/weaver-plugin-rust-analyzer/` will import `CapabilityId`
  and declare `RenameSymbol` in its manifest.
- 5.2.4: `crates/weaverd/src/dispatch/act/refactor/` will import
  `RenameSymbolContract` and call `validate_request()`/`validate_response()`
  around plugin execution.
