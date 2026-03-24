# Add unit, behavioural, and end-to-end coverage for `rename-symbol`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

Roadmap item 5.2.5 closes the test-coverage gap left after the preceding
plugin-migration work (5.2.1 through 5.2.4). The Python rope plugin, the Rust
rust-analyzer plugin, and the daemon-side capability-resolution layer all
shipped with per-milestone tests, but they do not yet share a unified contract
test suite, nor do they systematically cover rollback guarantees, cross-plugin
behavioural parity, or end-to-end flows under the new capability architecture.

After this change:

- Both plugins pass a shared set of contract fixtures that validate request
  schema conformance, response shape, reason-code semantics, and refusal
  behaviour identically for Python and Rust rename flows.
- Unit tests cover success paths, refusal paths, argument validation, and
  adapter-failure propagation in each plugin crate and in the daemon resolution
  layer.
- Behavioural tests (BDD via `rstest-bdd` v0.5.0) cover happy paths, unhappy
  paths, and edge cases across all three layers (plugin contract, plugin
  execution, daemon routing).
- End-to-end tests in `weaver-e2e` validate that the CLI-to-daemon-to-plugin
  pipeline produces correct observable output for both automatic and explicit
  provider routing, and that refusal scenarios emit deterministic structured
  diagnostics.
- Rollback guarantees are proven: every refusal path and every adapter-failure
  path leaves the filesystem unchanged, and this invariant is asserted
  explicitly in the behavioural and end-to-end tests.
- `docs/users-guide.md` is reviewed and updated if any observable behaviour
  changes surface during testing.
- `docs/roadmap.md` marks 5.2.5 as done.

Observable success: running `make test` passes with the new tests included, and
the shared contract fixtures produce identical pass/fail verdicts for both
plugins.

## Constraints

- The `rename-symbol` capability contract defined in
  `crates/weaver-plugins/src/capability/` is stable. This plan must not
  redefine the contract schema introduced by 5.2.1.
- The daemon resolution layer in
  `crates/weaverd/src/dispatch/act/refactor/resolution.rs` is stable. This plan
  must not change resolution semantics; it adds coverage only.
- The CLI command shape is stable. `--refactoring rename`, `offset`, and
  `new_name` remain the operator-facing inputs. `--provider` is optional.
- Preserve synchronous execution. Do not introduce async runtimes, async
  traits, or background work queues.
- The repository enforces a 400-line-per-file limit. New test files must
  respect this budget. Existing files near the limit must be split before
  growing.
- Behavioural tests must use `rstest-bdd` v0.5.0 patterns already used in the
  workspace, including mutable fixtures named exactly `world`.
- Comments and documentation must use en-GB-oxendict spelling ("-ize" /
  "-yse" / "-our").
- Lint suppressions remain a last resort. If unavoidable, use tightly scoped
  `#[expect(..., reason = "...")]` rather than `#[allow(...)]`.
- No new external dependencies should be added. Reuse existing workspace
  crates.
- Any design decision taken during implementation must be recorded in this
  ExecPlan.
- The final implementation must pass `make check-fmt`, `make lint`, and
  `make test`. Because this item also updates Markdown documents, `make fmt`,
  `make markdownlint`, and `make nixie` must also pass.
- The `rstest-bdd` fixture parameter must be named `world`, not `_world`
  (fixture matching is by name). Use `let _ = world;` to suppress
  unused-variable warnings.
- Workspace Clippy denies warnings across all targets, including tests. Helper
  functions with many parameters must group them into small context structs to
  avoid `clippy::too_many_arguments`.
- Insta snapshots with rstest parameterised tests need explicit names to avoid
  execution-order-dependent numbering.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 20 files or roughly
  1,200 net lines, stop and escalate.
- Interface: if satisfying the acceptance criteria requires a breaking change
  to the public `weaver-cli` command syntax or to the public `weaver-plugins`
  request/response contract, stop and escalate.
