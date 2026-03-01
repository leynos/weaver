# 4.1.1 Scaffold `sempai_core` and `sempai` with stable public types and facade entrypoints

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DONE

## Purpose / big picture

After this change, a downstream crate author can depend on the `sempai` facade
crate and access a stable set of public types for representing languages,
source spans, match results, capture bindings, diagnostics, and engine
configuration. The `sempai_core` crate provides the canonical definitions; the
`sempai` facade re-exports them and exposes a stub `Engine` struct whose
methods return structured "not yet implemented" diagnostics (never panics).

Running `cargo doc -p sempai --no-deps` produces complete, warning-free public
API documentation. Unit tests validate type construction, serde round-trips,
and diagnostic formatting. BDD scenarios exercise the public API surface from a
consumer's perspective.

Observable outcome: after all stages complete, the following commands succeed:

```plaintext
make check-fmt   # exits 0
make lint         # exits 0 (includes cargo doc --workspace --no-deps -D warnings)
make test         # exits 0, including all new sempai_core and sempai tests
cargo doc -p sempai --no-deps   # exits 0 with zero warnings
```

This satisfies roadmap task 4.1.1 from `docs/roadmap.md` (lines 347-350).

## Constraints

- `make check-fmt`, `make lint`, and `make test` must pass after all changes.
  These are defined in `Makefile` (lines 19-28, 34-35).
- The workspace uses edition 2024 and `rust-version = "1.85"`
  (`Cargo.toml` lines 18-21). Both new crates must inherit these.
- Both new crates must include `[lints] workspace = true` to inherit the strict
  lint configuration (`Cargo.toml` lines 51-125). This means over 60 denied
  Clippy lints including `unwrap_used`, `expect_used`, `indexing_slicing`,
  `string_slice`, `missing_docs`, `cognitive_complexity`, `allow_attributes`,
  `panic_in_result_fn`, `print_stdout`, and `print_stderr`.
- No panicking in library code. Because `unwrap_used`, `expect_used`, and
  `panic_in_result_fn` are all denied, Engine stub methods must return
  `Err(DiagnosticReport)` with a "not implemented" diagnostic — not `todo!()`,
  `unimplemented!()`, or `panic!()`.
- No single source file may exceed 400 lines (`AGENTS.md` line 31).
- Every module must begin with a `//!` doc comment explaining its purpose
  (`AGENTS.md` line 154).
- All public items must have `///` rustdoc comments (`AGENTS.md` line 156).
- Comments and documentation must use en-GB-oxendict spelling
  ("-ize" / "-yse" / "-our") (`AGENTS.md` line 24).
- `#[non_exhaustive]` must be used on public enums
  (`docs/sempai-query-language-design.md` line 191).
- Library crates use `thiserror`-derived error enums — no `eyre` or `anyhow`
  (`AGENTS.md` lines 220-227).
- All dependency versions use SemVer-compatible caret requirements
  (`AGENTS.md` lines 206-216).
- `rstest-bdd` v0.5.0 must be used for BDD tests
  (workspace `Cargo.toml` line 36).
- Use `str_to_string = "deny"` — use `String::from(...)` or `.into()` instead
  of `.to_string()` on `&str` values.
- Existing crate public APIs must not change.
- The ExecPlan file must also be written to
  `docs/execplans/4-1-1-scaffold-sempai-core-and-sempai.md`.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 30 files (net), stop
  and escalate.
- Interface: if any existing `pub` API signature in any existing crate must
  change, stop and escalate. This plan only adds new crates.
- Dependencies: no new external dependencies beyond those already in
  `[workspace.dependencies]` are required. If one becomes necessary, stop and
  escalate.
- Iterations: if tests still fail after 5 attempts at fixing a given issue,
  stop and escalate.
- Ambiguity: if the design document is ambiguous on a type definition and the
  choice materially affects the public API, stop and present options.

## Risks

- Risk: The `result_large_err = "deny"` lint may fire on
  `Result<T, DiagnosticReport>` if `DiagnosticReport` exceeds 128 bytes.
  Severity: medium. Likelihood: medium. Mitigation: Box the inner
  `Vec<Diagnostic>` or wrap the entire report in `Box<DiagnosticReport>` in the
  `Result` position. The existing `weaver-graph/src/error.rs` uses
  `Arc<std::io::Error>` for a similar reason.

