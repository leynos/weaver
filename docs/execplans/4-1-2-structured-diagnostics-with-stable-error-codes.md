# 4.1.2 Structured diagnostics with stable `E_SEMPAI_*` error codes

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

This change stabilizes Sempai diagnostics as a contract, not just an internal
type. After implementation, parser and validator flows will emit the same
structured diagnostic schema with stable `E_SEMPAI_*` codes, and JSON snapshots
will lock the schema so future refactors cannot silently break downstream
consumers.

The same implementation pass will also keep top-level CLI discoverability
intact for bare invocation (`weaver` with no arguments): non-zero exit, short
help with `Usage:`, the three domains (`observe`, `act`, `verify`), and
exactly one pointer to `weaver --help`.

Observable outcomes:

1. Diagnostic JSON snapshots pass for parser and validator paths.
2. Behaviour and unit tests assert the bare-invocation help contract.
3. `make check-fmt`, `make lint`, and `make test` pass.
4. Roadmap entry `4.1.2` is marked done. If the discoverability entry is
   already done in the current tree (`2.2.1`), keep it as done and preserve
   regression coverage.

## Constraints

- `make check-fmt`, `make lint`, and `make test` must pass after all edits.
- No file may exceed 400 lines.
- All new diagnostics must preserve stable string codes with `E_SEMPAI_*`
  names as documented in `docs/sempai-query-language-design.md`.
- Diagnostic payloads must include code, message, primary span, and notes.
- Parser and validator diagnostics must share one canonical JSON shape.
- Existing bare-invocation behaviour in `weaver-cli` must not regress.
- BDD coverage must use `rstest-bdd` v0.5.0 and include happy and unhappy
  paths.
- Public `sempai` facade stability must be preserved: no breaking rename or
  shape changes to existing exported types without explicit escalation.
- Update `docs/users-guide.md` for any user-visible behaviour or payload
  contract change.
- Record design decisions in `docs/sempai-query-language-design.md`.

## Tolerances (exception triggers)

- Scope: if this work requires edits in more than 16 files, stop and escalate.
- Dependencies: if a new external crate is required, stop and escalate.
- Interfaces: if a public API signature in `sempai` must change incompatibly,
  stop and escalate.
- Iterations: if a failing gate (`check-fmt`, `lint`, or `test`) cannot be
  fixed within 5 focused attempts, stop and escalate with failure evidence.
- Ambiguity: if parser/validator diagnostic-path boundaries are unclear, stop
  and resolve before implementation.

## Risks

- Risk: parser and validator code paths are still scaffolded, so forcing a
  stable cross-path contract may tempt over-abstraction too early.
  Severity: medium
  Likelihood: medium
  Mitigation: introduce a focused diagnostic-report schema layer and thin
  adapters in each path; avoid speculative parser architecture.

- Risk: snapshot tests may become noisy or brittle if they include unstable
  fields.
  Severity: medium
  Likelihood: medium
  Mitigation: snapshot only the stable JSON contract fields and ordering.

- Risk: roadmap numbering mismatch for bare-invocation help (`5.1.1` in request
  versus `2.2.1` in current roadmap) could cause incorrect checkbox edits.
  Severity: low
  Likelihood: high
  Mitigation: treat this as the same acceptance contract, validate behaviour,
  and only edit the checkbox that exists in the checked-in roadmap.

- Risk: changing diagnostics could break current tests expecting
  `NOT_IMPLEMENTED`.
  Severity: medium
  Likelihood: high
  Mitigation: update tests in red/green order and preserve explicit coverage
  for remaining stub paths where applicable.

## Progress

- [x] (2026-03-03 23:xxZ) Collected context from roadmap, Sempai design, and
  existing ExecPlans.
- [x] (2026-03-03 23:xxZ) Confirmed current bare-invocation behaviour and tests
  already exist in `weaver-cli`.
- [x] (2026-03-03 23:xxZ) Drafted this ExecPlan.
- [ ] Implement diagnostic schema and path-specific constructors.
- [ ] Add parser/validator snapshot coverage and BDD scenarios.
- [ ] Re-validate bare-invocation acceptance criteria with unit and BDD tests.
- [ ] Update design and user documentation.
- [ ] Run full quality gates and update roadmap checkbox state.

