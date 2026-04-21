# 4.1.2 Define structured diagnostics with stable `E_SEMPAI_*` error codes and report schema

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

After this change, Sempai diagnostics will have one stable machine-readable
contract regardless of whether the diagnostic is emitted by parsing logic or
semantic validation logic. Every emitted diagnostic payload will include:
`code`, `message`, `primary_span`, and `notes`, with stable `E_SEMPAI_*` codes
and snapshot-locked JSON output.

This directly fulfills roadmap item 4.1.2 in [docs/roadmap.md](../roadmap.md):

- Define structured diagnostics with stable `E_SEMPAI_*` error codes and report
  schema.
- Ensure JSON snapshots remain stable across parser and validator paths.

Observable outcome after implementation:

```plaintext
make check-fmt   # exits 0
make lint        # exits 0
make test        # exits 0
```

And additionally (because docs are updated):

```plaintext
make fmt          # exits 0
make markdownlint # exits 0
make nixie        # exits 0
```

## Constraints

- Keep the implementation aligned with
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md),
  especially the Diagnostics section and `E_SEMPAI_*` list.
- Preserve the `sempai` facade as the stable entrypoint. Any schema changes in
  `sempai_core` must remain consumable through `sempai` re-exports.
- Follow workspace lint policy (`[lints] workspace = true`) and do not add
  `#[allow(...)]`; use tightly scoped `#[expect(..., reason = "...")]` only when
  unavoidable.
- Keep files below 400 lines; split test or behaviour modules if needed.
- Add both unit tests and behaviour tests using `rstest-bdd` v0.5.0 for happy
  and unhappy paths, plus relevant edge cases.
- Snapshot tests must assert stable JSON for parser and validator diagnostic
  paths.
- Update documentation:
  - `docs/sempai-query-language-design.md` for design decisions.
  - `docs/users-guide.md` for user-visible diagnostic contract changes.
  - `docs/roadmap.md` mark 4.1.2 done when implementation is complete.
- Run all required quality gates before completion.

## Tolerances (exception triggers)

- Scope: if implementation exceeds 12 touched files (net), stop and escalate.
- Interface: if a breaking change to currently public Sempai API is required
  (for example, removing/renaming existing public methods without backward
  compatibility), stop and escalate.
- Dependencies: if any new third-party dependency is needed, stop and escalate.
- Iterations: if the same failing test/lint loop is attempted 5 times without
  progress, stop and escalate.
- Ambiguity: if schema naming is ambiguous (`span` vs `primary_span`) and the
  choice materially affects compatibility, stop and escalate with options.

## Risks

- Risk: Existing diagnostics use `span`, while acceptance language uses "primary
  span". Severity: medium. Likelihood: medium. Mitigation: Define a canonical
  JSON field (`primary_span`) and, if needed, support backward compatibility via
  serde aliasing and clear docs.

- Risk: Parser and validator crates are not yet implemented, so "both paths"
  must be represented via contract constructors/tests now. Severity: medium.
  Likelihood: high. Mitigation: Add explicit parser/validator constructor paths
  in `sempai_core::diagnostic` and lock both with snapshots and BDD scenarios.

- Risk: Strict Clippy lints on tests can block concise fixture-heavy behaviour
  tests. Severity: low. Likelihood: medium. Mitigation: Keep helpers small,
  split modules early, and use scoped `#[expect]` with reason only when
  structurally necessary.

## Progress

- [x] (2026-03-11 00:00Z) Drafted this ExecPlan from roadmap and design docs.
- [x] Stage A: Write failing unit/snapshot/BDD tests for schema stability.
- [x] Stage B: Implement diagnostic schema and stable code mappings.
- [x] Stage C: Stabilize parser/validator diagnostic construction paths.
- [x] Stage D: Update design/user docs and mark roadmap item 4.1.2 done.
- [x] Stage E: Run quality gates and capture evidence.

## Surprises & discoveries

- Observation: `sempai_core` already defines all target `E_SEMPAI_*` variants,
  but current contract wording and tests do not yet lock parser-vs-validator
  schema parity via snapshots. Evidence:
  [crates/sempai-core/src/diagnostic.rs](../../crates/sempai-core/src/diagnostic.rs)
  and
  [crates/sempai-core/src/tests/diagnostic_tests.rs](../../crates/sempai-core/src/tests/diagnostic_tests.rs).
  Impact: Work should focus on schema hardening and path parity, not inventing a
  new code set.

- Observation: `rstest-bdd` v0.5.0 is already pinned at workspace level.
  Evidence: [Cargo.toml](../../Cargo.toml). Impact: No dependency update is
  required; tests should reuse current setup.

- Observation: `insta::assert_json_snapshot!` is unavailable in the current
  `insta` configuration, while `assert_snapshot!` is available and stable.
  Evidence:
  [crates/sempai-core/src/tests/diagnostic_snapshot_tests.rs](../../crates/sempai-core/src/tests/diagnostic_snapshot_tests.rs).
  Impact: Snapshot tests should serialize deterministic pretty JSON strings and
  use `assert_snapshot!`.