- Risk: The `missing_const_for_fn = "deny"` lint may require `const fn` on
  trivial constructors and accessors where serde-derived types make `const`
  non-trivial. Severity: low. Likelihood: medium. Mitigation: Mark simple
  constructors as `const` where possible. For methods that return references to
  `String` or `Vec` fields, the lint typically does not fire because they are
  not `const`-eligible. If infeasible, use
  `#[expect(clippy::missing_const_for_fn, reason = "...")]`.

- Risk: The `shadow_reuse`, `shadow_same`, and `shadow_unrelated` lints are
  denied. Variable naming in tests must avoid shadowing. Severity: low.
  Likelihood: medium. Mitigation: Use distinct variable names in test functions.

- Risk: The `str_to_string = "deny"` lint prohibits `.to_string()` on `&str`.
  Severity: low. Likelihood: high. Mitigation: Use `String::from(...)` or
  `.into()` on string literals.

- Risk: `CaptureValue` uses `#[serde(tag = "kind")]` which requires
  struct-like variants (`Node { .. }` not `Node(..)`), or an adjacently-tagged
  approach. The design doc shows tuple variants. Severity: low. Likelihood:
  medium. Mitigation: Use `#[serde(tag = "kind", content = "value")]`
  (adjacently tagged) instead of internally tagged, which supports tuple
  variants. Or convert to a custom `Serialize`/`Deserialize` implementation.
  Evaluate during implementation.

## Progress

- [x] Stage A: Write ExecPlan file and register workspace members.
- [x] Stage B: Implement `sempai_core` types and diagnostics.
- [x] Stage C: Implement `sempai` facade with Engine stub and re-exports.
- [x] Stage D: Write unit tests for both crates.
- [x] Stage E: Write BDD tests with feature files.
- [x] Stage F: Documentation and roadmap updates.
- [x] Stage G: Final validation and commit gating.

## Surprises & discoveries

- The `result_large_err` risk did not materialise.
  `DiagnosticReport` is small enough (a single `Vec` pointer)
  that Clippy does not fire the lint.
- `missing_const_for_fn` fired on nearly every constructor and
  accessor in `sempai_core`. All simple constructors were made
  `const fn` without issue.
- `doc_markdown` lint fired on "HashiCorp" in doc comments.
  Rewrote to avoid the proper noun where possible, or used
  backtick-escaped forms.
- `option_if_let_else` fired on `match diagnostics.first()` in
  the `diagnostic_summary` helper. Refactored to
  `first().map_or_else(...)`.
- `too_many_arguments` fired on `Match::new` (5 params). Used
  `#[expect]` with a reason since the constructor mirrors the
  struct's five fields.
- `dead_code` fired on `QueryPlan::new` because it is only used
  `pub(crate)` and no internal callers exist yet. Used
  `#[expect]` with a reason explaining future use.

## Decision log

- **CaptureValue serde strategy**: Used adjacently tagged serde
  (`#[serde(tag = "kind", content = "value")]`) instead of
  internally tagged, because `CaptureValue` has tuple variants
  (`Node(CapturedNode)`, `Nodes(Vec<CapturedNode>)`). Internally
  tagged serde does not support tuple variants. This was
  identified as a risk in the plan and the adjacently tagged
  approach was chosen during implementation.
- **EngineConfig fields**: Made all fields private with `const`
  accessors to preserve encapsulation. A `const fn new()`
  constructor accepts all four fields.
- **QueryPlan `_plan` field**: Used `_plan: ()` as a private
  placeholder to prevent external construction via struct literal
  syntax. The `pub(crate) new()` constructor is the only way to
  create instances.

## Outcomes & retrospective

All acceptance criteria met:

1. `RUSTDOCFLAGS="-D warnings" cargo doc -p sempai --no-deps` exits 0
   with zero warnings.
2. All twelve public types are defined in `sempai_core` and re-exported
   by the `sempai` facade: `Language`, `LineCol`, `Span`,
   `CapturedNode`, `CaptureValue`, `Match`, `EngineConfig`,
   `DiagnosticCode`, `SourceSpan`, `Diagnostic`, `DiagnosticReport`,
   plus `Engine` and `QueryPlan` in the facade.