- Dependencies: if a new crate dependency appears necessary, stop and escalate.
- Iterations: if `make lint` or `make test` still fail after 5 repair loops,
  stop and escalate with the failing logs and current hypothesis.
- Ambiguity: if acceptance criteria are ambiguous about what "shared contract
  fixtures" means, stop and confirm before proceeding.

## Risks

- Risk: existing test files in
  `crates/weaverd/src/dispatch/act/refactor/tests.rs` (337 lines) and
  `crates/weaverd/src/dispatch/act/refactor/behaviour.rs` (351 lines) are near
  the 400-line budget. Adding new tests there may require splitting. Severity:
  medium. Likelihood: high. Mitigation: add new test coverage in new focused
  test modules rather than growing existing files.

- Risk: shared contract fixtures require both plugins to accept the same input
  shape. The rope plugin currently accepts a bare `uri` string while
  rust-analyzer expects a `file:///` URI. Fixtures must account for this
  difference. Severity: medium. Likelihood: high. Mitigation: design the shared
  fixture data at the `weaver-plugins` contract-validation level (which
  validates the abstract schema shape) rather than at the plugin-execution
  level (which handles URI normalisation internally). Each plugin's
  execution-level tests continue to use their own URI conventions.

- Risk: end-to-end tests in `weaver-e2e` use real `weaver` binary invocations
  with a `FakeDaemon`. Adding new e2e tests for capability-routed flows
  requires the fake daemon to understand the resolution envelope. Severity:
  medium. Likelihood: medium. Mitigation: extend the existing `FakeDaemon`
  response logic to handle both explicit-provider and automatic-routing request
  shapes, or add focused e2e tests that capture the CLI request shape without
  requiring a fully capable daemon response.

- Risk: rollback-guarantee tests need to assert filesystem immutability on
  failure. The existing BDD harness in `weaverd` uses in-memory response
  writers rather than real files. Severity: low. Likelihood: medium.
  Mitigation: assert rollback at the handler level (response status and no
  apply-patch invocation) rather than at the filesystem level. End-to-end tests
  that use temporary directories can assert filesystem immutability directly.

## Progress

- [ ] Reviewed `AGENTS.md`, the roadmap entry, the preceding ExecPlans
  (5.2.1 through 5.2.4), and project memory notes.
- [ ] Inspected current test coverage across all relevant crates.
- [ ] Drafted this ExecPlan.

## Surprises & Discoveries

(No surprises yet; this section will be updated during implementation.)

## Decision Log

- Decision: place shared contract fixtures in
  `crates/weaver-plugins/src/capability/` as a `test-support` feature-gated
  module rather than duplicating fixtures across plugin crates. Rationale: the
  capability contract is defined in `weaver-plugins`, and the acceptance
  criteria require "shared contract fixtures". A `test-support` feature flag
  follows the established pattern used by `sempai-core`. Both plugin crates can
  import shared fixtures via `weaver-plugins/test-support` dev-dependency.
  Date: 2026-03-24.

- Decision: structure the shared contract fixtures as parameterised `rstest`
  cases that exercise the `RenameSymbolContract::validate_request()` and
  `RenameSymbolContract::validate_response()` methods with a fixed set of
  inputs (valid request, missing fields, empty fields, wrong operation, valid
  diff response, non-diff response, failure with reason code). Rationale: this
  validates that both plugins conform to the same abstract contract without
  coupling the fixtures to plugin-specific execution details like URI
  normalisation or adapter mocking. Date: 2026-03-24.

- Decision: add new BDD feature files rather than growing existing ones past
  the 400-line budget. The new files are:
  `crates/weaver-plugin-rope/tests/features/rename_symbol_contract.feature`,
  `crates/weaver-plugin-rust-analyzer/tests/features/rename_symbol_contract.feature`,
   and `crates/weaverd/tests/features/rename_symbol_coverage.feature`.
  Rationale: keeps each feature file focused and within budget. Date:
  2026-03-24.

## Context and orientation

The rename-symbol capability spans four crates and one documentation layer.
This section orients a newcomer to each area.

### Capability contract (`crates/weaver-plugins/`)

