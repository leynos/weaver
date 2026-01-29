# Deliver act apply-patch sub-command

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: DRAFT

PLANS.md is not present in the repository, so no additional plan governance
applies beyond AGENTS.md and this ExecPlan.

## Purpose / big picture

Implement the `weaver act apply-patch` command end-to-end so an operator can
stream a patch via STDIN, have the daemon validate it through the Double-Lock
harness, and see a deterministic success or structured failure. The observable
success is: piping a patch into `weaver act apply-patch` results in a JSONL
request containing the patch, the daemon applies it atomically, and the CLI
exits with status 0 on success or non-zero with structured error output on
failure, without partial filesystem writes.

## Constraints

- Follow the `act apply-patch` semantics in `docs/weaver-design.md` section
  4.3, including SEARCH/REPLACE behaviour, line-ending handling, and
  traversal checks.
- The JSONL protocol must remain backward-compatible for existing commands.
- All `act` edits must pass the Double-Lock harness with no on-disk writes on
  syntactic or semantic lock failure.
- Patch application must be atomic per command (all-or-nothing).
- Tests must include unit coverage and rstest-bdd scenarios using
  `rstest-bdd` v0.4.0 and `rstest` fixtures.
- Every new Rust module starts with a `//!` module doc comment and stays under
  400 lines; extract helpers where needed per
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- Documentation updates are required in `docs/weaver-design.md`,
  `docs/users-guide.md`, and the Phase 2 roadmap entry in `docs/roadmap.md`.
- Run `make check-fmt`, `make lint`, and `make test` and ensure they pass.
- Use en-GB-oxendict spelling in prose and comments.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 20 files or exceeds
  ~1200 net lines, stop and escalate.
- Interface: if the JSONL schema must break existing clients, stop and
  escalate with options.
- Dependencies: if a new external dependency is required, stop and escalate.
- Iterations: if tests still fail after two focused fix attempts, stop and
  escalate with findings.
- Ambiguity: if multiple valid patch semantics emerge that materially affect
  behaviour, stop and ask for a decision.

## Risks

- Risk: the daemon request size cap (64 KiB) rejects realistic patch payloads.
  Severity: medium. Likelihood: medium.
  Mitigation: decide on and document a higher limit or a streaming strategy
  before implementing the request path.
- Risk: semantic lock integration requires additional LSP wiring beyond the
  placeholder lock.
  Severity: high. Likelihood: medium.
  Mitigation: implement a dedicated LSP-backed `SemanticLock` adapter and
  isolate it behind a trait for test doubles.
- Risk: fuzzy match behaviour could misapply if not cursor-scoped.
  Severity: high. Likelihood: medium.
  Mitigation: follow the cursor-based algorithm from the design doc and test
  no-match failure paths.
- Risk: CLI stdin handling could block non apply-patch commands.
  Severity: medium. Likelihood: low.
  Mitigation: only read STDIN when `domain=act` and `operation=apply-patch`.

## Progress

- [x] (2026-01-28 00:00Z) Drafted ExecPlan.
- [ ] Add JSONL request/response types for apply-patch and update CLI stdin
      handling.
- [ ] Implement patch parsing, matching, and safety harness integration in
      `weaverd`.
- [ ] Add unit and BDD tests (rstest + rstest-bdd) for happy/unhappy paths.
- [ ] Update design doc, user guide, and roadmap; run quality gates.

## Surprises & discoveries

- Observation: Qdrant notes store was unreachable during planning.
  Evidence: `qdrant-find` returned connection failures.
  Impact: no historical project notes were available for this plan.

## Decision log

- Decision: Extend the existing JSONL `CommandRequest` with an optional
  `patch` payload field (only populated for `act apply-patch`) to preserve
  backward compatibility.
  Rationale: avoids a breaking protocol change while still carrying raw patch
  content in a single JSONL line.
  Date/Author: 2026-01-28 / Codex
- Decision: Keep request/response structs in `weaver-cli` and `weaverd` for
  this step instead of introducing a new shared crate.
  Rationale: limits scope creep; revisit if protocol drift becomes painful.
  Date/Author: 2026-01-28 / Codex