3. 63+ tests pass across both crates (unit, BDD, and doc tests) using
   `rstest-bdd` v0.5.0 with happy and unhappy path scenarios.
4. `make check-fmt`, `make lint`, and `make test` all exit 0.
5. `docs/users-guide.md` updated with Sempai query engine section.
6. Roadmap task 4.1.1 marked as done in `docs/roadmap.md`.

Net new files: 28 (two crates with source, tests, and features).
No existing crate APIs were modified.

## Context and orientation

The Weaver project is a Rust workspace with 12 crates under
`/home/user/project/crates/`. The workspace root `Cargo.toml` at
`/home/user/project/Cargo.toml` defines shared edition (2024), version (0.1.0),
rust-version (1.85), dependencies, and lint configuration. No `sempai` crates
currently exist.

The design document at `docs/sempai-query-language-design.md` specifies six
Sempai crates (Table 1, lines 67-74). This task scaffolds only the first two:

- `crates/sempai-core` (`sempai_core`): Data model, diagnostics, planning IR.
- `crates/sempai` (`sempai`): Facade crate, stable API. Re-exports from
  `sempai_core` and provides the `Engine` struct.

The remaining four crates (`sempai-yaml`, `sempai-dsl`, `sempai-ts`,
`sempai-fixtures`) are out of scope for task 4.1.1.

### Existing crate patterns (references)

`crates/weaver-graph/` demonstrates the standard module pattern:

- `Cargo.toml`: inherits workspace edition/version/lints, declares deps and
  dev-deps using workspace references (`crates/weaver-graph/Cargo.toml`).
- `src/lib.rs`: crate-level `//!` doc with examples, private `mod`
  declarations, selective `pub use` re-exports, `#[cfg(test)] mod tests;`
  (`crates/weaver-graph/src/lib.rs`).
- `src/error.rs`: `thiserror`-derived error enum with `#[must_use]`
  constructors (`crates/weaver-graph/src/error.rs`).
- `src/tests/mod.rs`: test root with inline unit tests and `mod behaviour;`
  declaration (`crates/weaver-graph/src/tests/mod.rs`).

`crates/weaver-plugins/` demonstrates the BDD testing pattern:

- `src/tests/behaviour.rs`: `TestWorld` struct with `#[fixture]`, step
  definitions via `#[given]`, `#[when]`, `#[then]` macros, `QuotedString`
  parameter type, and `#[scenario(path = "...")]` registration
  (`crates/weaver-plugins/src/tests/behaviour.rs`).
- `tests/features/plugin_execution.feature`: Gherkin feature file with
  happy and unhappy path scenarios
  (`crates/weaver-plugins/tests/features/plugin_execution.feature`).

### Key types from the design document

The design document (`docs/sempai-query-language-design.md` lines 197-298)
specifies the following public types:

1. `Language` enum: `Rust`, `Python`, `TypeScript`, `Go`, `Hcl` — with
   `#[non_exhaustive]`, `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`,
   `Serialize`, `Deserialize`.

2. `LineCol` struct: `line: u32`, `column: u32` — with `Serialize`,
   `Deserialize`.

3. `Span` struct: `start_byte: u32`, `end_byte: u32`, `start: LineCol`,
   `end: LineCol` — with `Serialize`, `Deserialize`.

4. `CapturedNode` struct: `span: Span`, `kind: String`,
   `text: Option<String>` — with `Serialize`, `Deserialize`.

5. `CaptureValue` enum: `Node(CapturedNode)`,
   `Nodes(Vec<CapturedNode>)` — with `#[non_exhaustive]`, tagged serde,
   `Serialize`, `Deserialize`.

6. `Match` struct: `rule_id: String`, `uri: String`, `span: Span`,
   `focus: Option<Span>`, `captures: BTreeMap<String, CaptureValue>` — with
   `Serialize`, `Deserialize`.

7. `EngineConfig` struct: `max_matches_per_rule: usize`,
   `max_capture_text_bytes: usize`, `max_deep_search_nodes: usize`,
   `enable_hcl: bool` — with `Default`.

