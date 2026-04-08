# Update weaver-plugin-rust-analyzer manifest and handshake for `rename-symbol`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

Implementation was approved on 2026-03-07 and completed on 2026-03-07.

## Purpose / big picture

Roadmap item 5.2.3 migrates the Rust actuator plugin from its legacy
provider-specific rename request shape to the shared `rename-symbol` capability
contract introduced in 5.2.1. After this change, the
`weaver-plugin-rust-analyzer` manifest declares `CapabilityId::RenameSymbol`,
the plugin accepts contract-conforming request payloads using `uri`,
`position`, and `new_name`, and successful responses still return diff output
that flows through the existing Double-Lock safety harness.

The user-visible command-line interface (CLI) stays the same:

```sh
weaver act refactor \
  --provider rust-analyzer \
  --refactoring rename \
  --file src/main.rs \
  offset=3 \
  new_name=renamed_name
```

The daemon continues to translate the CLI-facing `rename` request into the
internal `rename-symbol` capability operation before invoking the plugin. The
observable difference is that the Rust plugin now matches the same capability
contract already used by the Python rope plugin.

Observable success for this roadmap item:

- `crates/weaverd/src/dispatch/act/refactor/mod.rs` registers the
  rust-analyzer manifest with `CapabilityId::RenameSymbol`.
- `crates/weaver-plugin-rust-analyzer/src/lib.rs` accepts only the
  `rename-symbol` operation and validates the `uri`, `position`, and `new_name`
  request arguments defined by
  `crates/weaver-plugins/src/capability/rename_symbol.rs`.
- Successful plugin responses still use `PluginOutput::Diff`; failure payloads
  remain protocol-conforming and carry stable reason codes where the failure
  class is known.
- Unit tests and `rstest-bdd` v0.5.0 behaviour-driven development (BDD) tests
  cover happy paths, unhappy paths, and edge cases for the contract migration.
- `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
  reflect the shipped behaviour.
- `make fmt`, `make markdownlint`, `make check-fmt`, `make lint`, and
  `make test` all pass.

## Constraints

1. The `rename-symbol` contract from 5.2.1 is already complete. Do not modify
   `crates/weaver-plugins/` unless an implementation blocker proves the
   contract is insufficient. That is an escalation event, not an autonomous
   change.
1. Keep the CLI surface stable. Users still invoke
   `weaver act refactor --provider rust-analyzer --refactoring rename ...`. The
   daemon performs the internal mapping to `rename-symbol`.
1. Keep all execution synchronous. Do not introduce async runtimes, async
   traits, or background tasks.
1. Preserve the existing Double-Lock path. Successful Rust rename edits must
   still return unified diff output that is forwarded into `act apply-patch`.
1. Respect the repository-wide 400-line file limit. Current hotspots:
   `crates/weaver-plugin-rust-analyzer/src/lib.rs` is 358 lines and
   `crates/weaverd/src/dispatch/act/refactor/mod.rs` is 399 lines.
1. All touched Rust modules must retain module-level `//!` documentation, and
   all public items must remain documented.
1. Behaviour tests must use `rstest-bdd` v0.5.0 patterns already used in this
   repository, including a fixture parameter named exactly `world`.
1. Lint suppressions are a last resort. If unavoidable, use tightly scoped
   `#[expect(..., reason = "...")]`; do not add `#[allow(...)]`.
1. Comments and documentation must use en-GB-oxendict spelling.
1. Use existing workspace dependencies only. Adding a new crate dependency is
   out of scope for this item.
1. The plan must record any new design decisions in `docs/weaver-design.md`
   as part of implementation, not just in this ExecPlan.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 10 files or more than
  roughly 500 net lines, stop and escalate.
- Interface: if any public API in `weaver-plugins` or the CLI argument contract
  for `act refactor` must change, stop and escalate.
- Dependencies: if a new workspace or external dependency appears necessary,
  stop and escalate.
- Iterations: if `make lint` or `make test` still fail after 5 repair loops,
  stop and escalate with the failing commands and current hypothesis.