The `weaver-plugins` crate defines the shared plugin infrastructure. The
`src/capability/` module contains:

- `mod.rs`: `CapabilityId` enum (5 variants including `RenameSymbol`),
  `ContractVersion` struct, `CapabilityContract` trait with
  `validate_request()` and `validate_response()` methods.
- `rename_symbol.rs`: `RenameSymbolContract` implementation and
  `RenameSymbolRequest` typed extraction struct.
- `reason_code.rs`: `ReasonCode` enum (7 variants including
  `SymbolNotFound`, `IncompletePayload`, `OperationNotSupported`).
- `tests.rs`: 332-line unit test file for contract validation.

BDD tests live in `tests/features/capability_contract.feature` (76 lines, 13
scenarios) with step definitions in `src/tests/capability_behaviour.rs` (268
lines).

### Python rope plugin (`crates/weaver-plugin-rope/`)

The rope plugin declares `CapabilityId::RenameSymbol` in its manifest. Source
files:

- `src/lib.rs` (396 lines): `RopeAdapter` trait, `execute_request()`,
  `run_with_adapter()`.
- `src/arguments.rs` (93 lines): `parse_rename_symbol_arguments()`.
- `src/tests/mod.rs` (271 lines): unit tests with `mockall` `MockAdapter`.
- `src/tests/behaviour.rs` (210 lines): BDD step definitions.
- `tests/features/rope_plugin.feature` (34 lines, 5 scenarios).

### Rust rust-analyzer plugin (`crates/weaver-plugin-rust-analyzer/`)

The rust-analyzer plugin declares `CapabilityId::RenameSymbol` in its manifest.
Source files:

- `src/lib.rs` (381 lines): `RustAnalyzerAdapter` trait,
  `execute_request()`.
- `src/arguments.rs` (94 lines): `parse_rename_symbol_arguments()`.
- `src/failure.rs` (52 lines): `PluginFailure` struct.
- `src/tests/mod.rs` (151 lines): test coordination.
- `src/tests/support.rs` (112 lines): shared `MockAdapter` builders,
  `rename_arguments()`, `request_with_args()`, `file_uri_for_path()`.
- `src/tests/argument_validation.rs` (102 lines): argument schema tests.
- `src/tests/behaviour.rs` (214 lines): BDD step definitions.
- `src/tests/dispatch_layer.rs` (101 lines): dispatch routing tests.
- `tests/features/rust_analyzer_plugin.feature` (43 lines, 6 scenarios).

### Daemon resolution layer (`crates/weaverd/`)

The `act refactor` handler lives in `src/dispatch/act/refactor/`. Key modules:

- `mod.rs` (352 lines): main handler, `RefactorPluginRuntime` trait,
  `prepare_plugin_request()`, `apply_rename_symbol_mapping()`, `to_file_uri()`.
- `resolution.rs` (370 lines): `resolve_provider()`,
  `CapabilityResolutionEnvelope`, `ResolutionRequest`, `SelectionMode`,
  `ResolutionOutcome`, `RefusalReason`, `CandidateEvaluation`.
- `arguments.rs` (136 lines): `RefactorArgs`, `parse_refactor_args()`.
- `manifests.rs` (34 lines): `rope_manifest()`,
  `rust_analyzer_manifest()`.
- `candidates.rs` (58 lines): `manifest_supports_language()`,
  `provider_rank()`.
- `refusal.rs` (36 lines): `RoutingContext`, `refused()`.
- `response_handling.rs` (65 lines): `handle_plugin_response()`.
- `refactor_helpers.rs` (189 lines): shared test/routing helpers.
- `behaviour.rs` (351 lines): BDD step definitions.
- `contract_tests.rs` (226 lines): contract validation tests.
- `resolution_tests.rs` (124 lines): provider resolution unit tests.
- `tests.rs` (337 lines): integration tests.
- `tests/features/refactor.feature` (52 lines, 6 scenarios).

### End-to-end (`crates/weaver-e2e/`)

