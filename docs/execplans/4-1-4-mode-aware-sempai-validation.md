# 4.1.4 Implement mode-aware Sempai validation and execution gating

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

After this change, Sempai will distinguish between rules it can parse and rules
it can actually execute. `sempai_yaml` must continue to accept
Semgrep-compatible `search`, `extract`, `taint`, and `join` rule files for
interoperability, but `sempai::Engine::compile_yaml` must stop treating every
successfully parsed rule the same way.

Observable user-facing behaviour after implementation:

- Valid `search` rules that satisfy the required key combinations continue past
  parsing and validation, then stop at the existing normalization placeholder
  with `NOT_IMPLEMENTED` until roadmap item 4.1.5 lands.
- Valid `extract`, `taint`, `join`, and any other non-search modes fail
  deterministically with `E_SEMPAI_UNSUPPORTED_MODE` instead of falling through
  to the generic placeholder.
- Invalid `search` rules fail deterministically with
  `E_SEMPAI_SCHEMA_INVALID`, including the missing-principal combinations
  defined in
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md).

This milestone is deliberately narrower than normalization or execution. It
must deliver the semantic validation boundary promised in roadmap item 4.1.4
without pulling 4.1.5 or 4.2.x work forward.

Observable completion evidence:

```plaintext
set -o pipefail; cargo test -p sempai_yaml 2>&1 | tee /tmp/4-1-4-sempai-yaml-test.log
set -o pipefail; cargo test -p sempai 2>&1 | tee /tmp/4-1-4-sempai-test.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-4-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-4-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-4-make-test.log
```

Because this milestone updates Markdown documentation and the roadmap:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-4-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-4-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-4-make-nixie.log
```

## Constraints

- Keep the implementation aligned with
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md),
  especially the mode-handling section, the schema-aligned validation section,
  and the parser-to-validation pipeline.
- Preserve the current compatibility split:
  - `sempai_yaml` parses all supported Semgrep modes.
  - `sempai::Engine::compile_yaml` gates execution to supported modes.
  - Unsupported-mode handling must not be pushed down into the parser as a
    parse-time rejection.
- Maintain the stable diagnostics contract from 4.1.2:
  `code`, `message`, `primary_span`, and `notes` remain the only emitted fields.
- Use `DiagnosticReport::validation_error(...)` for engine-side mode gating and
  keep `DiagnosticReport::parser_error(...)` for YAML shape and deserialization
  failures.
- Search-mode validation must cover the required key combinations documented in
  the design, including the Semgrep compatibility key
  `r2c-internal-project-depends-on`, which must be parsed but may remain
  execution-no-op for now.
- Keep this milestone out of 4.1.5 scope:
  do not normalize legacy and v2 principals into canonical `Formula`, and do
  not produce real `QueryPlan` values yet.
- Keep this milestone out of 4.1.6 scope:
  do not implement the one-liner domain-specific language (DSL) parser as part
  of this work.
- Add both unit tests and behaviour-driven development (BDD) tests using
  `rstest-bdd` v0.5.0. Cover happy paths, unhappy paths, and edge cases.
- Keep files below 400 lines. Split validator code and test helpers into small,
  purpose-specific modules instead of growing `engine.rs` or
  `crates/sempai-yaml/src/parser/builders.rs` into catch-all files.
- Every new module must begin with a `//!` module comment, and new public items
  must carry Rustdoc with examples where appropriate.
- Record any implementation decisions in
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md).
- Update [docs/users-guide.md](../users-guide.md) with the user-visible change
  in `compile_yaml(...)` behaviour by mode.
- Mark roadmap item 4.1.4 done in [docs/roadmap.md](../roadmap.md) only after
  all tests and quality gates pass.

## Tolerances

- Scope: if implementation requires more than 14 net file touches outside
  `crates/sempai/`, `crates/sempai-yaml/`, and the three required docs, stop
  and escalate.
- Interface: if satisfying deterministic `UnsupportedMode` diagnostics requires
  a breaking change to the public signature of `sempai::Engine::compile_yaml`
  or `sempai_yaml::parse_rule_file`, stop and escalate.