## Surprises & discoveries

- Observation: the current roadmap already marks bare invocation as done under
  `2.2.1`, while this request references `5.1.1`.
  Evidence: `docs/roadmap.md` section `2.2`.
  Impact: implementation should preserve behaviour and tests instead of
  re-implementing the feature.

- Observation: diagnostics types already exist in `sempai_core`, but there is
  not yet a dedicated, snapshot-locked parser-versus-validator contract.
  Evidence: `crates/sempai-core/src/diagnostic.rs` and current tests.
  Impact: this task should formalize schema stability and path-aware emission.

## Decision log

- Decision: treat the request's bare-invocation item as a regression contract
  verification milestone within this plan, not a fresh feature build.
  Rationale: the capability is already implemented and tested in the current
  tree; acceptance now is preservation and explicit coverage.
  Date/Author: 2026-03-03 / Codex

- Decision: implement stable diagnostics through one canonical JSON schema in
  `sempai_core`, then route parser and validator producers through it.
  Rationale: schema centralization prevents drift and satisfies snapshot
  stability acceptance criteria.
  Date/Author: 2026-03-03 / Codex

## Outcomes & retrospective

To be completed after implementation. Must include:

- which `E_SEMPAI_*` codes are emitted by each path,
- snapshot evidence for parser and validator payload stability,
- bare-invocation acceptance evidence,
- quality gate results (`check-fmt`, `lint`, `test`),
- roadmap checkbox updates.

## Context and orientation

Current relevant code and docs:

- `crates/sempai-core/src/diagnostic.rs`: current diagnostic types and codes.
- `crates/sempai/src/engine.rs`: current compile/execute entrypoints returning
  stub diagnostics.
- `crates/sempai-core/src/tests/diagnostic_tests.rs`: unit diagnostics tests.
- `crates/sempai-core/src/tests/behaviour.rs` and
  `crates/sempai-core/tests/features/sempai_core.feature`: current BDD for core
  diagnostics.
- `crates/weaver-cli/src/lib.rs`, `src/localizer.rs`,
  `src/tests/unit/bare_invocation.rs`, and
  `tests/features/weaver_cli.feature`: bare-invocation discoverability flow and
  tests.
- `docs/sempai-query-language-design.md`: required diagnostic structure and
  code list.
- `docs/users-guide.md`: user-facing Sempai and CLI behaviour docs.
- `docs/roadmap.md`: status checkboxes for `4.1.2` and discoverability
  milestone.

Terminology used in this plan:

- Parser path: diagnostic emitted while decoding YAML or one-liner DSL syntax.
- Validator path: diagnostic emitted after parse, while enforcing semantic or
  schema constraints.
- Primary span: the principal source location for the failure; represented as
  the diagnostic `span` in JSON.

## Plan of work

### Stage A: lock the diagnostic schema in `sempai_core` (red first)

Add failing tests first for the stable JSON contract expected from parser and
validator paths. Introduce snapshots that assert the exact shape and field
names for:

- parser failure report (example code: `E_SEMPAI_YAML_PARSE` or
  `E_SEMPAI_DSL_PARSE`);
- validator failure report (example code:
  `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND` or `E_SEMPAI_SCHEMA_INVALID`).

Use deterministic fixture data (code, message, span, notes) so snapshots are
stable and easy to review.

### Stage B: implement canonical report schema and path constructors

In `sempai_core`, refine diagnostics APIs so parser and validator producers use
the same schema builder surface. Keep output contract stable:

- `code` (stable string code),
- `message` (short human-readable message),
- `span` (primary span; nullable only when no source location exists),
- `notes` (ordered list of supplemental notes).

If needed, add small constructor helpers for parser/validator contexts (for
example, `from_parser_error`, `from_validation_error`) but ensure they compose
through one underlying `Diagnostic` shape.

### Stage C: wire parser/validator emission points in `sempai` scaffolding

Update `Engine::compile_yaml` and `Engine::compile_dsl` so they exercise the
new parser-path diagnostics. Add a minimal validator-path call site in the same
scaffold layer (or shared test helper) to prove both paths emit the same
schema.

This stage does not implement full YAML/DSL parsers from 4.1.3/4.1.6; it only
ensures stable diagnostics plumbing and contracts.