## Outcomes & retrospective

Pending execution. Populate after implementation with outcomes, gaps, and
lessons learned.

## Context and orientation

Key references and existing code:

- `docs/weaver-design.md` section 4.3 defines patch format, search/replace
  semantics, and lock interaction.
- `crates/weaver-cli/src/command.rs` and `crates/weaver-cli/src/lib.rs`
  define JSONL request construction and CLI execution flow.
- `crates/weaverd/src/dispatch/request.rs` defines daemon JSONL request
  parsing; `crates/weaverd/src/dispatch/handler.rs` enforces request size.
- `crates/weaverd/src/safety_harness` provides edit transactions and lock
  interfaces; only placeholder semantic locks exist today.
- BDD tests live in `crates/weaver-cli/tests/features/` and
  `crates/weaverd/tests/features/`, with step definitions under
  `crates/weaver-cli/src/tests/` and `crates/weaverd/src/tests/`.
- Testing guidance: `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rstest-bdd-users-guide.md`, and
  `docs/reliable-testing-in-rust-via-dependency-injection.md`.

## Plan of work

Stage A (confirm spec and constraints): re-read the `act apply-patch` section
in `docs/weaver-design.md`, confirm the accepted patch grammar, and decide how
to handle the request size limit. Capture any clarifications in the design
doc's decision log for section 4.3.

Stage B (CLI and JSONL protocol): extend `CommandRequest` in
`crates/weaver-cli/src/command.rs` to carry an optional patch payload and
update `execute_daemon_command` to read STDIN only for
`act apply-patch`. Update the test harness to inject stdin (likely by adding a
reader to `IoStreams` or passing a reader into `CliRunner`) so the BDD tests
can assert the JSONL request contains the expected patch content. Add or update
golden fixtures under `crates/weaver-cli/tests/golden/`.

Stage C (daemon request parsing and handler wiring): extend
`crates/weaverd/src/dispatch/request.rs` to deserialize the optional patch
field and validate it for `act apply-patch` (non-empty, text-only). Add a new
`dispatch::act` module with an `apply_patch` handler, and route it from
`DomainRouter::route_act`. Ensure the handler starts required backends and
uses the safety harness.

Stage D (patch parsing and matching): implement a dedicated patch parser in
`crates/weaverd/src/dispatch/act/apply_patch/` (split into modules as needed)
that supports modify/create/delete operations, cursor-based SEARCH/REPLACE
matching, fuzzy whitespace/line-ending matching, and path traversal checks.
Unit-test parsing and matching, including invalid headers, missing hunks, and
binary or NUL bytes rejection.

Stage E (Double-Lock integration): map parsed operations into a transaction
that produces modified buffers for syntactic and semantic locks and performs
atomic commits. Extend the safety harness if necessary to support delete
operations and full-content replacements. Implement an LSP-backed semantic
lock adapter that uses `did_open`, `did_change`, `did_close`, and
`diagnostics` to compare baseline vs modified diagnostics, returning
`SafetyHarnessError::SemanticBackendUnavailable` when the LSP backend cannot
be started.

Stage F (tests): add unit tests for patch parsing, path validation, line
ending preservation, and transaction mapping. Add rstest-bdd scenarios for
happy path (patch applies and commits) and unhappy paths (no match, invalid
header, traversal attempt, syntactic lock failure, semantic lock failure).

Stage G (docs and roadmap): update `docs/weaver-design.md` with decisions
made (request schema, error envelope, request size limit), update
`docs/users-guide.md` with CLI usage and behaviour changes, and mark the
apply-patch entry as done in `docs/roadmap.md`.

Stage H (quality gates): run `make check-fmt`, `make lint`, `make test`,
`make markdownlint`, `make fmt`, and `make nixie` as required, using `tee`
and `set -o pipefail` to preserve exit codes.

## Concrete steps

1. Reconfirm requirements and decide on request size handling, then update
   `docs/weaver-design.md` with the decision before coding.