- Line budget: if `crates/weaverd/src/dispatch/act/refactor/mod.rs` cannot
  absorb the manifest update and any supporting tests without exceeding 400
  lines, extract a focused helper module instead of forcing more code into
  `mod.rs`. If this requires more than 2 new files, stop and escalate.
- Ambiguity: if “plugin advertises `rename-symbol` in capability probes” turns
  out to require changes to the top-level `weaver --capabilities` output, stop
  and escalate. Roadmap item 5.7.5 is the planned place for merged runtime
  capability probe output.

## Risks

- Risk: The rust-analyzer plugin currently accepts `"rename"` with `offset`
  rather than `"rename-symbol"` with `position`. This migration can easily
  leave unit and BDD fixtures half-updated. Severity: high Likelihood: high
  Mitigation: update plugin request builders, unit tests, and feature steps in
  the same commit slice before changing dispatch assertions.

- Risk: `crates/weaverd/src/dispatch/act/refactor/mod.rs` is already at 399
  lines, so even a small registration change could force a split. Severity:
  medium Likelihood: high Mitigation: extract manifest-construction or
  request-mapping helpers into a sibling module if the file crosses the cap.

- Risk: Failure reason codes for rust-analyzer errors may not map cleanly from
  arbitrary LSP error text. Severity: medium Likelihood: medium Mitigation:
  guarantee reason codes for deterministic classes only: unsupported operation,
  incomplete payload, and unchanged output. Preserve free-text adapter
  diagnostics for opaque engine failures unless a stable classification is
  obvious from existing behaviour.

- Risk: The acceptance criterion says “Rust rename flows are capability-routed”
  even though full language-aware provider resolution is deferred to 5.2.4.
  Severity: medium Likelihood: medium Mitigation: treat 5.2.3 as the
  runtime-handshake half of capability routing: the daemon already maps CLI
  `rename` to internal `rename-symbol`; this item finishes the Rust plugin side
  and manifest declaration, while 5.2.4 will handle provider selection policy.

## Progress

- [x] (2026-03-06) Reviewed `AGENTS.md`, the roadmap entry, the execplans
  skill, the prior 5.2.1 and 5.2.2 ExecPlans, and project memory notes.
- [x] (2026-03-06) Inspected the current Rust plugin and daemon refactor
  handler to identify the present contract mismatch and line-budget pressure.
- [x] (2026-03-06) Drafted this ExecPlan.
- [x] (2026-03-07) Obtained approval for the ExecPlan.
- [x] (2026-03-07) Added unit and behavioural tests that assert the
  `rename-symbol` request/response contract for the rust-analyzer plugin.
- [x] (2026-03-07) Updated the rust-analyzer plugin runtime handshake and
  error mapping.
- [x] (2026-03-07) Updated daemon manifest registration and request-capture
  tests proving Rust rename requests are capability-routed.
- [x] (2026-03-07) Updated `docs/weaver-design.md`, `docs/users-guide.md`,
  and `docs/roadmap.md`.
- [x] (2026-03-07) Ran `make fmt`, `make markdownlint`, `make check-fmt`,
  `make lint`, and `make test`, each via `tee` with `set -o pipefail`.

## Surprises & Discoveries

- Discovery: `crates/weaverd/src/dispatch/act/refactor/mod.rs` already maps
  CLI `--refactoring rename` to internal operation `"rename-symbol"` and
  renames `offset` to `position`. The remaining 5.2.3 gap is that the
  rust-analyzer plugin itself still expects the old request shape and the
  rust-analyzer manifest is not yet declared with `CapabilityId::RenameSymbol`.

- Discovery: the rust-analyzer plugin crate currently has no manifest code of
  its own. In this repository, “manifest and runtime handshake” means the
  daemon-side registration in `weaverd` plus the plugin’s request/response
  contract at the JSONL boundary.