- Model shape: if engine-side diagnostics require source-span data that cannot
  be exposed additively from `sempai_yaml` models, stop and present the least
  disruptive API options before proceeding.
- Schema ambiguity: if the local Semgrep schema and the design document disagree
  materially about whether `r2c-internal-project-depends-on` satisfies search
  mode's required-principal contract, stop and escalate with the competing
  interpretations.
- Behaviour: if mixed-mode YAML files force a partial-success design that
  cannot fit the current `Result<Vec<QueryPlan>, DiagnosticReport>` surface,
  stop and escalate rather than inventing ad hoc partial compilation.
- Iterations: if the same failing lint or test loop is attempted five times
  without a clear path forward, stop and escalate.

## Risks

- Risk: the current `sempai_yaml::Rule` model does not expose source spans, but
  engine-side `UnsupportedMode` diagnostics should point at a deterministic
  location. Severity: high. Likelihood: high. Mitigation: carry rule-level or
  mode-field span information through the parsed model in an additive way so
  the facade can emit anchored validation errors.

- Risk: `sempai_yaml` already performs some mode-specific rejection, so the
  boundary between parser validation and engine validation is easy to blur.
  Severity: high. Likelihood: medium. Mitigation: keep schema and
  principal-shape validation in `sempai_yaml`, and add a separate engine-side
  validation pass only for execution support and search-mode semantic
  combinations.

- Risk: search-mode validation in the design includes
  `r2c-internal-project-depends-on`, but the current parser does not model that
  key. Severity: medium. Likelihood: high. Mitigation: add parser support for
  that compatibility key now and document that it satisfies validation while
  remaining ignored by execution and normalization until a later milestone
  needs more semantics.

- Risk: mixed documents containing both valid `search` rules and unsupported
  modes may tempt a partial compilation strategy. Severity: medium. Likelihood:
  medium. Mitigation: keep `compile_yaml(...)` whole-document and fail on the
  first unsupported rule in source order, documenting that deterministic
  behaviour in the design doc and tests.

- Risk: strict workspace lints will apply to behavioural test helpers and
  fixtures. Severity: low. Likelihood: medium. Mitigation: prefer small helper
  structs and shared test helpers over large step-definition functions with
  many parameters.

## Progress

- [x] (2026-03-28 UTC) Reviewed roadmap item 4.1.4, the Sempai design doc, the
  current `sempai_yaml` parser/builder code, the `sempai` facade tests,
  and adjacent ExecPlans.
- [x] (2026-03-28 UTC) Drafted this ExecPlan.
- [x] (2026-03-29 UTC) Stage A: Locked the intended behaviour with unit and
  BDD tests for dependency search rules, unsupported execution modes, and
  the preserved search-mode placeholder path.
- [x] (2026-03-29 UTC) Stage B: Added additive parsed-rule metadata
  (`mode_span` plus enclosing rule span) and model support for the
  compatibility key `r2c-internal-project-depends-on`.
- [x] (2026-03-29 UTC) Stage C: Implemented engine-side whole-document mode
  gating that reports the first unsupported rule in source order via
  `E_SEMPAI_UNSUPPORTED_MODE`.
- [x] (2026-03-29 UTC) Stage D: Updated the Sempai design doc, the user's
  guide, and the roadmap to reflect mode-aware `compile_yaml(...)`
  behaviour.
- [x] (2026-03-29 UTC) Stage E: Ran `make fmt`, `make markdownlint`,
  `make nixie`, `make check-fmt`, `make lint`, `make test`,
  `cargo test -p sempai_yaml`, and `cargo test -p sempai`.

## Surprises & Discoveries

- Observation: `crates/sempai-yaml/src/parser/mod.rs` already rejects
  cross-mode principal families, and
  `crates/sempai-yaml/src/parser/builders.rs` already rejects `match` for
  `extract` and `taint`. Impact: 4.1.4 is not starting from zero; it must
  clarify the parser-vs-engine boundary instead of re-implementing existing
  schema checks.