2. Implement CLI JSONL request changes and tests, then run the CLI-focused
   unit and BDD suites.
3. Implement daemon apply-patch handler, parser, and safety harness changes,
   then run unit and BDD suites for `weaverd`.
4. Update documentation and roadmap, then run the full workspace gates.

Commands (run from repo root, using `tee` + `pipefail`):

    set -o pipefail && make check-fmt 2>&1 | tee /tmp/make-check-fmt.log
    set -o pipefail && make lint 2>&1 | tee /tmp/make-lint.log
    set -o pipefail && make test 2>&1 | tee /tmp/make-test.log
    set -o pipefail && make markdownlint 2>&1 | tee /tmp/make-markdownlint.log
    set -o pipefail && make fmt 2>&1 | tee /tmp/make-fmt.log
    set -o pipefail && make nixie 2>&1 | tee /tmp/make-nixie.log

## Validation and acceptance

The feature is complete when:

- Running `weaver act apply-patch` with a patch on STDIN sends a JSONL request
  that includes the patch payload and exits with status 0 on success.
- Daemon responses include structured JSON error payloads for parse failures
  and verification failures, and the CLI surfaces them with non-zero status.
- The patch parser enforces modify/create/delete semantics, rejects missing
  hunks and binary content, and normalises line endings per the design doc.
- Syntactic and semantic locks run on modified/new files; failures leave the
  filesystem untouched.
- Unit tests and rstest-bdd scenarios cover happy and unhappy paths.
- `docs/weaver-design.md` and `docs/users-guide.md` reflect the new command
  behaviour, and `docs/roadmap.md` marks the entry as done.
- `make check-fmt`, `make lint`, `make test`, `make markdownlint`, `make fmt`,
  and `make nixie` succeed.

## Idempotence and recovery

All steps are re-runnable. If apply-patch parsing or lock validation fails,
the transaction must leave the filesystem unchanged, so re-running with a
corrected patch is safe. If documentation formatting fails, run `make fmt`
and `make markdownlint` again before re-running the Rust checks.

## Artifacts and notes

Expected artifacts include:

- New BDD feature files under `crates/weaver-cli/tests/features/` and
  `crates/weaverd/tests/features/`.
- New or updated golden JSONL request fixtures under
  `crates/weaver-cli/tests/golden/`.
- A new apply-patch handler module under `crates/weaverd/src/dispatch/act/`.
- Updated docs in `docs/weaver-design.md`, `docs/users-guide.md`, and
  `docs/roadmap.md`.

## Interfaces and dependencies

Define or adjust the following interfaces (names are suggestions; keep them
consistent and small, splitting modules as needed):

- In `crates/weaver-cli/src/command.rs`:
  - `CommandRequest` gains `patch: Option<String>`.
  - A helper like `CommandRequest::with_patch(...)` to build apply-patch
    requests.
- In `crates/weaverd/src/dispatch/request.rs`:
  - `CommandRequest` mirrors the optional `patch` field and includes a
    `patch()` accessor that returns `Option<&str>`.
- In `crates/weaverd/src/dispatch/act/apply_patch/`:
  - `PatchParseError` and `PatchOperation` enums (Modify/Create/Delete).
  - `ParsedPatch` holding ordered operations.
  - `PatchMatcher` functions that apply SEARCH/REPLACE blocks with cursor
    logic and line-ending normalisation.
  - `ApplyPatchHandler::handle(request, writer, backends)` that starts
    backends, applies the patch in-memory, and commits via the harness.
- In `crates/weaverd/src/safety_harness`:
  - Extend `EditTransaction` (or introduce a `FileChange` enum) to support
    deletes and full-content replacements without losing atomicity.
  - Add an LSP-backed `SemanticLock` adapter that uses
    `SemanticBackendProvider::with_lsp_host_mut` and LSP `did_*` notifications.

Where possible, use dependency injection for locks in tests by reusing
`ConfigurableSyntacticLock` and `ConfigurableSemanticLock`.

## Revision note

Initial draft created on 2026-01-28. No revisions yet.