8. `DiagnosticCode` enum: nine `E_SEMPAI_*` codes plus `NotImplemented` for
   stubs — with `#[non_exhaustive]`, `Display`, `Serialize`, `Deserialize`
   (design doc lines 969-981).

9. `SourceSpan` struct: `start: u32`, `end: u32`,
   `uri: Option<String>` — for diagnostic locations.

10. `Diagnostic` struct: `code: DiagnosticCode`, `message: String`,
    `span: Option<SourceSpan>`, `notes: Vec<String>`.

11. `DiagnosticReport` struct: wraps `Vec<Diagnostic>`, implements
    `std::error::Error` via `thiserror`.

12. `Engine` struct (in `sempai` facade): holds `EngineConfig`, exposes
    `compile_yaml`, `compile_dsl`, `execute` — all returning
    `Result<T, DiagnosticReport>`.

13. `QueryPlan` struct (in `sempai` facade): `rule_id: String`,
    `language: Language`, private placeholder plan field.

## Plan of work

### Stage A: Crate skeletons and workspace registration

This stage creates the directory structure, registers both crates in the
workspace, and verifies that the workspace compiles.

**A1.** Write the ExecPlan to
`docs/execplans/4-1-1-scaffold-sempai-core-and-sempai.md`.

**A2.** Create directory trees for both crates:

```plaintext
crates/sempai-core/src/
crates/sempai-core/tests/features/
crates/sempai/src/
crates/sempai/tests/features/
```

**A3.** Add `"crates/sempai-core"` and `"crates/sempai"` to the `members` array
in `Cargo.toml` (after `"crates/weaver-syntax"` on line 14).

**A4.** Create `crates/sempai-core/Cargo.toml`:

```toml
[package]
name = "sempai_core"
edition.workspace = true
version.workspace = true
rust-version.workspace = true

[dependencies]
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
rstest = { workspace = true }
rstest-bdd = { workspace = true }
rstest-bdd-macros = { workspace = true }
insta = { workspace = true }
serde_json = { workspace = true }

[lints]
workspace = true
```

**A5.** Create `crates/sempai/Cargo.toml`:

```toml
[package]
name = "sempai"
edition.workspace = true
version.workspace = true
rust-version.workspace = true

[dependencies]
sempai_core = { path = "../sempai-core" }

[dev-dependencies]
rstest = { workspace = true }
rstest-bdd = { workspace = true }
rstest-bdd-macros = { workspace = true }
serde_json = { workspace = true }

[lints]
workspace = true
```

**A6.** Create minimal `lib.rs` stubs so compilation succeeds. For
`crates/sempai-core/src/lib.rs`, a crate-level doc comment only. For
`crates/sempai/src/lib.rs`, a crate-level doc comment with a
`pub use sempai_core;` placeholder.

**A7.** Verify: `cargo check --workspace` exits 0.

### Stage B: Implement `sempai_core` types and diagnostics

Each type lives in a focused module to stay within the 400-line file limit. All
modules follow the weaver-graph pattern: private `mod` in `lib.rs`, selective
`pub use` re-exports.

**B1.** Create `crates/sempai-core/src/language.rs` containing the `Language`
enum with `#[non_exhaustive]`, all required derives,
`serde(rename_all = "snake_case")`, and a `Display` impl that produces
lowercase names matching the serde output (e.g. `"rust"`, `"python"`,
`"type_script"`, `"go"`, `"hcl"`).

**B2.** Create `crates/sempai-core/src/span.rs` containing `LineCol` and `Span`
structs with serde derives, `new()` constructors, and accessor methods.

**B3.** Create `crates/sempai-core/src/capture.rs` containing `CapturedNode`
and `CaptureValue`. `CaptureValue` uses `#[non_exhaustive]` and a serde tagging
strategy compatible with its tuple variants. Include `new()` and accessor
methods.

**B4.** Create `crates/sempai-core/src/match_result.rs` containing the `Match`
struct (named `match_result` because `match` is a Rust keyword). Include a
`new()` constructor and accessors.

**B5.** Create `crates/sempai-core/src/config.rs` containing `EngineConfig`
with a `Default` impl providing sensible defaults:
`max_matches_per_rule: 10_000`, `max_capture_text_bytes: 1_048_576` (1 MiB),
`max_deep_search_nodes: 100_000`, `enable_hcl: false`.