- Observation: `crates/sempai/src/engine.rs` currently parses YAML and then
  unconditionally returns `DiagnosticReport::not_implemented(...)` for any
  successful parse result. Impact: the acceptance gap is concentrated in the
  facade's post-parse validation path.

- Observation: the public `sempai_yaml::Rule` model currently preserves mode,
  principal, and metadata, but not rule source spans. Impact: deterministic
  engine-side `UnsupportedMode` diagnostics probably require additive model
  metadata before the facade can emit anchored validation errors.

- Observation: the design doc explicitly names
  `r2c-internal-project-depends-on` as satisfying search mode's required key
  combinations, but the current parser does not model that field at all.
  Impact: acceptance requires a small compatibility expansion in `sempai_yaml`,
  not just a facade change.

- Observation: current BDD coverage only locks three YAML parser paths and the
  generic engine placeholder path. Impact: 4.1.4 must add behavioural coverage
  in both `sempai_yaml` and `sempai`, not just unit assertions.

- Observation: the package name for focused parser tests is `sempai_yaml`
  rather than `sempai-yaml`, so the crate-level verification commands needed to
  use the underscore form. Impact: the completion evidence now records the
  executable commands that actually passed in this workspace.

## Decision Log

- Decision: keep parse-time and execution-time concerns separate. `sempai_yaml`
  remains responsible for YAML shape, required-field presence, and cross-family
  principal validation; `sempai` gains an explicit validator pass for execution
  support and supported-mode gating. Rationale: this preserves the design's
  "parse all, execute search only" contract. Date/Author: 2026-03-28 / Codex.

- Decision: treat unsupported modes as engine validation failures, not parser
  failures. Rationale: `extract`, `taint`, and `join` must remain parseable for
  compatibility even though execution is not implemented. Date/Author:
  2026-03-28 / Codex.

- Decision: fail whole-document compilation on the first unsupported or
  semantically invalid rule in source order. Rationale: the current public API
  returns either `Vec<QueryPlan>` or one `DiagnosticReport`; deterministic
  first-failure behaviour is simpler and matches the current compilation
  surface. Date/Author: 2026-03-28 / Codex.

- Decision: carry source-location data through the parsed rule model in an
  additive form if engine diagnostics need it. Rationale: deterministic
  `primary_span` values are more useful than location-free `UnsupportedMode`
  errors and remain compatible with the existing public API. Date/Author:
  2026-03-28 / Codex.

- Decision: keep `r2c-internal-project-depends-on` opaque inside
  `SearchQueryPrincipal` rather than introducing a partially normalized
  dependency model. Rationale: 4.1.4 only needs the key to satisfy search-mode
  validation and preserve forward-compatible data; richer semantics belong to
  later normalization and execution milestones. Date/Author: 2026-03-29 / Codex.

## Outcomes & Retrospective

Target outcome at completion:

1. `sempai_yaml` parses `search`, `extract`, `taint`, `join`, and
   forward-compatible mode strings while preserving enough metadata for a
   separate validation pass.
1. Search mode validation enforces the required key combinations documented in
   the design, including the compatibility-only
   `r2c-internal-project-depends-on` key.
1. `sempai::Engine::compile_yaml(...)` returns
   `E_SEMPAI_UNSUPPORTED_MODE` for `extract`, `taint`, `join`, and other
   non-search modes, with deterministic messaging and stable diagnostics.
1. Valid `search` rules continue to the existing normalization placeholder and
   still return `NOT_IMPLEMENTED` until 4.1.5 is complete.
1. Unit tests and `rstest-bdd` v0.5.0 scenarios cover happy, unhappy, and edge
   paths in both `sempai_yaml` and `sempai`.
1. `docs/sempai-query-language-design.md` records the final boundary between
   parser validation and engine validation.
1. `docs/users-guide.md` explains the mode-specific `compile_yaml(...)`
   behaviour users now see.
