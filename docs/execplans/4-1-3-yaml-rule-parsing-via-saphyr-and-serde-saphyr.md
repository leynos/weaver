# 4.1.3 Implement YAML rule parsing via `saphyr` and `serde-saphyr`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

After this change, Sempai will be able to read Semgrep-compatible YAML rule
files and turn them into typed rule models that preserve rule metadata and the
query principal for both legacy and v2 rule forms. Malformed YAML and
schema-shape failures will no longer collapse into the generic
`NOT_IMPLEMENTED` stub path; they will emit structured `DiagnosticReport`
payloads with stable `E_SEMPAI_*` codes and `primary_span` locations.

This milestone is intentionally narrower than full query compilation. It
delivers the YAML front-end described in
[docs/sempai-query-language-design.md](../sempai-query-language-design.md) and
prepares the normalization and validation work scheduled for 4.1.4 and 4.1.5.
Successful parsing is observable through a new public parser API and through
updated `sempai::Engine::compile_yaml` behaviour: malformed YAML must return
parser/schema diagnostics, while successfully parsed rules may still stop short
of executable `QueryPlan` output until the later normalization milestone lands.

Observable outcome after implementation:

```plaintext
cargo test -p sempai_yaml --all-targets --all-features
cargo test -p sempai --all-targets --all-features
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-3-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-3-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-3-make-test.log
```

And because this milestone updates documentation and roadmap state:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-3-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-3-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-3-make-nixie.log
```

## Constraints

- Keep the implementation aligned with
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md),
  especially Table 1, the YAML parser stack, the rule-model section, and the
  diagnostic contract.
- Use `saphyr` for YAML parsing and source-location retention, and
  `serde-saphyr` for deserializing YAML values into Rust structs. Do not swap
  in a different YAML stack.
- Create the YAML parser as the planned `crates/sempai-yaml` crate
  (`sempai_yaml`) rather than embedding the parser directly inside
  `sempai_core` or `sempai`. The design document already allocates that crate
  boundary.
- Preserve the `sempai` facade as the stable top-level entrypoint. Any new
  parser behaviour surfaced to end users must remain consumable through
  `sempai::Engine`.
- Do not conflate parser work with the later semantic validation and
  normalization milestones:
  - 4.1.3 parses metadata and query principals.
  - 4.1.4 handles mode-aware validation and supported-mode gating.
  - 4.1.5 normalizes legacy and v2 forms into canonical `Formula`.
- Continue using the stable diagnostics contract established in 4.1.2:
  `code`, `message`, `primary_span`, and `notes` remain the only emitted
  diagnostic keys.
- Add both unit tests and behaviour tests using `rstest-bdd` v0.5.0. Cover
  happy paths, unhappy paths, and relevant edge cases.
- Keep source files under 400 lines. Split parser, model, span-mapping, and
  test modules early instead of growing one large file.
- Every new module must start with a `//!` module comment, and every public
  item must have Rustdoc with examples where appropriate.
- Record any design decisions taken during implementation in
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md).
- Update [docs/users-guide.md](../users-guide.md) for any user-visible
  behaviour changes, including any change to `compile_yaml` failure semantics.
- Mark roadmap item 4.1.3 done in [docs/roadmap.md](../roadmap.md) only after
  the code, tests, and documentation all pass their gates.

## Tolerances (exception triggers)

- Scope: if implementation needs more than 16 net file touches outside the new
  `crates/sempai-yaml/` crate and its direct tests/docs wiring, stop and
  escalate.
- Interface: if this milestone requires changing the public signature of
  `Engine::compile_yaml` or any existing `sempai_core` diagnostic type, stop
  and escalate.
- Dependencies: if `saphyr` and `serde-saphyr` are insufficient and any
  additional third-party crate is needed, stop and escalate before adding it.
- Parsing contract: if the local Semgrep parser-aligned schema in
  `docs/semgrep-language-reference/semgrep-rule-schema.yaml` is ambiguous for a
  shape that materially affects the public Rust model, stop and present the
  alternatives.
- Diagnostics: if exact field-level span mapping for schema errors proves
  infeasible with `saphyr`, stop when the fallback would be broader than the
  containing rule object. Do not silently degrade to location-free diagnostics
  for every structural error.