**B6.** Create `crates/sempai-core/src/diagnostic.rs` containing
`DiagnosticCode`, `SourceSpan`, `Diagnostic`, and `DiagnosticReport`.

`DiagnosticCode` is `#[non_exhaustive]` with ten variants: nine `E_SEMPAI_*`
codes from the design doc (lines 973-981) plus `NotImplemented` for stubs. Each
variant has a `Display` impl producing the string code (e.g.
`"E_SEMPAI_YAML_PARSE"`). Include `Serialize`, `Deserialize`.

`DiagnosticReport` wraps `Vec<Diagnostic>` and implements `std::error::Error`
via `thiserror`. Include a `not_implemented(feature: &str)` constructor that
creates a single-diagnostic report with `DiagnosticCode::NotImplemented`.

If the `result_large_err` lint fires on `DiagnosticReport`, box the
`diagnostics` field or provide a `Box<DiagnosticReport>` alias for use in
`Result` types.

**B7.** Wire up `crates/sempai-core/src/lib.rs` with module declarations and
`pub use` re-exports for all public types.

**B8.** Verify: `cargo check -p sempai_core` and
`cargo clippy -p sempai_core --all-targets -- -D warnings` both exit 0.

### Stage C: Implement `sempai` facade

**C1.** Create `crates/sempai/src/engine.rs` containing:

- `QueryPlan` struct with `rule_id: String`, `language: Language`, and a
  private `_plan: ()` placeholder field (prevents external construction).
  Public accessors for `rule_id()` and `language()`. A `pub(crate)` or `pub`
  `new()` constructor.

- `Engine` struct holding `EngineConfig`. Three stub methods:
  - `compile_yaml` — returns
    `Result<Vec<QueryPlan>, DiagnosticReport>`
  - `compile_dsl` — returns
    `Result<QueryPlan, DiagnosticReport>`
  - `execute` — returns
    `Result<Vec<Match>, DiagnosticReport>`

  Each returns `Err(DiagnosticReport::not_implemented("feature_name"))`.

**C2.** Wire up `crates/sempai/src/lib.rs` with re-exports from `sempai_core`
and the `engine` module:

```rust
pub use sempai_core::{
    CaptureValue, CapturedNode, Diagnostic, DiagnosticCode,
    DiagnosticReport, EngineConfig, Language, LineCol, Match,
    SourceSpan, Span,
};
pub use engine::{Engine, QueryPlan};
```

**C3.** Verify: `cargo check -p sempai` and
`RUSTDOCFLAGS="-D warnings" cargo doc -p sempai --no-deps` both exit 0.

### Stage D: Unit tests

Tests follow the weaver-graph pattern: `src/tests/mod.rs` as test root with
sub-modules for each concern.

**D1.** Create `crates/sempai-core/src/tests/mod.rs` importing sub-modules:

```rust
//! Unit tests for sempai_core types.

mod capture_tests;
mod config_tests;
mod diagnostic_tests;
mod language_tests;
mod match_tests;
mod span_tests;
```

**D2.** Create test files for `sempai_core`:

- `language_tests.rs`: `rstest` parameterised tests for `Display` output on
  each variant, serde JSON round-trip for each variant.
- `span_tests.rs`: `LineCol` and `Span` construction, serde round-trip.
- `capture_tests.rs`: `CapturedNode` and `CaptureValue` construction, serde
  round-trip, verify tagged `"kind"` field in JSON output.
- `match_tests.rs`: `Match` construction with empty and populated captures,
  serde round-trip, verify `BTreeMap` ordering preserved.
- `config_tests.rs`: `EngineConfig::default()` returns expected defaults,
  custom construction.
- `diagnostic_tests.rs`: `DiagnosticCode` Display for each variant,
  `DiagnosticReport::not_implemented` constructor, `DiagnosticReport` as
  `std::error::Error` (Display output), serde round-trip for `Diagnostic` and
  `DiagnosticReport`, `SourceSpan` construction and serde round-trip.

**D3.** Create `crates/sempai/src/tests/mod.rs` with sub-modules:

```rust
//! Unit tests for the sempai facade crate.

mod engine_tests;
mod reexport_tests;
```