The `weaver-e2e` crate contains CLI ergonomics snapshot tests:

- `tests/refactor_rope_cli_snapshots.rs` (296 lines): rope CLI snapshots
  with `FakeDaemon`.
- `tests/refactor_rust_analyzer_cli_snapshots.rs` (296 lines):
  rust-analyzer CLI snapshots with `FakeDaemon`.

### Documentation

- `docs/users-guide.md`: documents `act refactor` syntax, parameter
  semantics, routing rationale, and plugin inventory.
- `docs/roadmap.md`: section 5.2.5 is currently unchecked.

## Plan of work

The work is organised into five milestones. Each milestone ends with a
validation step that must pass before proceeding.

### Milestone 1: Shared contract test fixtures in `weaver-plugins`

Create a `test-support` feature-gated module in `weaver-plugins` that exports
shared contract fixture data and helper functions. Both plugin crates will
import these fixtures to prove they conform to the same `rename-symbol`
contract.

In `crates/weaver-plugins/Cargo.toml`, add a `test-support` feature flag (no
additional dependencies).

In `crates/weaver-plugins/src/lib.rs`, add a
`#[cfg(any(test, feature = "test-support"))]` public module `test_support`.

Create `crates/weaver-plugins/src/test_support.rs` containing:

- `valid_rename_request()`: returns a `PluginRequest` with operation
  `"rename-symbol"` and all three required arguments (`uri`, `position`,
  `new_name`) populated with representative values.
- `valid_rename_response()`: returns a `PluginResponse` with a
  `PluginOutput::Diff` payload.
- `failure_response_with_reason(reason: ReasonCode)`: returns a failure
  `PluginResponse` with a `PluginDiagnostic` carrying the given reason code.
- `contract_fixture_cases()`: a set of
  `(description, PluginRequest, expected_valid: bool)` tuples covering the
  canonical contract scenarios: valid request, missing `uri`, missing
  `position`, missing `new_name`, empty `uri`, empty `position`, empty
  `new_name`, wrong operation.

These helpers are intentionally stateless and deterministic. They validate the
abstract contract schema, not plugin-specific execution behaviour.

Add unit tests in `crates/weaver-plugins/src/capability/tests.rs` (or a new
sibling test module if the file is near budget) that call the shared fixtures
and assert the expected validation outcomes.

Validation: `cargo test -p weaver-plugins` passes.
`cargo clippy -p weaver-plugins --all-targets --all-features -- -D warnings`
passes.

### Milestone 2: Plugin-level unit and BDD coverage

For each plugin crate (`weaver-plugin-rope` and `weaver-plugin-rust-analyzer`),
add coverage for the gaps identified below. Both crates already have unit tests
and BDD scenarios; this milestone extends them.

#### 2a: Shared contract conformance tests

In each plugin crate's `Cargo.toml`, add
`weaver-plugins = { path = "../weaver-plugins", features = ["test-support"] }`
to `[dev-dependencies]`.

Create a new test module in each plugin crate
(`src/tests/contract_conformance.rs`) that imports the shared fixtures from
`weaver_plugins::test_support` and runs every fixture case through the plugin's
`execute_request()` function. The test asserts that the plugin's response
matches the expected contract shape:

- Valid requests produce a success response with `PluginOutput::Diff`.
- Invalid requests (missing/empty fields) produce a failure response with a
  `PluginDiagnostic` that includes the appropriate field name in the message
  and (where applicable) a `ReasonCode`.
- Wrong-operation requests produce a failure response with
  `ReasonCode::OperationNotSupported`.

These tests use `MockAdapter` (already available in both crates) to isolate
contract validation from adapter execution.

#### 2b: Rollback-guarantee unit tests

Add unit tests in each plugin crate that verify the rollback invariant:

- When `execute_request()` returns a failure response, no `PluginOutput::Diff`
  is emitted.
- When the adapter returns an error, the plugin emits a failure diagnostic
  without producing any filesystem-modifying output.
- When the adapter returns unchanged content, the plugin emits a failure
  diagnostic (not a no-op success).