1. `docs/roadmap.md` marks 4.1.4 done.
1. `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
   `make lint`, and `make test` all pass.

Retrospective notes:

- Engine validation diagnostics now prefer the parsed `mode` field span and
  fall back to the enclosing rule span. This keeps unsupported-mode reports
  anchored to the most actionable location without changing public signatures.
- `r2c-internal-project-depends-on` stayed opaque as
  `SearchQueryPrincipal::ProjectDependsOn(ProjectDependsOnPayload)`. That was
  enough to satisfy search-mode validation while deferring dependency semantics
  to a later milestone.
- No existing parser-time checks had to move into the engine validator. The
  parser still owns YAML shape, required metadata fields, and cross-family
  principal rejection; the engine now owns execution support gating.
- Final verification commands and logs:
  - `set -o pipefail; cargo test -p sempai_yaml 2>&1 | tee /tmp/4-1-4-sempai-yaml-test.log`
  - `set -o pipefail; cargo test -p sempai 2>&1 | tee /tmp/4-1-4-sempai-test.log`
  - `set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-4-make-fmt.log`
  - `set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-4-make-markdownlint.log`
  - `set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-4-make-nixie.log`
  - `set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-4-make-check-fmt.log`
  - `set -o pipefail; make lint 2>&1 | tee /tmp/4-1-4-make-lint.log`
  - `set -o pipefail; make test 2>&1 | tee /tmp/4-1-4-make-test.log`

## Context and orientation

Current files that matter for this milestone:

- [crates/sempai/src/engine.rs](../../crates/sempai/src/engine.rs)
- [crates/sempai/src/tests/engine_tests.rs](../../crates/sempai/src/tests/engine_tests.rs)
- [crates/sempai/src/tests/behaviour.rs](../../crates/sempai/src/tests/behaviour.rs)
- [crates/sempai/tests/features/sempai_engine.feature](../../crates/sempai/tests/features/sempai_engine.feature)
- [crates/sempai-yaml/src/parser/mod.rs](../../crates/sempai-yaml/src/parser/mod.rs)
- [crates/sempai-yaml/src/parser/builders.rs](../../crates/sempai-yaml/src/parser/builders.rs)
- [crates/sempai-yaml/src/model.rs](../../crates/sempai-yaml/src/model.rs)
- [crates/sempai-yaml/src/raw.rs](../../crates/sempai-yaml/src/raw.rs)
- [crates/sempai-yaml/src/tests/behaviour.rs](../../crates/sempai-yaml/src/tests/behaviour.rs)
- [crates/sempai-yaml/tests/features/sempai_yaml.feature](../../crates/sempai-yaml/tests/features/sempai_yaml.feature)
- [crates/sempai-core/src/diagnostic.rs](../../crates/sempai-core/src/diagnostic.rs)
- [docs/sempai-query-language-design.md](../sempai-query-language-design.md)
- [docs/users-guide.md](../users-guide.md)
- [docs/roadmap.md](../roadmap.md)

Current behaviour to preserve or intentionally change:

- `parse_rule_file(...)` already produces real parser/schema diagnostics.
- Search rules already require `message`, `languages`, and `severity` in the
  parser path, and they already reject mixed legacy-plus-`match` principals.
- `compile_yaml(...)` still treats every successfully parsed rule file as the
  same generic placeholder case.
- The public diagnostics contract is already stable; this milestone must change
  the emitted codes and messages for some valid YAML inputs, not the payload
  schema itself.

## Plan of work

### Stage A: Lock expected behaviour with failing tests first

Add tests before changing production code so the intended behaviour is explicit
and reviewable.

In `crates/sempai-yaml`:

- Add unit tests covering search-mode required-principal combinations:
  - legacy `pattern`
  - `patterns`
  - `pattern-either`
  - `pattern-regex`
  - `match`
  - `r2c-internal-project-depends-on`
- Add unhappy-path unit tests for search rules that declare the search header
  but none of the allowed principal keys.
- Extend the behaviour-driven development (BDD) feature file with one happy
  path proving the compatibility key is accepted and one unhappy path proving
  missing-principal search rules still fail with `E_SEMPAI_SCHEMA_INVALID`.

In `crates/sempai`:

- Add unit tests proving `compile_yaml(...)` returns
  `E_SEMPAI_UNSUPPORTED_MODE` for valid `extract`, `join`, and `taint` rules.
- Add a unit test for a forward-compatible unknown mode string such as
  `mode: custom-mode`, verifying deterministic `UnsupportedMode` handling.
- Add a unit test for a mixed-rule file, verifying that the first unsupported
  rule in source order determines the returned diagnostic.
- Extend the BDD feature file with:
  - a happy path for valid search mode that still reaches `NOT_IMPLEMENTED`
  - unhappy paths for `extract`, `join`, and `taint`
  - an edge path for mixed-mode ordering

Go/no-go:

- Do not proceed until at least one new `sempai_yaml` test and one new
  `sempai` test fail for the intended 4.1.4 behaviour.

### Stage B: Add the parser and model data the validator needs

Bridge the current data gap between parsed rules and engine-side validation.

- Extend `crates/sempai-yaml/src/raw.rs` to deserialize
  `r2c-internal-project-depends-on` in a forward-compatible way. It can remain
  opaque data if no typed semantics are needed yet.
- Update `crates/sempai-yaml/src/model.rs` so parsed search rules can report
  whether they satisfied validation through a recognized principal or through
  the compatibility key.
- Add additive source-location data to the parsed rule model so the facade can
  attach `primary_span` to engine-side validation diagnostics. A whole-rule
  span is sufficient if a mode-field span is not cheaply available.
- Keep parse-time schema validation in `sempai_yaml`; do not move existing
  malformed-YAML or structural checks into the facade.

Go/no-go:

- Do not proceed until the `sempai_yaml` unit and BDD suites pass and the new
  model still preserves existing parser behaviour for already-supported rules.

### Stage C: Implement engine-side mode-aware validation and gating

Add an explicit validation seam between YAML parsing and the normalization
placeholder.

- Introduce a small validator module in `crates/sempai/src/` rather than
  inflating `engine.rs`.
- Validate parsed rules in source order and stop on the first rule that cannot
  reach execution.
- For `search` rules:
  - accept rules whose required key combinations are satisfied
  - keep returning the existing `NOT_IMPLEMENTED` normalization placeholder
    after validation succeeds
- For `extract`, `join`, `taint`, and `RuleMode::Other(_)`:
  - return `DiagnosticReport::validation_error(...)`
  - use `DiagnosticCode::ESempaiUnsupportedMode`
  - include a deterministic message naming the unsupported mode
  - attach the span propagated from `sempai_yaml` when available
- Keep the execution surface unchanged:
  `Engine::execute(...)` still returns the existing placeholder until backend
  work lands in 4.2.x.

Go/no-go:

- Do not proceed until `cargo test -p sempai --all-targets --all-features`
  passes with the new gating behaviour.

### Stage D: Update design docs, user docs, and roadmap state

Once the implementation is stable, synchronize the living documentation.

- Update
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md):
  - clarify the parser-vs-engine validation boundary
  - document how unsupported modes are surfaced
  - record the compatibility treatment for
    `r2c-internal-project-depends-on`
- Update [docs/users-guide.md](../users-guide.md):
  - explain that `compile_yaml(...)` now distinguishes supported search rules
    from parse-only unsupported modes
  - show which diagnostic codes users should expect
- Update [docs/roadmap.md](../roadmap.md):
  - mark 4.1.4 done only after every required gate passes

Go/no-go:

- Do not mark the roadmap item complete until all code, tests, and docs have
  landed and passed their gates.

### Stage E: Run the full gate sequence and capture evidence

Run the targeted crate tests first, then the full repository gates using the
project-mandated `tee` logging pattern.

Required commands:

```plaintext
set -o pipefail; cargo test -p sempai_yaml --all-targets --all-features 2>&1 | tee /tmp/4-1-4-sempai-yaml-test.log
set -o pipefail; cargo test -p sempai --all-targets --all-features 2>&1 | tee /tmp/4-1-4-sempai-test.log
set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-4-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-4-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-4-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-4-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-4-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-4-make-test.log
```

Acceptance evidence to record in the finished plan:

- the exact failing tests from Stage A that turned green
- the final unsupported-mode messages asserted by unit and BDD tests
- the final log-file paths from the required gate runs