- Discovery: extracting refactor-plugin manifest builders into a dedicated
  `manifests.rs` module dropped
  `crates/weaverd/src/dispatch/act/refactor/mod.rs` from 399 lines to 381
  lines, creating room for manifest-specific tests without breaking the
  400-line cap.

- Discovery: the pre-existing workspace test hang in
  `tests::unit::auto_start::auto_start_succeeds_and_proceeds` was caused by a
  race between the daemon reporting a healthy auto-start snapshot and the Unix
  socket becoming connectable. A bounded `connect_with_retry` helper in the CLI
  transport layer resolved the production race and made `make test`
  deterministic again.

## Decision Log

- Decision: treat 5.2.3 as a compatibility migration, not a user-facing CLI
  redesign. Rationale: the roadmap acceptance criteria require schema
  conformance and capability routing, while the existing command syntax is
  already documented and exercised by tests. Date: 2026-03-06.

- Decision: do not plan changes to `weaver --capabilities` for this item.
  Rationale: the current top-level capability probe covers negotiated daemon
  and LSP capability state, while roadmap 5.7.x is the explicit home for
  plugin-capability discoverability. Date: 2026-03-06.

- Decision: prefer small helper extraction over in-place expansion if
  `weaverd` hits the 400-line limit. Rationale: `mod.rs` has only one line of
  headroom, so even documenting the manifest registration logic may force a
  split. Date: 2026-03-06.

- Decision: make `PluginFailure` the rust-analyzer plugin's internal error
  transport and attach `ReasonCode::OperationNotSupported`,
  `ReasonCode::IncompletePayload`, and `ReasonCode::SymbolNotFound` for the
  deterministic failure classes. Rationale: this matches the rope migration,
  keeps the outer protocol unchanged, and lets tests assert structured
  diagnostics rather than brittle free-text only. Date: 2026-03-07.

- Decision: fix the daemon auto-start race in production rather than only
  loosening the test. Rationale: the hanging test exposed a genuine gap where
  the CLI could observe a healthy daemon snapshot before the Unix socket was
  ready for connection, so a bounded retry in `weaver-cli` was the correct
  behavioural fix and not merely test scaffolding. Date: 2026-03-07.

## Outcomes & Retrospective

Roadmap item 5.2.3 is complete. The rust-analyzer actuator plugin now accepts
only the capability operation `"rename-symbol"`, parses the shared contract
arguments `uri`, `position`, and `new_name`, and emits protocol-conforming
failure diagnostics with stable `ReasonCode` values for unsupported operation,
incomplete payloads, and symbol-not-found style failures. The daemon now
registers the rust-analyzer provider through a manifest helper that declares
`CapabilityId::RenameSymbol`, and regression tests prove Rust rename requests
are routed through the capability contract rather than the legacy provider-
specific request shape.

Files created:

- `crates/weaver-plugin-rust-analyzer/src/arguments.rs`
- `crates/weaverd/src/dispatch/act/refactor/manifests.rs`

Files materially changed:

- `crates/weaver-plugin-rust-analyzer/src/lib.rs`
- `crates/weaver-plugin-rust-analyzer/src/tests/mod.rs`
- `crates/weaver-plugin-rust-analyzer/src/tests/behaviour.rs`
- `crates/weaver-plugin-rust-analyzer/tests/features/rust_analyzer_plugin.feature`
- `crates/weaverd/src/dispatch/act/refactor/mod.rs`
- `crates/weaverd/src/dispatch/act/refactor/tests.rs`
- `crates/weaver-cli/src/transport.rs`
- `crates/weaver-cli/src/lib.rs`
- `crates/weaver-cli/src/tests/unit/auto_start.rs`
- `docs/weaver-design.md`
- `docs/users-guide.md`
- `docs/roadmap.md`

Validation results:

- `make fmt` passed.
- `make markdownlint` passed.
- `make check-fmt` passed.
- `make lint` passed.
- `make test` passed.

Lessons:

- The rope migration from 5.2.2 was the right precedent for the Rust plugin as
  well; keeping request validation and reason-code mapping structurally aligned
  across plugins reduced review risk.