- Iterations: if the same failing lint/test loop is attempted five times
  without a clear path forward, stop and escalate.

## Risks

- Risk: Semgrep rule `languages` accepts aliases and values outside the current
  executable `sempai_core::Language` enum (`py`, `ts`, `terraform`, `generic`,
  `none`). Severity: high. Likelihood: high. Mitigation: define a parser-local
  rule-language model instead of reusing `sempai_core::Language`; defer runtime
  support gating to 4.1.4 and 4.1.5.

- Risk: `serde-saphyr` shape errors may not carry enough location data on their
  own. Severity: high. Likelihood: medium. Mitigation: parse with `saphyr`
  first, retain the raw located YAML tree, and build a small source-locator for
  top-level rules and known keys so `DiagnosticReport` can still emit useful
  `primary_span` values.

- Risk: The Semgrep rule schema deliberately sets `additionalProperties: true`
  on the rule object, while this milestone only models a subset of keys.
  Severity: medium. Likelihood: high. Mitigation: accept and preserve or ignore
  unknown rule-level keys without parse failure, while still rejecting invalid
  shapes for the modeled keys.

- Risk: `Engine::compile_yaml` cannot produce real `QueryPlan` values until the
  normalization layer exists. Severity: medium. Likelihood: high. Mitigation:
  wire the engine through the parser so malformed YAML yields real parser
  diagnostics, and return a deliberate post-parse placeholder diagnostic only
  after successful parsing if normalization is still pending.

- Risk: Strict workspace lints will also apply to behaviour tests and fixture
  helpers. Severity: medium. Likelihood: medium. Mitigation: keep test worlds
  and helper signatures small, prefer helper structs over long parameter lists,
  and use tightly scoped `#[expect(..., reason = "...")]` only when
  structurally necessary.

## Progress

- [x] (2026-03-21 UTC) Reviewed roadmap item 4.1.3, the Sempai design document,
  the local parser-aligned Semgrep schema, adjacent 4.1.1 and 4.1.2
  ExecPlans, and the testing/documentation guidance requested in the task.
- [x] (2026-03-21 UTC) Drafted this ExecPlan.
- [x] (2026-03-22 UTC) Stage A: Scaffold `sempai_yaml`, dependencies, and
  red-path tests.
- [x] (2026-03-22 UTC) Stage B: Implement schema-aligned YAML rule models and
  parsing entrypoint.
- [x] (2026-03-22 UTC) Stage C: Implement structured parser diagnostics with
  source spans.
- [x] (2026-03-22 UTC) Stage D: Wire `sempai::Engine::compile_yaml` to the
  parser boundary.
- [x] (2026-03-22 UTC) Stage E: Update design docs, users guide, and roadmap
  state.
- [x] (2026-03-22 UTC) Stage F: Run all required quality gates and capture
  evidence.

## Surprises & Discoveries

- Observation: the workspace currently contains only `crates/sempai-core` and
  `crates/sempai`; the planned `crates/sempai-yaml` crate does not yet exist.
  Impact: 4.1.3 should establish that crate boundary now rather than pushing
  YAML parsing into a temporary location that would need to be undone later.

- Observation: `sempai::Engine::compile_yaml` currently returns
  `DiagnosticReport::not_implemented("compile_yaml")` for every input. Impact:
  this milestone should at minimum distinguish malformed YAML from the generic
  stub path, even if successful parse results still await normalization.

- Observation: the 4.1.2 work already established
  `DiagnosticReport::parser_error` and the `primary_span` JSON contract in
  [crates/sempai-core/src/diagnostic.rs](../../crates/sempai-core/src/diagnostic.rs).
  Impact: 4.1.3 should reuse that contract rather than inventing a
  parser-local error shape.

- Observation: the repository already carries a local parser-aligned Semgrep
  schema at
  [docs/semgrep-language-reference/semgrep-rule-schema.yaml](../semgrep-language-reference/semgrep-rule-schema.yaml).
  Impact: the implementation can lock its model and test fixtures to that
  local source of truth instead of reverse-engineering the shape ad hoc.

