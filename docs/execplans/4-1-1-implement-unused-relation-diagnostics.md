# Implement `unused-relation` diagnostics

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

After this change, a rule set that declares a relation but never references it
anywhere else will produce one deterministic structured diagnostic per unused
declaration. The diagnostic will point at the declaration span, use the stable
Sempai diagnostic schema (`code`, `message`, `primary_span`, `notes`), and be
emitted in source order. A rule set where every declared relation is used will
not emit `unused-relation` diagnostics.

The implementation must also leave behind regression coverage at three levels:
direct unit tests for the lint rule, behaviour tests through the public compile
surface, and full workspace gate runs. Because this task changes Markdown
documentation as well as Rust code, the final validation set includes both the
Rust gates and the Markdown gates.

Observable outcome at completion:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/unused-relation-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/unused-relation-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/unused-relation-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/unused-relation-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/unused-relation-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/unused-relation-test.log
```

## Constraints

- Preserve the stable diagnostic schema already established in
  `crates/sempai-core/src/diagnostic.rs`: emitted JSON must continue to use
  `code`, `message`, `primary_span`, and `notes`, and must not introduce a new
  top-level diagnostic shape in this task.
- Treat `unused-relation` as a semantic-analysis lint that runs only after
  successful parsing, normalization, and name resolution. Do not add a second,
  parallel relation checker outside the existing rule-runner and semantic
  analysis pipeline.
- The lint must operate on resolved declarations and resolved usage sites, not
  raw text matching. The declaration itself does not count as a usage site.
- Emit at most one `unused-relation` diagnostic per declaration, in stable
  source order.
- The diagnostic `primary_span` must identify the declaration name token or the
  smallest available span representing that declaration. Do not point at the
  whole file or a synthetic span if a declaration span exists.
- If semantic analysis already failed for the same rule file in a way that
  prevents trustworthy relation binding, do not layer `unused-relation`
  diagnostics on top of a broken symbol table.
- Keep the `sempai` facade stable. Any changes needed for this task must remain
  consumable through existing `sempai` public types and compile methods.
- Keep files under the repository's 400-line limit. If the lint implementation
  or tests approach that limit, extract helper modules instead of growing one
  file further.
- Update the relevant design document with any final contract decisions. The
  prompt names `docs/ddlint-design.md`, but that file is not present in this
  checkout, so this plan uses `docs/sempai-query-language-design.md` as the
  live source of truth until the missing document appears.
- Update `docs/roadmap.md` only after the implementation and all gates pass.
  The prompt refers to a roadmap section that is not present in the current
  checkout, so roadmap edits must reconcile that mismatch explicitly rather
  than silently renumbering unrelated items.

## Tolerances (exception triggers)

- Prerequisites: if the promised rule-runner and semantic-analysis substrate is
  still absent from the working tree at implementation time, stop and escalate
  before writing code. This checkout currently has only `sempai_core` and the
  `sempai` facade, and `Engine::compile_yaml` still returns `NOT_IMPLEMENTED`.
- Interface: if implementing `unused-relation` requires adding a new public
  field to `Diagnostic` or changing existing serialized field names, stop and
  escalate.
- Scope: if the change grows beyond 15 touched files or roughly 600 net new
  lines, stop and re-evaluate whether prerequisite work is being folded into
  this milestone accidentally.
- Dependencies: if a new third-party crate is required, stop and escalate.
- Ambiguity: if the relation grammar or usage-site definition cannot be derived
  from code already in the working tree, stop and request the missing
  specification or prerequisite branch instead of guessing.
- Roadmap: if marking the roadmap item done requires inventing a new roadmap
  section because the task does not exist in `docs/roadmap.md`, stop and ask
  whether the roadmap should be rebased first.
- Iterations: if the same failing test or lint loop repeats 5 times without a
  clear change in hypothesis, stop and document the blocker.

## Risks

- Risk: the prompt references `docs/ddlint-design.md`,
  `docs/differential-datalog-parser-syntax-spec-updated.md`, and
  `docs/parser-implementation-notes.md`, but none of those files exist in this
  checkout. Severity: high. Likelihood: high. Mitigation: begin with a
  prerequisite audit, use the current Sempai design doc as the working source
  of truth, and escalate if the missing docs are needed to resolve grammar
  ambiguity.
- Risk: the current roadmap has been renumbered and does not contain the
  requested "4. Lint rule implementation" section. Severity: high. Likelihood:
  high. Mitigation: document the mismatch, anchor the work to the requested
  task name and current branch name, and update the roadmap only with an
  explicit reconciliation note.
- Risk: the stable diagnostic contract has no severity field, but lint rules
  are often warnings rather than hard errors. Severity: medium. Likelihood:
  medium. Mitigation: keep this task within the existing contract and add one
  new stable code for the lint; defer any broader severity model to a separate
  milestone.
- Risk: relation usage can be undercounted if the checker walks syntax instead
  of resolved semantic references. Severity: high. Likelihood: medium.
  Mitigation: base the lint on the semantic symbol table or resolved IR
  produced by the prerequisite work, and write tests covering aliases or
  indirection if those exist.
- Risk: behaviour tests can become brittle if they depend on speculative rule
  syntax examples while the referenced grammar docs are missing. Severity:
  medium. Likelihood: medium. Mitigation: derive fixtures from the actual
  parser tests or semantic-IR fixtures present once the prerequisite branch is
  merged.

## Progress

- [x] (2026-03-21 00:00Z) Audited the current checkout, roadmap, and existing
  Sempai diagnostics before drafting this plan.
- [ ] Stage 0: Confirm or merge the prerequisite rule-runner and semantic
  analysis implementation for relation declarations and usage sites.
- [ ] Stage 1: Lock the new diagnostic contract in tests before production
  changes.
- [ ] Stage 2: Implement the semantic `unused-relation` rule on resolved
  relation symbols.
- [ ] Stage 3: Wire the rule into the lint catalog or compile pipeline so it
  is observable through the public API.
- [ ] Stage 4: Update design documentation and reconcile the roadmap entry.
- [ ] Stage 5: Run targeted verification and all required workspace gates.

## Surprises & Discoveries

- The current branch name is `implement-unused-relation-lint-x5jvb3`, which
  matches the requested task, but the live `docs/roadmap.md` does not yet have
  the same milestone structure. The current section `4.*` covers Sempai query
  infrastructure instead.
- The files named in the prompt for Datalog-specific parser and lint design are
  absent from this checkout.
- The only Sempai crates present today are `crates/sempai-core` and
  `crates/sempai`. No parser, normalization, or semantic-analysis crate is
  present yet, and the facade still returns `NOT_IMPLEMENTED` from
  `compile_yaml`, `compile_dsl`, and `execute`.
- Stable structured diagnostics already exist in
  `crates/sempai-core/src/diagnostic.rs`, along with unit tests, BDD scenarios,
  and snapshot tests added by the completed 4.1.2 milestone.

## Decision Log

- Decision: keep `unused-relation` inside the existing structured diagnostic
  contract rather than adding a severity field in this task. Rationale: the
  stable contract was just locked in 4.1.2, and broadening it now would turn a
  focused lint implementation into a schema redesign.
- Decision: the lint should run after semantic name resolution and before
  execution. Rationale: that is the earliest phase where declaration-to-use
  bindings are trustworthy, and it keeps the rule independent of backend
  execution details.
- Decision: the diagnostic span should target the declaration name, not the
  whole declaration form. Rationale: that gives the operator the most precise
  actionable location and aligns with the established primary-span contract.
- Decision: one declaration yields one diagnostic even if the relation has zero
  uses across multiple scopes. Rationale: duplicate diagnostics for the same
  declaration add noise without new information.
- Decision: if the prerequisite semantic layer already defines a lint catalog
  registration mechanism, use it. Do not special-case `unused-relation` by
  inserting an ad hoc pass directly into the facade or a test harness.

## Outcomes & Retrospective

Target outcome at completion:

1. A declared-but-unused relation produces a deterministic structured
   diagnostic with a stable code and declaration span.
2. A relation that is referenced at least once does not trigger the lint.
3. Multiple unused declarations emit diagnostics in stable source order.
4. Unit tests, behaviour tests, and snapshots cover the contract and the
   semantic edge cases.
5. The design document records the final code, message shape, ordering rule,
   and phase ordering for this lint.
6. `docs/roadmap.md` is reconciled with the task request instead of silently
   drifting further.
7. `make check-fmt`, `make lint`, and `make test` pass, plus the Markdown
   gates required for the documentation edits.

## Context and orientation

This repository currently contains the Sempai core data model in
`crates/sempai-core` and a facade crate in `crates/sempai`. The stable
diagnostic contract already exists in `crates/sempai-core/src/diagnostic.rs`,
with regression coverage in:

- `crates/sempai-core/src/tests/diagnostic_tests.rs`
- `crates/sempai-core/src/tests/diagnostic_snapshot_tests.rs`
- `crates/sempai-core/tests/features/sempai_core.feature`

The public `Engine` entrypoints in `crates/sempai/src/engine.rs` are still
stubs returning `DiagnosticReport::not_implemented(...)`, and the behaviour
tests in `crates/sempai/tests/features/sempai_engine.feature` currently lock in
that stub behaviour.

This means the requested lint-rule work cannot be implemented only by editing
today's files. The implementation must either:

1. land on top of a not-yet-merged prerequisite branch that adds parsing,
   normalization, semantic analysis, and a rule-runner, or
2. explicitly expand scope to include those prerequisites.

This plan assumes the first case. If implementation begins and the working tree
still looks like today's checkout, Stage 0 is a hard stop until the missing
substrate is present.

Use these existing files as the anchor points that will definitely change once
the prerequisite branch is available:

- `crates/sempai-core/src/diagnostic.rs`
- `crates/sempai-core/src/tests/diagnostic_tests.rs`
- `crates/sempai-core/src/tests/diagnostic_snapshot_tests.rs`
- `crates/sempai-core/tests/features/sempai_core.feature`
- `crates/sempai/src/engine.rs`
- `crates/sempai/src/tests/behaviour.rs`
- `crates/sempai/tests/features/sempai_engine.feature`
- `docs/sempai-query-language-design.md`
- `docs/roadmap.md`

The concrete semantic-analysis files introduced by the prerequisite work should
also be updated, but this plan intentionally avoids inventing filenames that do
not exist yet in the current tree.

## Plan of work

### Stage 0: Prerequisite audit and route selection

Before writing code, confirm that the promised substrate exists in the working
tree:

1. Find the semantic-analysis module that owns resolved relation declarations
   and resolved usage sites.
2. Find the lint catalog or rule-runner registration surface that executes
   semantic rules after name resolution.
3. Confirm whether `compile_yaml` or an equivalent compilation entrypoint now
   surfaces semantic diagnostics through `DiagnosticReport`.

Go/no-go rule:

- If any of those pieces are still missing, stop and escalate. Do not build a
  one-off relation checker on the facade stub path.

### Stage 1: Lock the contract in tests first

Write failing tests before production code.

At minimum add:

1. Diagnostic-code coverage in
   `crates/sempai-core/src/tests/diagnostic_tests.rs` for the new stable code,
   expected display text, and serde round-trip.
2. A JSON snapshot in
   `crates/sempai-core/src/tests/diagnostic_snapshot_tests.rs` proving the new
   diagnostic serializes with `code`, `message`, `primary_span`, and `notes`
   only.
3. A BDD scenario in
   `crates/sempai-core/tests/features/sempai_core.feature` covering the
   contract-level shape of a single `unused-relation` diagnostic.
4. Semantic-lint unit tests in the prerequisite semantic crate that cover:
   one unused declaration, one used declaration, mixed used/unused
   declarations, and stable ordering for multiple unused declarations.
5. A behaviour-level test through the public compile surface. Update
   `crates/sempai/tests/features/sempai_engine.feature` and
   `crates/sempai/src/tests/behaviour.rs` only after the facade compilers stop
   being stubs and can surface real semantic diagnostics.

Fixture guidance:

- Derive the relation syntax from the actual parser fixtures that exist in the
  prerequisite branch. Do not invent new syntax examples based only on the task
  name.
- Keep fixtures small. Prefer one declaration and one use site per scenario so
  failures isolate cleanly.

### Stage 2: Extend the diagnostic contract, not the schema

Add one new stable diagnostic code in `crates/sempai-core/src/diagnostic.rs`.
Use the project's established naming scheme. If no lint-specific code exists in
the prerequisite branch, add `E_SEMPAI_UNUSED_RELATION`.

Required behaviour:

1. `Display` returns the stable string for the new code.
2. Serde serializes and deserializes the new code exactly.
3. The new code flows through existing `Diagnostic`, `DiagnosticReport`,
   parser-error, and validation-error constructors without schema changes.

Do not add a new severity field or a second diagnostic envelope in this stage.

### Stage 3: Implement the semantic rule

Implement the rule in the semantic-analysis crate introduced by the
prerequisites.

Required algorithm:

1. Build or reuse the resolved relation table produced by semantic analysis.
2. For each declared relation symbol, count resolved usage sites elsewhere in
   the rule or rule set.
3. Exclude the declaration site itself from that count.
4. Emit one diagnostic when the usage count is zero.
5. Sort emitted diagnostics by declaration source order before returning them.

Keep the implementation honest:

- Use semantic IDs or resolved references, not string equality on relation
  names.
- If the semantic layer already rejects duplicate declarations or unresolved
  references, run `unused-relation` only after those checks succeed.
- If helper signatures start to grow, group context into small structs instead
  of tripping the repository's denied Clippy arity lints.

Expected diagnostic content:

- Code: the new stable code from Stage 2.
- Message: `relation '<name>' is declared but never used` unless the
  prerequisite design doc already defines exact copy.
- Primary span: declaration name span.
- Notes: one short remediation note, for example `remove the declaration or add
  a usage site`, unless the prerequisite design doc prescribes different copy.

### Stage 4: Wire the rule into the lint catalog and public surface

Register the rule with the semantic lint catalog or rule runner rather than
calling it manually from a test.

Required behaviour:

1. A successful parse plus semantic analysis with an unused relation must
   surface the diagnostic through the public compilation path.
2. A rule set with no unused relations must not emit the diagnostic.
3. Existing non-lint semantic failures keep their current precedence and do not
   gain spurious `unused-relation` noise.

If the prerequisite implementation already has a catalog abstraction for
multiple lint rules, add `unused-relation` there and cover the registration in
unit tests. If it does not, stop and confirm whether introducing the catalog is
in scope for this task.

### Stage 5: Documentation and roadmap reconciliation

Update the live design documentation with the final behaviour:

1. In `docs/sempai-query-language-design.md`, add a short subsection covering:
   the new code name, phase ordering, one-diagnostic-per-declaration semantics,
   source-order stability, and the declaration-span rule.
2. If a Datalog-specific design doc appears during implementation, mirror the
   same decisions there and note which document is normative.
3. Update `docs/users-guide.md` only if the public compile surface or user
   diagnostics section now exposes this rule directly.
4. Reconcile `docs/roadmap.md` explicitly. If the requested task entry exists
   after rebasing, mark it done. If it still does not exist, add a short note
   near the relevant Sempai milestone instead of silently checking an unrelated
   item.

### Stage 6: Verification and evidence capture

Run targeted tests first, then the full gates. Always use `tee` and
`set -o pipefail` so truncated output does not hide failures.

Suggested targeted commands once the semantic crate exists:

```plaintext
set -o pipefail; cargo test -p sempai_core diagnostic --all-targets --all-features 2>&1 | tee /tmp/unused-relation-sempai-core-diagnostic.log
set -o pipefail; cargo test -p sempai --all-targets --all-features 2>&1 | tee /tmp/unused-relation-sempai-facade.log
set -o pipefail; cargo test -p <semantic-package> unused_relation --all-targets --all-features 2>&1 | tee /tmp/unused-relation-semantic.log
```

Full required gates:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/unused-relation-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/unused-relation-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/unused-relation-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/unused-relation-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/unused-relation-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/unused-relation-test.log
```

Success criteria:

1. The new diagnostic code is covered by unit tests, BDD, and JSON snapshots.
2. The semantic lint tests prove one-diagnostic-per-unused-declaration and
   no false positives for used relations.
3. The public compile behaviour shows the diagnostic at the expected span.
4. All required gate commands exit 0.

## Rollback and retry notes

This task should be additive and safe to retry. If a stage fails:

1. Keep the failing test that exposed the issue.
2. Fix the production code or the incorrect assumption, not the symptom.
3. Re-run the narrowest relevant targeted command first.
4. Only re-run the full workspace gates after the targeted scope is green.

If implementation reveals that the prerequisite branch is still missing, the
correct rollback is not to delete tests. Instead, stop after Stage 0, record
the blocker in `Decision Log`, and wait for the prerequisite substrate or an
explicit scope change.