### Stage D: expand tests (unit + BDD + snapshots)

Add or update:

- `sempai_core` unit tests for schema fields and ordering;
- snapshot tests for parser and validator reports;
- `rstest-bdd` scenarios that cover happy/unhappy diagnostic-path cases;
- regression tests ensuring any remaining stub path still returns deterministic
  diagnostics.

For CLI discoverability, keep existing behaviour and strengthen assertions if
needed so acceptance remains explicit: non-zero exit, `Usage:` line, three
domains, exactly one `weaver --help` pointer.

### Stage E: documentation and roadmap updates

Update:

- `docs/sempai-query-language-design.md` with decisions on diagnostic schema
  and any naming/serialization clarifications.
- `docs/users-guide.md` with the new stable diagnostics contract and examples
  of parser vs validator errors.
- `docs/roadmap.md` by marking `4.1.2` as done after verification. For
  discoverability, keep the existing completed checkbox in its current section.

### Stage F: full validation gates and evidence capture

Run repository gates with logs captured via `tee` and `set -o pipefail`:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/execplan-4-1-2-check-fmt.log
make lint 2>&1 | tee /tmp/execplan-4-1-2-lint.log
make test 2>&1 | tee /tmp/execplan-4-1-2-test.log
```

If Markdown files changed, also run:

```bash
set -o pipefail
make markdownlint 2>&1 | tee /tmp/execplan-4-1-2-markdownlint.log
make nixie 2>&1 | tee /tmp/execplan-4-1-2-nixie.log
```

Do not mark completion until all applicable gates pass.

## Concrete steps

All commands are executed from the workspace root (`/home/user/project`).

- Inspect current diagnostics and tests:

```bash
rg -n "DiagnosticCode|DiagnosticReport|E_SEMPAI|NOT_IMPLEMENTED" \
  crates/sempai-core crates/sempai
```

- Add failing unit/snapshot tests for parser and validator JSON contracts.

- Implement schema builders and path-specific constructors in
   `crates/sempai-core/src/diagnostic.rs` (or focused submodules if needed to
   stay under line limits).

- Wire compile entrypoints in `crates/sempai/src/engine.rs` to emit parser and
   validator diagnostics through the canonical schema.

- Add/adjust BDD features and step definitions in `sempai_core` / `sempai`.

- Re-run discoverability tests in `weaver-cli` and adjust only if regression
   is detected.

- Update documentation and roadmap.

- Run quality gates and review logs:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/execplan-4-1-2-check-fmt.log
make lint 2>&1 | tee /tmp/execplan-4-1-2-lint.log
make test 2>&1 | tee /tmp/execplan-4-1-2-test.log
```

## Validation and acceptance

Acceptance checks for 4.1.2:

1. Diagnostics include `code`, `message`, primary `span`, and `notes`.
2. Parser and validator diagnostic JSON snapshots are stable and pass.
3. Unit and BDD tests cover happy and unhappy paths, including edge cases.

Acceptance checks for bare invocation discoverability contract:

1. `weaver` with no arguments exits non-zero.
2. Output contains a `Usage:` line.
3. Output lists `observe`, `act`, and `verify`.
4. Output contains exactly one `weaver --help` pointer.

Quality criteria:

- `make check-fmt` exits 0.
- `make lint` exits 0.
- `make test` exits 0.
- `make markdownlint` and `make nixie` exit 0 when docs are changed.

## Idempotence and recovery

- All edits are source-controlled and re-runnable.
- Snapshot updates are deterministic; if a snapshot unexpectedly changes,
  inspect semantic intent before accepting.
- If a stage fails, revert only the incomplete stage changes and re-run from
  the last passing gate.
- Do not use destructive git commands (`reset --hard` / checkout overwrite).

## Interfaces and dependencies

Expected interfaces after implementation:

- Stable diagnostic JSON contract centered in `sempai_core`.
- Parser and validator producers emitting `DiagnosticReport` via the same
  schema path.
- `sempai` compile entrypoints returning deterministic `E_SEMPAI_*` diagnostics
  where applicable.
- No new external dependencies required.

## Revision note

Initial draft created for roadmap item `4.1.2` and the bare-invocation
discoverability acceptance contract referenced in the request.