- Observation: `serde-saphyr` already reports byte-oriented error spans when
  parsing from `&str`, so `sempai_yaml` can use those locations directly for
  malformed YAML and Serde shape failures while keeping `saphyr` as a coarse
  fallback for rule-object spans. Impact: parser diagnostics retain useful
  `primary_span` data without needing a bespoke event-stream mapper in 4.1.3.

## Decision Log

- Decision: implement YAML parsing in a dedicated `sempai_yaml` crate and keep
  `sempai` as a facade over that parser. Rationale: this matches the design
  document's crate layout, keeps parsing isolated from normalization/execution,
  and prevents `sempai_core` from becoming a catch-all crate. Date/Author:
  2026-03-21 / Codex.

- Decision: use parser-local models for rule metadata values whose accepted
  schema is wider than the current executable engine surface, especially
  `languages`, `severity`, and `mode`. Rationale: reusing `sempai_core`
  execution enums here would either reject valid Semgrep forms too early or
  force premature runtime commitments. Date/Author: 2026-03-21 / Codex.

- Decision: treat structural schema failures as parser-time
  `E_SEMPAI_SCHEMA_INVALID` diagnostics and reserve semantic rule checks such
  as unsupported modes or invalid logical composition for later milestones.
  Rationale: this keeps 4.1.3 focused on the parsing boundary that the roadmap
  explicitly calls out. Date/Author: 2026-03-21 / Codex.

- Decision: update `Engine::compile_yaml` to surface real parse diagnostics now,
  even if successful compilation still cannot yield final `QueryPlan` values.
  Rationale: this produces immediate user-visible value without skipping the
  later normalization milestone. Date/Author: 2026-03-21 / Codex.

- Decision: keep search-mode principals strongly typed in `sempai_yaml`, but
  preserve join and taint bodies as opaque `serde_json::Value` payloads for
  now. Rationale: 4.1.3 needs schema-aligned parsing without prematurely
  committing 4.1.4/4.1.5 semantic models for mode-specific execution.
  Date/Author: 2026-03-22 / Codex.

## Outcomes & Retrospective

Target outcome at completion:

1. `crates/sempai-yaml` exists and exposes a documented public parser API for
   Semgrep-compatible YAML rule files.
1. Rule metadata and query principals parse from supported Semgrep-compatible
   YAML forms, covering both legacy and v2 query entrypoints.
1. Malformed YAML and structural schema errors emit `DiagnosticReport` values
   with stable `E_SEMPAI_*` codes and useful `primary_span` locations.
1. Unit tests and `rstest-bdd` v0.5.0 scenarios cover happy paths, unhappy
   paths, and edge cases.
1. `docs/sempai-query-language-design.md` records the design decisions taken
   during implementation.
1. `docs/users-guide.md` explains any changed user-visible behaviour,
   especially `compile_yaml` failure semantics.