- `engine_tests.rs`: `Engine::new` with default config, `compile_yaml` returns
  `Err` with `NotImplemented`, `compile_dsl` returns `Err` with
  `NotImplemented`, `execute` returns `Err` with `NotImplemented`, `QueryPlan`
  accessors.
- `reexport_tests.rs`: verify all re-exported types are accessible via
  `crate::` paths (compile-time check with simple construction).

**D4.** Verify: `cargo test -p sempai_core -p sempai` exits 0.

### Stage E: BDD tests

BDD tests follow the weaver-plugins pattern: `.feature` files in
`tests/features/`, step definitions in `src/tests/behaviour.rs`, scenario
registration via `#[scenario]` macro.

**E1.** Create `crates/sempai-core/tests/features/sempai_core.feature` with
scenarios covering:

- Happy path: Span serializes to JSON with byte and line/column fields.
- Happy path: Language enum round-trips through serde for each variant.
- Happy path: DiagnosticReport formats with code and message.
- Unhappy path: DiagnosticReport with NotImplemented code displays the
  feature name.

**E2.** Create `crates/sempai-core/src/tests/behaviour.rs` with a `TestWorld`
struct, fixtures, given/when/then steps, and scenario registration.

**E3.** Create `crates/sempai/tests/features/sempai_engine.feature` with
scenarios covering:

- Unhappy path: Engine `compile_yaml` returns not-implemented error.
- Unhappy path: Engine `compile_dsl` returns not-implemented error.
- Unhappy path: Engine `execute` returns not-implemented error.
- Happy path: Engine is constructible with default configuration.

**E4.** Create `crates/sempai/src/tests/behaviour.rs` with corresponding step
definitions.

**E5.** Verify: `cargo test -p sempai_core -p sempai` exits 0.

### Stage F: Documentation and roadmap updates

**F1.** Update `docs/users-guide.md` with a "Sempai query engine" subsection
documenting the existence of the `sempai` and `sempai_core` crates, the
available public types, and a note that engine methods are stubbed pending
backend implementation.

**F2.** Mark roadmap task 4.1.1 as done in `docs/roadmap.md` (line 347): change
`- [ ] 4.1.1.` to `- [x] 4.1.1.`.

**F3.** Record design decisions taken in the design document
`docs/sempai-query-language-design.md` if any type definitions diverge from the
design (e.g. serde tagging strategy for `CaptureValue`).

**F4.** Run `make fmt` to format all changed files.

### Stage G: Final validation and commit gating

**G1.** Run the full commit gating suite:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

All three must exit 0.