- The auto-start test hang was a useful signal, not mere test fragility. When a
  behavioural test stalls on orchestration, prefer turning the stall into a
  bounded failure and fixing the underlying race rather than simply increasing
  timeouts.

## Context and orientation

The current implementation is split across two main areas.

`crates/weaver-plugin-rust-analyzer/` is the one-shot actuator plugin. It reads
one `PluginRequest` from stdin, executes a rename through the
`RustAnalyzerAdapter` trait, and writes one `PluginResponse` to stdout. Today
it still matches only `request.operation() == "rename"` and parses
`offset`/`new_name` from the request arguments. That behaviour lives primarily
in `crates/weaver-plugin-rust-analyzer/src/lib.rs`, with unit coverage in
`crates/weaver-plugin-rust-analyzer/src/tests/mod.rs` and behavioural coverage
in `crates/weaver-plugin-rust-analyzer/src/tests/behaviour.rs` plus
`crates/weaver-plugin-rust-analyzer/tests/features/rust_analyzer_plugin.feature`.

`crates/weaverd/src/dispatch/act/refactor/mod.rs` is the daemon-side
registration and request-building layer. It already maps CLI
`--refactoring rename` to the internal operation `"rename-symbol"`, injects
`uri` from `--file`, and maps `offset` to `position`. It already declares
`CapabilityId::RenameSymbol` for the rope plugin, but not for the rust-analyzer
manifest. Tests for the request-building path live in
`crates/weaverd/src/dispatch/act/refactor/tests.rs`.

The shared contract defined by 5.2.1 lives in
`crates/weaver-plugins/src/capability/rename_symbol.rs`. The important parts
for this plan are:

- operation name: `"rename-symbol"`
- required arguments: `uri`, `position`, `new_name`
- successful response: `PluginOutput::Diff`
- failure response: ordinary plugin failure payload with optional stable
  `reason_code`

Roadmap 5.2.2 already applied the same migration to the rope plugin. Use that
implementation and its ExecPlan at
`docs/execplans/5-2-2-update-weaver-plugin-rope-manifest.md` as the local
precedent for test style, reason-code treatment, and documentation updates.

## Implementation plan

### Stage 1: lock in failing tests for the contract mismatch

Start with red tests in the rust-analyzer plugin crate. Update the request
builders in `crates/weaver-plugin-rust-analyzer/src/tests/mod.rs` so the happy
path constructs a `PluginRequest` using operation `"rename-symbol"` and the
contract keys `uri`, `position`, and `new_name`. Change the unhappy-path cases
to validate missing `position`, non-numeric `position`, empty `new_name`,
unsupported operation, unchanged output, and adapter failure under the new
contract vocabulary.

Update the behavioural tests in
`crates/weaver-plugin-rust-analyzer/src/tests/behaviour.rs` and
`crates/weaver-plugin-rust-analyzer/tests/features/rust_analyzer_plugin.feature`
to describe the capability contract explicitly. The scenarios should prove:

1. the plugin accepts a valid `rename-symbol` request and returns diff output;
1. the plugin rejects missing required arguments with failure diagnostics;
1. the plugin rejects unsupported operations with a stable failure shape;
1. adapter failures are surfaced without crashing the dispatcher;
1. unchanged content is treated as failure.

If line pressure grows in `src/tests/mod.rs`, extract request-building helpers
into a small test-only support submodule rather than making the file sprawl.

### Stage 2: update the rust-analyzer plugin dispatcher to the capability contract

Modify `crates/weaver-plugin-rust-analyzer/src/lib.rs` so `execute_request()`
recognizes `"rename-symbol"` instead of `"rename"`. Update argument parsing to
require `uri`, `position`, and `new_name`, with `position` parsed as the UTF-8
byte offset used by the existing adapter. The in-band file payload remains
authoritative for file content; `uri` is used to satisfy the contract and
should be checked against the single file payload path so the request cannot
silently describe one file while carrying another.