1. `docs/roadmap.md` marks 4.1.3 done.
1. `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
   `make lint`, and `make test` all pass.

Retrospective notes:

1. `serde-saphyr::Spanned<T>` was useful for raw deserialization, but keeping
   it out of the public `sempai_yaml` API avoided leaking parser dependency
   types into the stable model.
1. The strongest user-visible value in this milestone came from replacing the
   blanket `NOT_IMPLEMENTED` YAML path with real parser/schema diagnostics,
   even though query-plan normalization is still pending.
1. Final verification passed with `make fmt`, `make markdownlint`,
   `make nixie`, `make check-fmt`, `make lint`, and `make test`.

## Context and orientation

The current Sempai surface is split across two crates:

- `crates/sempai-core/` contains the stable data model and diagnostics.
- `crates/sempai/` is the facade crate that exports `Engine` and `QueryPlan`.

Relevant current files:

- [Cargo.toml](../../Cargo.toml)
- [crates/sempai/src/engine.rs](../../crates/sempai/src/engine.rs)
- [crates/sempai/src/tests/engine_tests.rs](../../crates/sempai/src/tests/engine_tests.rs)
- [crates/sempai/src/tests/behaviour.rs](../../crates/sempai/src/tests/behaviour.rs)
- [crates/sempai/tests/features/sempai_engine.feature](../../crates/sempai/tests/features/sempai_engine.feature)
- [crates/sempai-core/src/diagnostic.rs](../../crates/sempai-core/src/diagnostic.rs)
- [docs/sempai-query-language-design.md](../sempai-query-language-design.md)
- [docs/semgrep-language-reference/semgrep-rule-schema.yaml](../semgrep-language-reference/semgrep-rule-schema.yaml)
- [docs/users-guide.md](../users-guide.md)
- [docs/roadmap.md](../roadmap.md)

The design document's planned crate map adds `crates/sempai-yaml` as the YAML
front-end. This milestone should create that crate and keep the public surface
small and explicit. A likely file layout is:

- `crates/sempai-yaml/Cargo.toml`
- `crates/sempai-yaml/src/lib.rs`
- `crates/sempai-yaml/src/model.rs`
- `crates/sempai-yaml/src/parser.rs`
- `crates/sempai-yaml/src/source_map.rs`
- `crates/sempai-yaml/src/tests/...`
- `crates/sempai-yaml/tests/features/sempai_yaml.feature`

The parser API should accept YAML text and optionally a source URI, then return
either a typed rule-file model or `DiagnosticReport`. The model should cover,
at minimum:

- Top-level `rules: [...]`.
- Rule metadata: `id`, `message`, `languages`, `severity`, `mode`.
- Query principal:
  - Legacy: `pattern`, `pattern-regex`, `patterns`, `pattern-either`.
  - v2: `match` as string or structured object.
- Pass-through compatibility for rule-level unknown keys that must not break
  parsing merely because later milestones have not modeled them yet.

This milestone does not need to produce a canonical `Formula`; that is 4.1.5.
It does, however, need to preserve enough structure that 4.1.4 and 4.1.5 can
consume the parsed output without reparsing YAML.

## Plan of work

### Stage A: Scaffold the crate and lock the contract with failing tests

Create `crates/sempai-yaml` and wire it into the workspace with the new parser
dependencies. Before implementing the parser, write tests that define the
observable contract:

- Unit tests for:
  - A minimal search-mode rule using `pattern`.
  - A legacy rule using `patterns`.
  - A legacy rule using `pattern-either`.
  - A v2 rule using `match`.
  - Invalid YAML syntax.
  - Missing required top-level `rules`.
  - Invalid metadata shapes such as non-array `languages` or invalid
    `severity`.
  - Acceptance of unknown rule-level keys without failure.
- Behaviour scenarios using `rstest-bdd` v0.5.0 for:
  - Happy path parsing of legacy rules.
  - Happy path parsing of v2 `match` rules.
  - Unhappy path syntax errors with span-bearing diagnostics.
  - Unhappy path schema-shape failures with stable codes/messages.
  - Edge path multi-rule files and unknown extra keys.
- Facade tests in `crates/sempai/` that assert `compile_yaml` now returns real
  parser diagnostics for malformed YAML.

Go/no-go:

- Do not proceed until at least one new parser-focused test fails against the
  current codebase.

### Stage B: Build schema-aligned rule models and parser entrypoints

Implement the public `sempai_yaml` API and its internal rule models. The parser
should:

- Parse YAML text with `saphyr`.
- Deserialize the parsed YAML tree through `serde-saphyr`.
- Expose typed structs/enums for the supported metadata and query-principal
  shapes.
- Separate parser-local rule metadata from runtime execution types where the
  accepted Semgrep schema is wider than current execution support.
- Preserve or ignore additional rule-level properties without failing the
  parse, in line with the schema's `additionalProperties: true`.

Likely public entrypoints:

- `parse_rule_file(...) -> Result<RuleFile, DiagnosticReport>`
- accessors on `RuleFile`, `Rule`, and the principal enum sufficient for later
  normalization work

Go/no-go:

- Do not proceed until all `sempai_yaml` unit tests for successful parsing pass.

### Stage C: Add structured diagnostics and source-location mapping

Implement the error path for both YAML syntax failures and structural
schema-shape failures:

- Map raw YAML syntax failures to `E_SEMPAI_YAML_PARSE`.
- Map structural shape or missing-required-field failures to
  `E_SEMPAI_SCHEMA_INVALID`.
- Reuse `DiagnosticReport::parser_error(...)` and the existing
  `primary_span` schema from `sempai_core`.
- Build a small YAML source locator from the `saphyr` parse tree so
  diagnostics can point to the top-level rule, offending key, or parser
  location instead of always returning `null`.

Go/no-go:

- Do not proceed until unhappy-path unit tests and BDD scenarios pass with
  stable diagnostics.

### Stage D: Wire the facade engine to the parser boundary

Update `sempai::Engine::compile_yaml` so that it no longer short-circuits every
input to `NOT_IMPLEMENTED`:

- Malformed YAML should return the real parser/schema `DiagnosticReport`.
- Successfully parsed rules should take the narrowest honest next step:
  - if a temporary internal representation is enough to construct a placeholder
    `QueryPlan`, do so only if the result remains semantically truthful, or
  - otherwise return a deliberate post-parse placeholder diagnostic that makes
    clear parsing succeeded but normalization/execution is not implemented yet.

This stage must also update unit and behaviour tests in `crates/sempai/` and
refresh the users guide so the user-facing contract matches reality.

Go/no-go:

- Do not proceed until `cargo test -p sempai --all-targets --all-features`
  passes.

### Stage E: Synchronize design docs, users guide, roadmap, and quality gates

Update documentation after the parser behaviour is stable:

- `docs/sempai-query-language-design.md`
  - record the crate boundary, parser-local type decisions, and diagnostic
    mapping rules.
- `docs/users-guide.md`
  - explain the current `compile_yaml` behaviour and what errors users should
    expect from malformed or structurally invalid YAML.
- `docs/roadmap.md`
  - mark 4.1.3 done only after all tests and gates pass.

Then run the full gate sequence with `tee` logs.

Go/no-go:

- Do not finalize until all documentation and workspace gates pass.

## Concrete steps

Run from repository root (`/home/user/project`).

1. Confirm the current baseline before edits.

```plaintext
cargo test -p sempai --all-targets --all-features
cargo test -p sempai_core --all-targets --all-features
```

Expected: current baseline passes and confirms the starting point before new
parser tests are added.

1. Create `crates/sempai-yaml/`, add workspace membership/dependencies, and
   write red-path unit and BDD tests.

```plaintext
cargo test -p sempai_yaml --all-targets --all-features
```

Expected: this initially fails because the crate or parser implementation is
not yet complete.

1. Implement the parser models, entrypoint, and diagnostic mapping.

```plaintext
cargo test -p sempai_yaml --all-targets --all-features
```

Expected: `sempai_yaml` tests pass for both successful parses and structured
failure diagnostics.

1. Wire `sempai::Engine::compile_yaml` to the parser boundary and update facade
   tests.

```plaintext
cargo test -p sempai --all-targets --all-features
```

Expected: malformed YAML now returns parser/schema diagnostics instead of
generic `NOT_IMPLEMENTED`.

1. Update docs and roadmap, then run the full gate sequence with captured logs.

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-3-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-3-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-3-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-3-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-3-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-3-make-test.log
```