These tests already exist partially (adapter-failure and unchanged-output
scenarios) but the rollback assertion ("no diff emitted on failure") should be
made explicit.

#### 2c: BDD coverage for refusal paths and edge cases

Add new BDD feature files for each plugin:

`crates/weaver-plugin-rope/tests/features/rename_symbol_contract.feature` with
scenarios:

- Rename-symbol request with missing `uri` fails with `incomplete_payload`.
- Rename-symbol request with missing `new_name` fails with
  `incomplete_payload`.
- Rename-symbol request with empty `new_name` fails with
  `incomplete_payload`.
- Shared contract fixture cases pass validation identically.
- Rollback: adapter failure produces no diff output.

`crates/weaver-plugin-rust-analyzer/tests/features/rename_symbol_contract.feature`
 with scenarios mirroring the rope feature file above to demonstrate
cross-plugin parity.

Step definitions for the new feature files live in new behaviour modules
(`src/tests/contract_behaviour.rs` or similar) to stay within the 400-line
budget.

Validation: `cargo test -p weaver-plugin-rope` and
`cargo test -p weaver-plugin-rust-analyzer` both pass. `make lint` passes.

### Milestone 3: Daemon-level coverage for routing and rollback

Extend the `weaverd` refactor test suite to cover gaps not addressed by 5.2.4's
per-milestone tests.

#### 3a: Resolution-layer coverage

Add unit tests in a new module
`crates/weaverd/src/dispatch/act/refactor/resolution_coverage.rs` (or extend
`resolution_tests.rs` if it has room) covering:

- Automatic selection with multiple candidates for the same language
  (deterministic ordering by `provider_rank()`).
- Explicit `--provider` for a provider that exists but lacks `rename-symbol`
  capability: refused with `ProviderLacksCapability`.
- Explicit `--provider` for a non-existent provider: refused with
  `ProviderNotFound`.
- Resolution envelope JSON shape: assert exact field names and values for a
  success resolution and a refusal resolution.

#### 3b: Handler-level rollback guarantee tests

Add unit tests that verify the handler's rollback invariant at the `weaverd`
level:

- When resolution refuses, the handler returns status 1 and does not invoke
  the plugin runtime.
- When the plugin returns a failure response, the handler returns status 1
  and does not invoke the apply-patch path.
- When the plugin returns a malformed diff, the handler returns status 1 and
  does not write to the filesystem.

These assertions complement the existing `behaviour.rs` scenarios but make the
"no side-effect on failure" invariant explicit and unit-testable.

#### 3c: BDD coverage for routing edge cases

Add a new feature file
`crates/weaverd/tests/features/rename_symbol_coverage.feature` with scenarios:

- Provider lacks capability: refused deterministically.
- Non-existent provider: refused deterministically.
- Successful Python rename leaves filesystem unchanged on adapter failure.
- Successful Rust rename leaves filesystem unchanged on adapter failure.
- Automatic routing emits structured `CapabilityResolution` with correct
  `selection_mode` and `candidates` array.

Step definitions for the new feature file live in a new behaviour module (e.g.
`resolution_behaviour.rs` or by extending `behaviour.rs` if within budget).

Validation: `cargo test -p weaverd dispatch::act::refactor` passes. `make lint`
passes.

### Milestone 4: End-to-end coverage in `weaver-e2e`

Extend the `weaver-e2e` test suite to validate the full CLI-to-daemon pipeline
for capability-routed rename flows.

#### 4a: Automatic-routing e2e tests

Add a new test file
`crates/weaver-e2e/tests/refactor_capability_routing_snapshots.rs` containing:

- Python automatic routing: CLI invocation without `--provider` for a `.py`
  file. Assert that the daemon request includes the correct command shape and
  that the response includes a `CapabilityResolution` envelope.
- Rust automatic routing: CLI invocation without `--provider` for a `.rs`
  file. Assert the same.