## Decision log

- Decision: Keep this milestone narrowly focused on diagnostics contract
  infrastructure in `sempai_core`; defer parser implementation details to 4.1.3
  and validation engine logic to 4.1.4/4.1.5. Rationale: 4.1.2 is explicitly
  contract-focused and should reduce risk for follow-on parser and validator
  milestones. Date/Author: 2026-03-11 / Codex.

- Decision: Lock JSON schema with snapshot tests in addition to unit assertions.
  Rationale: Snapshots provide explicit, reviewable contract evidence and catch
  accidental field-shape drift. Date/Author: 2026-03-11 / Codex.

- Decision: Emit `primary_span` in JSON while accepting legacy `span` on input
  via serde aliasing. Rationale: This satisfies the roadmap contract and keeps
  backward compatibility for older payloads during transition. Date/Author:
  2026-03-14 / Codex.

## Outcomes & retrospective

Target outcome at completion:

1. Stable diagnostic payload contract exists and is documented.
1. Parser-path and validator-path diagnostics serialize to the same schema.
1. Unit, behavioural, and snapshot tests cover happy/unhappy/edge paths.
1. `docs/users-guide.md` explains the updated diagnostic contract.
1. `docs/sempai-query-language-design.md` records decisions made here.
1. Roadmap item 4.1.2 is checked off.
1. `make check-fmt`, `make lint`, and `make test` pass.

Retrospective notes:

- Implemented canonical diagnostic JSON keys (`code`, `message`, `primary_span`,
  `notes`) and maintained input compatibility for legacy `span` payloads through
  serde aliasing.
- Added parser/validator constructor helpers in `Diagnostic` and
  `DiagnosticReport` to encode path parity in the type surface.
- Added unit tests for happy/unhappy/edge paths, plus BDD scenarios and snapshot
  tests proving parser/validator schema consistency.
- Verified complete gate sequence with logs: `/tmp/4-1-2-make-fmt.log`,
  `/tmp/4-1-2-make-markdownlint.log`, `/tmp/4-1-2-make-nixie.log`,
  `/tmp/4-1-2-make-check-fmt.log`, `/tmp/4-1-2-make-lint.log`,
  `/tmp/4-1-2-make-test.log`.

## Context and orientation

Current Sempai state relevant to this milestone:

- `sempai_core` already exposes:
  - `DiagnosticCode` with `E_SEMPAI_*` variants.
  - `Diagnostic`, `SourceSpan`, and `DiagnosticReport`.
- The existing `Diagnostic` struct currently uses a `span` field, and tests
  validate construction/serde/display, but they do not enforce parser/validator
  contract parity with schema snapshots.
- `sempai` engine entrypoints remain stubs and currently emit `NOT_IMPLEMENTED`,
  which is outside the final parser/validator code paths.

Primary files for this work:

- [crates/sempai-core/src/diagnostic.rs](../../crates/sempai-core/src/diagnostic.rs)
- [crates/sempai-core/src/tests/diagnostic_tests.rs](../../crates/sempai-core/src/tests/diagnostic_tests.rs)
- [crates/sempai-core/src/tests/behaviour.rs](../../crates/sempai-core/src/tests/behaviour.rs)
- [crates/sempai-core/tests/features/sempai_core.feature](../../crates/sempai-core/tests/features/sempai_core.feature)
- [docs/sempai-query-language-design.md](../sempai-query-language-design.md)
- [docs/users-guide.md](../users-guide.md)
- [docs/roadmap.md](../roadmap.md)

## Plan of work

### Stage A: Define the contract in tests first (red phase)

Add failing tests that express the intended contract before editing production
diagnostic code:

- Unit tests:
  - Assert the serialized diagnostic object contains exactly `code`, `message`,
    `primary_span`, and `notes`.
  - Add parser-path and validator-path constructors/fixtures in tests and assert
    they serialize to the same shape.
  - Add unhappy-path tests for unknown code deserialization and malformed span
    payloads.
- Snapshot tests (`insta`):
  - Snapshot parser diagnostic report JSON.
  - Snapshot validator diagnostic report JSON.
  - Snapshot mixed-report ordering stability.
- BDD (`rstest-bdd` v0.5.0):
  - Happy path: parser and validator reports expose all required fields.
  - Unhappy path: invalid code payload fails with deterministic error text.
  - Edge path: `primary_span = null` remains explicit and stable.

Go/no-go:

- Do not proceed until at least one new test fails for the intended schema
  change.

### Stage B: Implement the schema and stable constructors (green phase)

Update `sempai_core` diagnostics implementation to satisfy Stage A tests:

- Make `Diagnostic` schema explicitly model primary span (`primary_span` in
  JSON).
- Keep accessors ergonomic and explicit (`primary_span()`), and preserve
  compatibility helpers only if required by existing callers.