Expected transcript endings:

```plaintext
... Finished `dev` profile ...
... test result: ok. <N> passed; 0 failed ...
```

## Validation and acceptance

Acceptance is satisfied when all of the following are true:

- A Semgrep-compatible YAML document with `rules: [...]` parses into a typed
  rule-file model.
- Rule metadata (`id`, `message`, `languages`, `severity`, `mode`) parses from
  supported YAML forms.
- Query principals parse for:
  - `pattern`
  - `pattern-regex`
  - `patterns`
  - `pattern-either`
  - `match`
- Invalid YAML syntax emits `E_SEMPAI_YAML_PARSE` with a useful
  `primary_span`.
- Structural schema failures emit `E_SEMPAI_SCHEMA_INVALID` with stable
  messages and a non-trivial `primary_span` when the offending location can be
  identified.
- Unit tests and `rstest-bdd` behaviour tests cover happy, unhappy, and edge
  paths for the new parser crate and the `sempai` facade behaviour.
- `docs/sempai-query-language-design.md` records the parser decisions taken.
- `docs/users-guide.md` reflects any changed user-visible behaviour.
- `docs/roadmap.md` marks 4.1.3 done.
- `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
  `make lint`, and `make test` all succeed.

## Approval record

Approved and implemented. Implementation completed 2026-03-22 UTC; see the
Progress section and Outcomes & Retrospective above.