These tests use the existing `FakeDaemon` pattern from the sibling snapshot
test files. The `FakeDaemon` response logic is extended (or a new response
handler added) to emit a `CapabilityResolution` envelope in the stream before
the exit message.

#### 4b: Refusal e2e tests

Add tests in the same file covering:

- Unsupported language (e.g. `.java` file): CLI exits with status 1 and
  stderr contains `unsupported_language`.
- Explicit provider mismatch (e.g. `--provider rope` for `.rs` file): CLI
  exits with status 1 and stderr contains `explicit_provider_mismatch`.

These tests prove that refusal diagnostics survive the full CLI→daemon→CLI
rendering pipeline.

#### 4c: Rollback e2e tests

Add a test that creates a temporary file, runs a refactor command that is
expected to fail (e.g. unsupported language), and asserts that the temporary
file content is unchanged after the command exits.

Validation: `cargo test -p weaver-e2e` passes. Snapshot files are committed.
`make lint` passes.

### Milestone 5: Documentation, roadmap, and final validation

#### 5a: Review and update `docs/users-guide.md`

Review the users guide for any changes to observable behaviour surfaced during
testing. If new refusal reasons or edge-case behaviours were discovered, add
them to the relevant sections. If no behaviour changes are needed, record in
the Decision Log that the users guide was reviewed and found current.

#### 5b: Mark roadmap 5.2.5 as done

In `docs/roadmap.md`, change the `- [ ]` checkbox on line 590 to `- [x]` for
item 5.2.5.

#### 5c: Final validation

Run all workspace gates:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/5-2-5-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/5-2-5-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/5-2-5-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/5-2-5-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/5-2-5-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/5-2-5-test.log
```

All six commands must exit zero.

## Validation and acceptance

Quality criteria (what "done" means):

- Tests: `make test` passes with no regressions. The new shared contract
  fixture cases pass identically for both plugins. New BDD scenarios pass. New
  e2e snapshot tests pass.
- Lint/typecheck: `make check-fmt` and `make lint` both pass.
- Documentation: `make markdownlint` and `make nixie` pass.
  `docs/users-guide.md` is reviewed and current. `docs/roadmap.md` marks 5.2.5
  as done.
- Rollback guarantees: every refusal and failure path is asserted to produce
  no filesystem-modifying output (no `PluginOutput::Diff` on failure, no
  apply-patch invocation on refusal).
- Cross-plugin parity: both plugins pass the same shared contract fixture
  cases with identical pass/fail verdicts.

Quality method (how we check):

- Run `make check-fmt && make lint && make test` and verify exit code 0.
- Run `make fmt && make markdownlint && make nixie` and verify exit code 0.
- Inspect the test output for the shared contract fixture test names and
  confirm they appear for both `weaver-plugin-rope` and
  `weaver-plugin-rust-analyzer`.
- Inspect the e2e snapshot files to confirm they include
  `CapabilityResolution` envelope data.

Expected targeted evidence while iterating:

```sh
cargo test -p weaver-plugins --all-features -- test_support
cargo test -p weaver-plugin-rope contract
cargo test -p weaver-plugin-rust-analyzer contract
cargo test -p weaverd dispatch::act::refactor -- --nocapture
cargo test -p weaver-e2e refactor_capability
```

## Idempotence and recovery

All steps are re-runnable. `make test` is idempotent. Snapshot tests can be
updated with `cargo insta review` if the expected output changes. If a step
fails, fix the issue and re-run from the failing command. No destructive or
irreversible operations are involved.

## Artifacts and notes

Key file paths created or modified by this plan:

New files:

- `crates/weaver-plugins/src/test_support.rs`
- `crates/weaver-plugin-rope/src/tests/contract_conformance.rs`
- `crates/weaver-plugin-rope/tests/features/rename_symbol_contract.feature`
- `crates/weaver-plugin-rope/src/tests/contract_behaviour.rs`
- `crates/weaver-plugin-rust-analyzer/src/tests/contract_conformance.rs`
- `crates/weaver-plugin-rust-analyzer/tests/features/rename_symbol_contract.feature`
- `crates/weaver-plugin-rust-analyzer/src/tests/contract_behaviour.rs`
- `crates/weaverd/src/dispatch/act/refactor/resolution_coverage.rs`
- `crates/weaverd/tests/features/rename_symbol_coverage.feature`
- `crates/weaverd/src/dispatch/act/refactor/resolution_behaviour.rs`
  (or extend `behaviour.rs`)
- `crates/weaver-e2e/tests/refactor_capability_routing_snapshots.rs`

Modified files:

- `crates/weaver-plugins/Cargo.toml` (add `test-support` feature)
- `crates/weaver-plugins/src/lib.rs` (add `test_support` module)
- `crates/weaver-plugins/src/capability/tests.rs` (or new sibling)
- `crates/weaver-plugin-rope/Cargo.toml` (add `test-support` dev-dep)
- `crates/weaver-plugin-rope/src/tests/mod.rs` (register new modules)
- `crates/weaver-plugin-rust-analyzer/Cargo.toml` (add `test-support` dev-dep)
- `crates/weaver-plugin-rust-analyzer/src/tests/mod.rs` (register new modules)
- `crates/weaverd/src/dispatch/act/refactor/mod.rs` (register new test
  modules)
- `docs/users-guide.md` (review; update only if needed)
- `docs/roadmap.md` (mark 5.2.5 as done)

## Interfaces and dependencies

### Shared test support module

In `crates/weaver-plugins/src/test_support.rs`, define:

```rust
//! Shared test fixtures for the `rename-symbol` capability contract.
//!
//! These fixtures are feature-gated behind `test-support` and
//! intended for use by plugin crates that need to prove conformance
//! to the shared contract.