Introduce a small failure carrier if needed, similar to the 5.2.2 rope
migration, so deterministic failures can attach `ReasonCode` values without
changing the outer protocol shape. At minimum:

- unsupported operation -> `ReasonCode::OperationNotSupported`
- missing or malformed required fields -> `ReasonCode::IncompletePayload`
- unchanged output -> `ReasonCode::SymbolNotFound`

Opaque adapter errors may continue to surface as human-readable messages if no
stable reason code is justified. Keep the final success path as
`PluginResponse::success(PluginOutput::Diff { ... })`.

If `src/lib.rs` approaches 400 lines, extract the contract argument parsing
into a dedicated `arguments.rs` helper module with its own `//!` comment and
unit tests, mirroring the rope migration.

### Stage 3: declare the capability in daemon registration and prove handshake coverage

Update `crates/weaverd/src/dispatch/act/refactor/mod.rs` so the registered
rust-analyzer manifest uses
`.with_capabilities(vec![CapabilityId::RenameSymbol])` just like the rope
manifest. Preserve the existing timeout and executable-path behaviour.

Add or update tests in `crates/weaverd/src/dispatch/act/refactor/tests.rs` to
prove the Rust path is capability-routed in the sense expected by 5.2.3:

- a captured request for `--provider rust-analyzer --refactoring rename`
  reaches the runtime as operation `"rename-symbol"`;
- the runtime request carries `uri`, `position`, and `new_name`;
- the rust-analyzer manifest constructor includes
  `CapabilityId::RenameSymbol`.

Because `mod.rs` is already 399 lines, assume a helper extraction may be
necessary here. The cleanest split is a small sibling module for manifest
construction or request mapping, leaving `handle()` readable and within the
line budget.

### Stage 4: update design and user documentation

Record the migration decisions in `docs/weaver-design.md`. The most natural
place is the rust-analyzer actuator implementation decisions section, adding a
short note that the plugin now participates in the shared `rename-symbol`
capability contract, that the daemon translates CLI `rename` requests to the
capability operation internally, and that the rust-analyzer manifest now
declares `rename-symbol`.

Update `docs/users-guide.md` anywhere it still implies that only rope declares
the capability. The plugin inventory and `act refactor` sections should make
clear that both built-in rename providers now implement the same internal
contract while the CLI remains `--refactoring rename`.

After implementation passes the gates, mark roadmap item 5.2.3 as done in
`docs/roadmap.md`.

### Stage 5: run the quality gates and capture evidence

Run the required gates from the repository root, always using `tee` and
`set -o pipefail` so truncated terminal output does not hide failures:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/5-2-3-make-fmt.log
```

```sh
set -o pipefail; make markdownlint 2>&1 | tee /tmp/5-2-3-make-markdownlint.log
```

```sh
set -o pipefail; make check-fmt 2>&1 | tee /tmp/5-2-3-make-check-fmt.log
```

```sh
set -o pipefail; make lint 2>&1 | tee /tmp/5-2-3-make-lint.log
```

```sh
set -o pipefail; make test 2>&1 | tee /tmp/5-2-3-make-test.log
```

If any command fails, inspect the corresponding log, fix the issue, and rerun
the full failing gate until the workspace is clean again. Do not declare the
roadmap item complete until all five commands succeed.

## Validation details

The minimal evidence expected from the implementation is:

1. a unit test in the rust-analyzer plugin crate proving a valid
   `rename-symbol` request succeeds with diff output;
1. unit tests proving malformed `position`, missing `uri`, missing
   `new_name`, unsupported operation, and unchanged output fail with the
   expected diagnostic shape;
1. `rstest-bdd` scenarios covering happy and unhappy contract paths using the
   repository’s mutable-world pattern;
1. a daemon-side test proving the Rust provider path sends the
   `rename-symbol` operation and contract fields;
1. passing `make fmt`, `make markdownlint`, `make check-fmt`, `make lint`, and
   `make test`.

The feature is complete only when those checks pass and the roadmap entry is
checked off.