- Add explicit constructors or helper functions for parser-path and
  validator-path diagnostics so both routes share one shape and code surface.
- Ensure `DiagnosticCode` display/serde remains stable and only emits the
  documented code set.

Go/no-go:

- Do not proceed until targeted unit and snapshot tests pass.

### Stage C: Behaviour coverage and hardening (refactor phase)

Refactor tests and step definitions for clarity and maintainability:

- If `behaviour.rs` approaches 400 lines, split into `diagnostic_behaviour.rs`
  and keep scenario registration clear.
- Ensure BDD scenarios verify observable behaviour rather than implementation
  internals.
- Remove duplicate assertions by reusing shared fixtures/helpers.

Go/no-go:

- Do not proceed until `cargo test -p sempai-core --all-targets --all-features`
  passes.

### Stage D: Documentation and roadmap synchronization

Update docs once implementation is stable:

- `docs/sempai-query-language-design.md`:
  - Record decisions (field naming, parser/validator parity, stability rules).
  - Include a canonical JSON diagnostic example.
- `docs/users-guide.md`:
  - Document the diagnostic schema users can rely on.
  - Clarify code semantics and parser/validator applicability.
- `docs/roadmap.md`:
  - Mark 4.1.2 as done only after all tests and gates pass.

Go/no-go:

- Do not finalize until Markdown and Rust quality gates pass.

## Concrete steps

Run from repository root (`/home/user/project`).

1. Establish baseline and create red tests.

```plaintext
cargo test -p sempai-core --all-targets --all-features
```

Expected: baseline passes before adding new assertions.

1. Add unit tests, BDD scenarios, and snapshots that encode the new contract.

```plaintext
cargo test -p sempai-core diagnostic --all-targets --all-features
```

Expected: new tests fail before implementation changes.

1. Implement schema and constructor changes in `diagnostic.rs`.

```plaintext
cargo test -p sempai-core --all-targets --all-features
```

Expected: all `sempai-core` tests pass; snapshot outputs are stable.

1. Update docs and roadmap, then run formatting/lint/test gates with logs.

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-2-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-2-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-2-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-2-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-2-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-2-make-test.log
```

Expected transcript endings:

```plaintext
... Finished `dev` profile ...
... test result: ok. <N> passed; 0 failed ...
```

## Validation and acceptance

Acceptance is satisfied when all are true:

- Diagnostics serialize with required fields: `code`, `message`, `primary_span`,
  `notes`.
- Parser-path and validator-path diagnostic JSON snapshots match the same schema
  contract and remain stable.
- Unit tests cover:
  - Happy path serialization and accessor behaviour.
  - Unhappy path deserialization failures.
  - Edge cases (null span, empty notes, mixed report ordering).
- Behaviour tests (`rstest-bdd` v0.5.0) cover happy, unhappy, and edge paths.
- Design and user documentation reflect the finalized behaviour.
- Roadmap item 4.1.2 is checked as complete.
- `make check-fmt`, `make lint`, and `make test` all exit 0.

## Idempotence and recovery

- All commands above are safe to re-run.
- Snapshot generation should be deterministic; if snapshots change unexpectedly,
  inspect diffs before accepting.
- If a documentation gate fails after code gates pass, fix docs and re-run only
  doc gates first, then re-run `make check-fmt`, `make lint`, `make test` to
  confirm no regressions.
- If tolerance triggers are hit, stop and record escalation details in
  `Decision Log` before proceeding.

## Artifacts and notes

Capture the following during implementation:

- Snapshot files proving parser/validator schema stability.
- Final gate logs:
  - `/tmp/4-1-2-make-check-fmt.log`
  - `/tmp/4-1-2-make-lint.log`
  - `/tmp/4-1-2-make-test.log`
  - `/tmp/4-1-2-make-markdownlint.log`
  - `/tmp/4-1-2-make-nixie.log`
- Brief changelog note summarizing final schema and code mapping.

## Interfaces and dependencies

Implementation should preserve and/or define these stable interfaces in
`sempai_core`:

```rust
pub enum DiagnosticCode {
    // stable E_SEMPAI_* variants
}

pub struct SourceSpan {
    // byte offsets and optional URI
}

pub struct Diagnostic {
    // code, message, primary_span, notes
}

pub struct DiagnosticReport {
    // ordered list of diagnostics
}
```

Recommended API additions for parser/validator parity:

```rust
impl Diagnostic {
    pub fn parser(... ) -> Self;
    pub fn validator(... ) -> Self;
    pub fn primary_span(&self) -> Option<&SourceSpan>;
}

impl DiagnosticReport {
    pub fn parser_error(... ) -> Self;
    pub fn validation_error(... ) -> Self;
}
```

Dependencies remain within existing workspace crates:

- `serde`, `serde_json`, `thiserror`
- `rstest`, `rstest-bdd`, `rstest-bdd-macros`
- `insta`

No new dependencies should be introduced for this milestone.