use crate::capability::rename_symbol::RenameSymbolContract;
use crate::capability::CapabilityContract;
use crate::protocol::{PluginRequest, PluginResponse};

/// Returns a `PluginRequest` with all required `rename-symbol` fields.
pub fn valid_rename_request() -> PluginRequest { /* ... */ }

/// Returns a success `PluginResponse` containing a `Diff` output.
pub fn valid_rename_response() -> PluginResponse { /* ... */ }

/// Returns a failure `PluginResponse` with the given reason code.
pub fn failure_response_with_reason(
    reason: crate::capability::reason_code::ReasonCode,
) -> PluginResponse { /* ... */ }

/// Describes a single contract fixture case.
pub struct ContractFixtureCase {
    pub description: &'static str,
    pub request: PluginRequest,
    pub expected_valid: bool,
}

/// Returns the canonical set of contract fixture cases.
pub fn contract_fixture_cases() -> Vec<ContractFixtureCase> { /* ... */ }
```

### Plugin contract conformance test pattern

Each plugin crate's `src/tests/contract_conformance.rs` follows this pattern:

```rust
//! Shared contract conformance tests for the `rename-symbol` capability.

use weaver_plugins::test_support::{
    contract_fixture_cases, valid_rename_request,
};

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::valid_request(0, true)]
    #[case::missing_uri(1, false)]
    // ... one case per fixture
    fn contract_fixture_conformance(
        #[case] fixture_index: usize,
        #[case] expected_valid: bool,
    ) {
        let cases = contract_fixture_cases();
        let case = &cases[fixture_index];
        // ... validate via plugin's execute_request()
    }
}
```

### BDD feature file pattern

Each new `.feature` file follows the established Gherkin conventions:

```gherkin
Feature: Rename-symbol contract conformance

  Scenario: Valid rename request passes contract validation
    Given a valid rename-symbol request from the shared fixtures
    When the contract validates the request
    Then validation succeeds

  Scenario: Missing uri is refused with incomplete payload
    Given a rename-symbol request missing uri from the shared fixtures
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And the failure reason code is "incomplete_payload"
    And no diff output is emitted

  Scenario: Rollback on adapter failure
    Given a rename-symbol request with required arguments
    And an adapter that fails with an error
    When the plugin executes the request
    Then the plugin returns failure diagnostics
    And no diff output is emitted
```