**G2.** Verify cargo doc specifically for sempai:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc -p sempai --no-deps 2>&1 | tee /tmp/sempai-doc.log
```

Must exit 0.

## Concrete steps

All commands run from the workspace root `/home/user/project`.

### Stage A

1. Create directory trees:

   ```bash
   mkdir -p crates/sempai-core/src crates/sempai-core/tests/features
   mkdir -p crates/sempai/src crates/sempai/tests/features
   ```

2. Edit `Cargo.toml`: add `"crates/sempai-core"` and `"crates/sempai"` to
   `members`.

3. Create `crates/sempai-core/Cargo.toml` and `crates/sempai/Cargo.toml` (see
   Stage A plan above).

4. Create minimal `lib.rs` stubs.

5. Verify: `cargo check --workspace` — expect exit 0.

### Stage B

1. Create six module files under `crates/sempai-core/src/`.

2. Wire modules and re-exports in `crates/sempai-core/src/lib.rs`.

3. Verify: `cargo clippy -p sempai_core --all-targets -- -D warnings` —
   expect exit 0.

### Stage C

1. Create `crates/sempai/src/engine.rs`.

2. Wire re-exports in `crates/sempai/src/lib.rs`.

3. Verify: `RUSTDOCFLAGS="-D warnings" cargo doc -p sempai --no-deps` —
   expect exit 0.

### Stage D

1. Create `crates/sempai-core/src/tests/mod.rs` and six test sub-modules.

2. Create `crates/sempai/src/tests/mod.rs` and two test sub-modules.

3. Verify: `cargo test -p sempai_core -p sempai` — expect all tests pass.

### Stage E

1. Create `.feature` files and behaviour step definitions.

2. Verify: `cargo test -p sempai_core -p sempai` — expect all tests pass.

### Stage F

1. Update `docs/users-guide.md`, `docs/roadmap.md`.

2. Run `make fmt`.

### Stage G

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

Expected: all exit 0.

## Validation and acceptance

**Acceptance criteria** (from roadmap task 4.1.1):

1. Public API documentation builds for `sempai` — verified by
   `RUSTDOCFLAGS="-D warnings" cargo doc -p sempai --no-deps` exiting 0.
2. Stable types cover language, span, match, capture, and diagnostics models —
   verified by unit tests constructing and round-tripping every type.
3. Unit tests and BDD tests using rstest-bdd v0.5.0 covering happy and unhappy
   paths — verified by `make test` passing.
4. `make check-fmt`, `make lint`, `make test` all succeed — verified by
   Stage G.
5. `docs/users-guide.md` updated — verified by Stage F.
6. Roadmap entry marked as done — verified by Stage F.

**Quality criteria:**

- Tests: `make test` passes with zero exit code including all new tests.
  Expected: approximately 30-40 new tests across both crates.
- Lint: `make lint` passes (Clippy pedantic with warnings denied, rustdoc
  with warnings denied).
- Format: `make check-fmt` reports no violations.
- Documentation: `cargo doc -p sempai --no-deps` with `-D warnings` produces
  no warnings.

**Quality method:**

```bash
make check-fmt && make lint && make test
```

## Idempotence and recovery

All steps are file creations or edits and can be re-applied. If any step fails
partway through, the working tree can be reset with `git checkout -- .` and the
steps re-executed from the beginning. No external state is modified. The crate
creation steps use `mkdir -p` (no failure on existing directory).

To roll back entirely: remove `crates/sempai-core/` and `crates/sempai/` from
the filesystem and their entries from the workspace `members` list.

## Artifacts and notes

N/A (to be populated during implementation).

## Interfaces and dependencies

### New crate: `sempai_core`

Dependencies: `serde` 1.0 (workspace, with `derive`), `thiserror` 2.0
(workspace).

Dev-dependencies: `rstest` 0.26.1, `rstest-bdd` 0.5.0, `rstest-bdd-macros`
0.5.0, `insta` 1.41, `serde_json` 1.0 (all workspace).

### New crate: `sempai`

Dependencies: `sempai_core` (path `"../sempai-core"`).

Dev-dependencies: `rstest`, `rstest-bdd`, `rstest-bdd-macros`, `serde_json`
(all workspace).

### Types defined by `sempai_core`

In `crates/sempai-core/src/language.rs`:

```rust
/// A supported host language identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Language {
    /// The Rust programming language.
    Rust,
    /// The Python programming language.
    Python,
    /// The TypeScript programming language.
    TypeScript,
    /// The Go programming language.
    Go,
    /// HashiCorp Configuration Language.
    Hcl,
}
```

In `crates/sempai-core/src/span.rs`:

```rust
/// A line and column position within a source file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineCol {
    /// Zero-indexed line number.
    pub line: u32,
    /// Zero-indexed column number (byte offset within the line).
    pub column: u32,
}

/// A byte and line/column span in a UTF-8 source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start_byte: u32,
    /// End byte offset (exclusive).
    pub end_byte: u32,
    /// Start position as line and column.
    pub start: LineCol,
    /// End position as line and column.
    pub end: LineCol,
}
```

In `crates/sempai-core/src/diagnostic.rs`:

```rust
/// Stable error codes for Sempai diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// YAML rule file parse failure.
    ESempaiYamlParse,
    /// One-liner DSL parse failure.
    ESempaiDslParse,
    /// Schema validation failure.
    ESempaiSchemaInvalid,
    /// Unsupported execution mode.
    ESempaiUnsupportedMode,
    /// Negated branch inside pattern-either/any.
    ESempaiInvalidNotInOr,
    /// Conjunction with no positive match-producing term.
    ESempaiMissingPositiveTermInAnd,
    /// Pattern snippet failed to parse as host language.
    ESempaiPatternSnippetParseFailed,
    /// Unsupported constraint in current context.
    ESempaiUnsupportedConstraint,
    /// Invalid Tree-sitter query syntax.
    ESempaiTsQueryInvalid,
    /// Feature not yet implemented (used by stub methods).
    NotImplemented,
}

/// A collection of diagnostics produced during compilation or execution.
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
#[error("{}", diagnostic_summary(&self.diagnostics))]
pub struct DiagnosticReport {
    diagnostics: Vec<Diagnostic>,
}
```

### Types defined by `sempai` facade

In `crates/sempai/src/engine.rs`:

```rust
/// A compiled query plan for one rule and one language.
#[derive(Debug, Clone)]
pub struct QueryPlan {
    rule_id: String,
    language: Language,
    _plan: (),
}

/// Compiles and executes Semgrep-compatible queries on Tree-sitter syntax
/// trees.
pub struct Engine {
    config: EngineConfig,
}

impl Engine {
    /// Creates a new engine with the given configuration.
    pub const fn new(config: EngineConfig) -> Self;

    /// Returns the engine configuration.
    pub const fn config(&self) -> &EngineConfig;

    /// Compiles a YAML rule file into query plans.
    ///
    /// # Errors
    /// Returns a diagnostic report. Currently returns a "not implemented"
    /// diagnostic for all inputs.
    pub fn compile_yaml(
        &self, yaml: &str,
    ) -> Result<Vec<QueryPlan>, DiagnosticReport>;

    /// Compiles a one-liner query DSL expression into a query plan.
    ///
    /// # Errors
    /// Returns a diagnostic report. Currently returns a "not implemented"
    /// diagnostic for all inputs.
    pub fn compile_dsl(
        &self, rule_id: &str, language: Language, dsl: &str,
    ) -> Result<QueryPlan, DiagnosticReport>;

    /// Executes a compiled query plan against a source snapshot.
    ///
    /// # Errors
    /// Returns a diagnostic report. Currently returns a "not implemented"
    /// diagnostic for all inputs.
    pub fn execute(
        &self, plan: &QueryPlan, uri: &str, source: &str,
    ) -> Result<Vec<Match>, DiagnosticReport>;
}
```

## Files created and modified (summary)

| File                                                      | Change |
| --------------------------------------------------------- | ------ |
| `Cargo.toml`                                              | Edit   |
| `crates/sempai-core/Cargo.toml`                           | New    |
| `crates/sempai-core/src/lib.rs`                           | New    |
| `crates/sempai-core/src/language.rs`                      | New    |
| `crates/sempai-core/src/span.rs`                          | New    |
| `crates/sempai-core/src/capture.rs`                       | New    |
| `crates/sempai-core/src/match_result.rs`                  | New    |
| `crates/sempai-core/src/config.rs`                        | New    |
| `crates/sempai-core/src/diagnostic.rs`                    | New    |
| `crates/sempai-core/src/tests/mod.rs`                     | New    |
| `crates/sempai-core/src/tests/language_tests.rs`          | New    |
| `crates/sempai-core/src/tests/span_tests.rs`              | New    |
| `crates/sempai-core/src/tests/capture_tests.rs`           | New    |
| `crates/sempai-core/src/tests/match_tests.rs`             | New    |
| `crates/sempai-core/src/tests/config_tests.rs`            | New    |
| `crates/sempai-core/src/tests/diagnostic_tests.rs`        | New    |
| `crates/sempai-core/src/tests/behaviour.rs`               | New    |
| `crates/sempai-core/tests/features/sempai_core.feature`   | New    |
| `crates/sempai/Cargo.toml`                                | New    |
| `crates/sempai/src/lib.rs`                                | New    |
| `crates/sempai/src/engine.rs`                             | New    |
| `crates/sempai/src/tests/mod.rs`                          | New    |
| `crates/sempai/src/tests/engine_tests.rs`                 | New    |
| `crates/sempai/src/tests/reexport_tests.rs`               | New    |
| `crates/sempai/src/tests/behaviour.rs`                    | New    |
| `crates/sempai/tests/features/sempai_engine.feature`      | New    |
| `docs/users-guide.md`                                     | Edit   |
| `docs/roadmap.md`                                         | Edit   |
| `docs/execplans/4-1-1-scaffold-sempai-core-and-sempai.md` | New    |
