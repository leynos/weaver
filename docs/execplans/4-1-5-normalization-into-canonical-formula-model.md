# 4.1.5 Implement legacy and v2 normalization into canonical Formula model

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS

## Purpose / big picture

After this change, the Sempai compilation pipeline will lower both legacy
Semgrep operators (`pattern`, `patterns`, `pattern-either`, `pattern-not`,
`pattern-inside`, `pattern-not-inside`, `pattern-not-regex`,
`semgrep-internal-pattern-anywhere`) and v2 `match` operators (`pattern`,
`regex`, `all`, `any`, `not`, `inside`, `anywhere`) into one canonical
`Formula` enum defined in `sempai_core`. Two parser-enforced semantic
constraint checks will reject invalid formula shapes deterministically:

- `E_SEMPAI_INVALID_NOT_IN_OR`: a negated branch inside `pattern-either` or
  `any`.
- `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`: a conjunction (`patterns` or `all`)
  with no positive match-producing term (except in metavariable-pattern
  contexts, which are preserved as opaque constraints for now).

Observable user-facing behaviour after implementation:

- `Engine::compile_yaml(...)` accepts valid `search` rules and produces
  `Vec<QueryPlan>` containing a normalised `Formula` for each rule, instead of
  returning `NOT_IMPLEMENTED`.
- Paired legacy and v2 YAML fixtures that express equivalent queries normalise
  to structurally identical `Formula` values.
- Semantically invalid rules (negation in disjunction, missing positive term in
  conjunction) emit deterministic rule diagnostics with stable error codes and
  source spans pointing at the offending formula node.
- Legacy `LegacyClause::Constraint` objects are preserved as opaque
  `Formula::Constraint` nodes rather than dropped or rejected, so constraint
  evaluation can be added in a later milestone.
- v2 `Decorated` formulas have their `where`, `as`, and `fix` metadata carried
  through to the canonical `Decorated<Formula>` wrapper.
- `Engine::execute(...)` continues to return `NOT_IMPLEMENTED` until backend
  work lands in 4.2.x.

Observable completion evidence:

```plaintext
set -o pipefail; cargo test -p sempai_core 2>&1 | tee /tmp/4-1-5-sempai-core-test.log
set -o pipefail; cargo test -p sempai 2>&1 | tee /tmp/4-1-5-sempai-test.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-5-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-5-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-5-make-test.log
```

Because this milestone updates Markdown documentation and the roadmap:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-5-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-5-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-5-make-nixie.log
```

## Constraints

- Follow the normalised formula model from
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md)
  section "Normalised formula model" and the operator mappings in
  [docs/semgrep-language-reference/semgrep-operator-precedence.md](../semgrep-language-reference/semgrep-operator-precedence.md)
  and
  [docs/semgrep-language-reference/semgrep-legacy-vs-v2-guidance.md](../semgrep-language-reference/semgrep-legacy-vs-v2-guidance.md).
- The canonical `Formula` enum and its `Decorated<T>` wrapper live in
  `sempai_core`, not in `sempai_yaml` or `sempai`. This keeps the formula model
  available to all downstream crates including the future `sempai_ts` backend.
- Normalization must be a pure function from `SearchQueryPrincipal` to
  `Result<Formula, DiagnosticReport>`, with no side-effects and no I/O.
- Preserve the existing parser-vs-engine validation boundary: `sempai_yaml`
  owns YAML shape and structural validation; the normalization pass in
  `sempai_core` owns semantic constraint checks on the canonical formula.
- Parser-level types (`LegacyFormula`, `MatchFormula`, `LegacyValue`,
  `LegacyClause`) remain in `sempai_yaml`. Do not remove or restructure them.
- Semantic constraint checks must use the existing diagnostic codes
  `ESempaiInvalidNotInOr` and `ESempaiMissingPositiveTermInAnd` from
  `sempai_core::DiagnosticCode`.
- Legacy `LegacyClause::Constraint` objects must be preserved as opaque nodes
  (`Formula::Constraint(serde_json::Value)`) so metavariable-regex,
  metavariable-pattern, and similar constraints can be evaluated in a later
  milestone.
- The `QueryPlan` struct in `sempai/src/engine.rs` must carry a real `Formula`
  instead of the current `_plan: ()` placeholder.
- `ProjectDependsOn` search principals must be preserved as-is and skip
  normalization, since they have no formula semantics.
- Tests must include unit tests and BDD tests using `rstest-bdd` v0.5.0,
  covering happy paths, unhappy paths (semantic constraint violations), and
  edge cases (empty conjunctions, single-element disjunctions, deeply nested
  formulas, decorated v2 nodes).
- Keep files below 400 lines. Split the normalization module, semantic
  validator, and test helpers into focused modules.
- Every new module must begin with a `//!` module comment. New public items must
  carry Rustdoc with examples where appropriate.
- Use en-GB-oxendict spelling and grammar throughout code, comments, and
  documentation.
- Record any design decisions in
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md).
- Update [docs/users-guide.md](../users-guide.md) with the user-visible change
  in `compile_yaml(...)` behaviour.
- Mark roadmap item 4.1.5 done in [docs/roadmap.md](../roadmap.md) only after
  all tests and quality gates pass.
- `make check-fmt`, `make lint`, and `make test` must all succeed.

## Tolerances (exception triggers)

- Scope: if implementation requires touching more than 18 net files outside
  `crates/sempai-core/`, `crates/sempai/`, `crates/sempai-yaml/`, and the four
  required docs, stop and escalate.
- Interface: if normalisation requires a breaking change to
  `sempai_yaml::SearchQueryPrincipal`, `sempai_yaml::LegacyFormula`,
  `sempai_yaml::MatchFormula`, or `sempai::Engine::compile_yaml`, stop and
  present the least disruptive options before proceeding.
- Dependency: if the canonical `Formula` requires a new workspace dependency
  beyond `serde_json` (already available), stop and escalate.
- Iterations: if the same failing lint or test loop is attempted five times
  without a clear path forward, stop and escalate.
- Ambiguity: if the Semgrep schema and the design document disagree on whether
  `pattern-not-inside` normalises to `Not(Inside(...))` versus a distinct
  `NotInside(...)` variant, stop and escalate with competing interpretations.

## Risks

- Risk: the existing `LegacyValue` type wraps either a string or a nested
  formula, but the canonical `Formula` expects atoms. Normalization must
  handle both forms. Severity: medium. Likelihood: high. Mitigation: treat
  `LegacyValue::String(s)` as `Formula::Atom(Atom::Pattern(s))` during
  lowering.

- Risk: `MatchFormula::Pattern(s)` and `MatchFormula::PatternObject(s)` are
  semantically identical (both are pattern atoms) but structurally different.
  The normaliser must unify them. Severity: low. Likelihood: certain.
  Mitigation: map both to `Formula::Atom(Atom::Pattern(s))`.

- Risk: the `MissingPositiveTermInAnd` check must not reject conjunctions
  where the only positive term comes from a metavariable-pattern constraint.
  The Semgrep spec allows this as a special case. Severity: medium. Likelihood:
  medium. Mitigation: treat `LegacyClause::Constraint` as a non-positive,
  non-negative term for the purpose of the positive-term check, and add a
  fixture specifically testing the metavariable-pattern exception.

- Risk: `Decorated` v2 formulas nest decorators around formulas. Normalization
  must preserve decoration while lowering the inner formula. Severity: medium.
  Likelihood: high. Mitigation: normalise the inner formula recursively, then
  wrap in `Decorated<Formula>` carrying the `where`, `as`, and `fix` metadata.

- Risk: the positive-term check interacts with `Inside` and `Anywhere`
  semantics. Per the design doc, these are constraints, not match-producers, so
  they do not count as positive terms. Severity: medium. Likelihood: medium.
  Mitigation: follow the design doc classification precisely and add explicit
  test fixtures for `all: [inside(...)]` (should fail) and
  `all: [pattern(...), inside(...)]` (should pass).

## Progress

- [ ] Reviewed roadmap item 4.1.5, the Sempai design doc, the Semgrep operator
      precedence and legacy-vs-v2 guidance, the current `sempai_yaml` model, and
      the 4.1.4 ExecPlan.
- [ ] Drafted this ExecPlan.

## Surprises & Discoveries

(To be filled in during implementation.)

## Decision Log

(To be filled in during implementation.)

## Outcomes & Retrospective

Target outcome at completion:

1. `sempai_core` exports a canonical `Formula` enum, `Atom` enum, and
   `Decorated<T>` wrapper as stable public types.
2. `sempai_core` exports a `normalise_search_principal(...)` function that
   lowers `SearchQueryPrincipal` to `Formula`.
3. `sempai_core` exports a `validate_formula_constraints(...)` function that
   checks semantic constraints and returns `DiagnosticReport` on violation.
4. Paired legacy and v2 test fixtures produce structurally equal `Formula`
   values after normalization.
5. Invalid formula shapes (`Not` in `Or`, missing positive term in `And`) emit
   deterministic diagnostics with the correct `E_SEMPAI_*` codes.
6. `Engine::compile_yaml(...)` returns `Ok(Vec<QueryPlan>)` for valid search
   rules, with each plan carrying a normalised `Formula`.
7. `Engine::execute(...)` still returns `NOT_IMPLEMENTED` until 4.2.x.
8. `docs/sempai-query-language-design.md` records the normalization mapping and
   any implementation decisions.
9. `docs/users-guide.md` documents the change in `compile_yaml(...)` behaviour.
10. `docs/roadmap.md` marks 4.1.5 done.
11. `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
    `make lint`, and `make test` all pass.

Retrospective notes: (to be filled in after completion.)

## Context and orientation

Current files that matter for this milestone:

- [crates/sempai-core/src/lib.rs](../../crates/sempai-core/src/lib.rs) —
  re-exports; will expose the new formula module.
- `crates/sempai-core/src/diagnostic.rs` — already defines
  `ESempaiInvalidNotInOr` and `ESempaiMissingPositiveTermInAnd`.
- [crates/sempai-yaml/src/model.rs](../../crates/sempai-yaml/src/model.rs) —
  `SearchQueryPrincipal`, `LegacyFormula`, `MatchFormula`, `LegacyClause`,
  `LegacyValue`, `MatchFormula::Decorated`.
- [crates/sempai/src/engine.rs](../../crates/sempai/src/engine.rs) — `Engine`,
  `QueryPlan`, and the `NOT_IMPLEMENTED` placeholder at line 116.
- `crates/sempai/src/mode_validation.rs` — mode-aware gating
  from 4.1.4.
- `crates/sempai/src/tests/engine_tests.rs` — existing engine
  test coverage.
- `crates/sempai/tests/features/sempai_engine.feature` —
  existing BDD scenarios for the engine.
- `crates/sempai/src/tests/behaviour.rs` — BDD step
  definitions for the engine.
- `docs/sempai-query-language-design.md` — design reference,
  records implementation decisions.
- `docs/semgrep-language-reference/semgrep-operator-precedence.md`
  — operator mapping and precedence ladder.
- `docs/semgrep-language-reference/semgrep-legacy-vs-v2-guidance.md`
  — legacy-to-v2 equivalence table.
- [docs/users-guide.md](../users-guide.md) — user-facing documentation.
- [docs/roadmap.md](../roadmap.md) — roadmap state tracking.

Current behaviour to preserve or intentionally change:

- `parse_rule_file(...)` produces `RuleFile` with `Rule` and
  `SearchQueryPrincipal` for search-mode rules. This remains unchanged.
- `validate_supported_modes(...)` rejects non-search modes with
  `E_SEMPAI_UNSUPPORTED_MODE`. This remains unchanged.
- `compile_yaml(...)` currently returns `NOT_IMPLEMENTED` for all valid search
  rules after mode validation. This will be replaced with normalization and
  semantic validation, producing real `QueryPlan` values on success.
- `QueryPlan._plan` is currently `()`. This will be replaced with `Formula`.

## Plan of work

### Stage A: Define the canonical Formula model in sempai_core

Add the `Formula`, `Atom`, and `Decorated<T>` types to `sempai_core` with full
Rustdoc documentation. These types follow the design document's normalised
formula model.

### Stage B: Implement normalization functions

Implement `normalise_legacy(...)` and `normalise_match(...)` functions that
lower `LegacyFormula` and `MatchFormula` respectively into `Formula`. Add a
top-level `normalise_search_principal(...)` dispatcher. These are pure
functions in `sempai_core` that depend on `sempai_yaml` model types.

### Stage C: Implement semantic constraint checks

Add `validate_formula_constraints(...)` that walks the normalised `Formula`
tree and checks:

- No `Not` children inside `Or` nodes (`InvalidNotInOr`).
- Every `And` node has at least one positive term (`MissingPositiveTermInAnd`).

### Stage D: Lock expected behaviour with tests first

Add unit tests and BDD scenarios before wiring the normalization into the
engine, so the intended behaviour is explicit and verifiable.

### Stage E: Wire normalization into Engine::compile_yaml

Replace the `NOT_IMPLEMENTED` placeholder in `Engine::compile_yaml(...)` with
the normalization and validation pipeline. Update `QueryPlan` to carry a real
`Formula`.

### Stage F: Update design docs, user docs, and roadmap

Synchronise living documentation with the implementation.

### Stage G: Run the full gate sequence and capture evidence

Run targeted crate tests and the full repository gates.

## Concrete steps

### Step 1: Define canonical Formula types (Stage A)

Create `crates/sempai-core/src/formula.rs`:

```rust
/// Canonical normalised formula shared by legacy and v2 paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formula {
    Atom(Atom),
    Not(Box<Decorated<Formula>>),
    Inside(Box<Decorated<Formula>>),
    Anywhere(Box<Decorated<Formula>>),
    And(Vec<Decorated<Formula>>),
    Or(Vec<Decorated<Formula>>),
    /// Opaque constraint preserved for later evaluation.
    Constraint(serde_json::Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    Pattern(String),
    Regex(String),
    TreeSitterQuery(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decorated<T> {
    pub node: T,
    pub where_clauses: Vec<serde_json::Value>,
    pub as_name: Option<String>,
    pub fix: Option<String>,
}
```

Export from `sempai_core/src/lib.rs`. Keep file under 400 lines.

### Step 2: Implement normalization module (Stages B and C)

Create `crates/sempai-core/src/normalise.rs` (or split across
`normalise/mod.rs`, `normalise/legacy.rs`, `normalise/v2.rs` if needed to
stay within the 400-line limit).

Key mappings:

**Legacy to Formula:**

| Legacy | Formula |
| --- | --- |
| `Pattern(s)` | `Atom(Pattern(s))` |
| `PatternRegex(s)` | `Atom(Regex(s))` |
| `Patterns(clauses)` | `And(normalised_clauses)` |
| `PatternEither(branches)` | `Or(normalised_branches)` |
| `PatternNot(val)` | `Not(normalise_value(val))` |
| `PatternInside(val)` | `Inside(normalise_value(val))` |
| `PatternNotInside(val)` | `Not(Inside(normalise_value(val)))` |
| `PatternNotRegex(s)` | `Not(Atom(Regex(s)))` |
| `Anywhere(val)` | `Anywhere(normalise_value(val))` |
| `LegacyValue::String(s)` | `Atom(Pattern(s))` |
| `LegacyValue::Formula(f)` | `normalise_legacy(f)` |
| `LegacyClause::Constraint(v)` | `Constraint(v)` |

**v2 Match to Formula:**

| v2 Match | Formula |
| --- | --- |
| `Pattern(s)` | `Atom(Pattern(s))` |
| `PatternObject(s)` | `Atom(Pattern(s))` |
| `Regex(s)` | `Atom(Regex(s))` |
| `All(items)` | `And(normalised_items)` |
| `Any(items)` | `Or(normalised_items)` |
| `Not(inner)` | `Not(normalise_match(inner))` |
| `Inside(inner)` | `Inside(normalise_match(inner))` |
| `Anywhere(inner)` | `Anywhere(normalise_match(inner))` |
| `Decorated { formula, where_, as_, fix }` | `Decorated { normalise_match(formula), where_, as_, fix }` |

Implement `validate_formula_constraints(formula: &Formula)`:

- Walk the tree recursively.
- For `Or(branches)`: check that no direct child (after unwrapping
  `Decorated`) is a `Not(...)`. If found, emit
  `E_SEMPAI_INVALID_NOT_IN_OR`.
- For `And(terms)`: check that at least one direct child is a positive term
  (i.e., not `Not`, not `Inside`, not `Anywhere`, not `Constraint`). If none
  found, emit `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`.

### Step 3: Add unit tests for normalization (Stage D)

Create `crates/sempai-core/src/tests/normalise_tests.rs` with `rstest`
parameterised cases:

- Legacy `pattern` → `Atom(Pattern(...))`.
- Legacy `pattern-regex` → `Atom(Regex(...))`.
- Legacy `patterns` with mixed positive and negative terms →
  `And([Atom(...), Not(...)])`.
- Legacy `pattern-either` with two branches →
  `Or([Atom(...), Atom(...)])`.
- Legacy `pattern-not-inside` → `Not(Inside(Atom(...)))`.
- Legacy `pattern-not-regex` → `Not(Atom(Regex(...)))`.
- Legacy `semgrep-internal-pattern-anywhere` → `Anywhere(Atom(...))`.
- v2 `match: "foo"` → `Atom(Pattern("foo"))`.
- v2 `all` with `not` and `pattern` → `And([Atom(...), Not(...)])`.
- v2 `any` with two patterns → `Or([Atom(...), Atom(...)])`.
- v2 decorated formula carries `where`, `as`, `fix` metadata.
- Paired equivalence: legacy `patterns` vs v2 `all` produce the same
  `Formula`.
- Paired equivalence: legacy `pattern-either` vs v2 `any` produce the same
  `Formula`.
- Deep nesting: `pattern-either` containing `patterns` containing
  `pattern-not`.

### Step 4: Add unit tests for semantic constraints (Stage D)

Create `crates/sempai-core/src/tests/constraint_tests.rs`:

- `Or` with `Not` child → `E_SEMPAI_INVALID_NOT_IN_OR`.
- `Or` without `Not` children → passes.
- `And` with no positive terms → `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`.
- `And` with one positive and one negative → passes.
- `And` with only `Inside` and `Anywhere` → fails (not positive).
- `And` with only `Constraint` objects → passes (metavariable-pattern
  exception: constraints alone do not trigger the positive-term violation,
  because they may be metavariable-pattern constraints that act as implicit
  positive terms).
- Nested: `And` inside `Or` where the `And` has no positive term → fails.

### Step 5: Add BDD scenarios (Stage D)

Add feature file
`crates/sempai/tests/features/formula_normalization.feature` (or extend the
existing `sempai_engine.feature`) with scenarios:

```gherkin
Feature: Formula normalization

  Scenario: Legacy pattern normalises to atom
    Given a valid search rule with a legacy "pattern" principal
    When the rule is compiled
    Then the query plan contains a pattern atom

  Scenario: v2 match string normalises to atom
    Given a valid search rule with a v2 match string principal
    When the rule is compiled
    Then the query plan contains a pattern atom

  Scenario: Legacy and v2 conjunction produce equivalent formulas
    Given a legacy "patterns" rule and an equivalent v2 "all" rule
    When both rules are compiled
    Then both query plans contain structurally equal formulas

  Scenario: Negation inside disjunction is rejected
    Given a search rule with a "not" inside "pattern-either"
    When the rule is compiled
    Then compilation fails with E_SEMPAI_INVALID_NOT_IN_OR

  Scenario: Conjunction without positive term is rejected
    Given a search rule with "patterns" containing only "pattern-not"
    When the rule is compiled
    Then compilation fails with E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND
```

Add corresponding step definitions in
`crates/sempai/src/tests/behaviour.rs` (or a new
`crates/sempai/src/tests/normalise_behaviour.rs` if the file exceeds 400
lines).

### Step 6: Wire normalization into Engine (Stage E)

Update `crates/sempai/src/engine.rs`:

- Import `normalise_search_principal` and `validate_formula_constraints` from
  `sempai_core`.
- In `compile_yaml(...)`:
  1. Parse YAML → `RuleFile`.
  2. Validate supported modes.
  3. For each search rule, extract its `SearchQueryPrincipal`.
  4. If `ProjectDependsOn`, skip normalization and produce a plan without a
     formula (or a sentinel marker).
  5. Call `normalise_search_principal(principal)` → `Formula`.
  6. Call `validate_formula_constraints(&formula)` → check for violations.
  7. Construct `QueryPlan` with the rule's ID, language, and `Formula`.
  8. Return `Ok(plans)`.

Update `QueryPlan`:

- Replace `_plan: ()` with `formula: Option<Formula>`.
- `None` represents a `ProjectDependsOn` rule with no formula.
- Add `pub fn formula(&self) -> Option<&Formula>` accessor.
- Remove the `#[cfg(test)]` gate on `QueryPlan::new`.

### Step 7: Ensure sempai_core depends on sempai_yaml types (Stage B)

The normalization functions in `sempai_core` accept `sempai_yaml` types
(`SearchQueryPrincipal`, `LegacyFormula`, `MatchFormula`). This creates a
dependency `sempai_core → sempai_yaml`. If this introduces a circular
dependency (since `sempai_yaml` depends on `sempai_core` for `SourceSpan` and
diagnostics), resolve it by one of:

- **Option A (preferred):** Keep normalization in `sempai_core` and accept the
  `sempai_yaml` dependency. This requires `sempai_yaml` to depend on
  `sempai_core` for diagnostics, and `sempai_core` to depend on `sempai_yaml`
  for model types. This is circular and **not viable**.
- **Option B:** Place normalization in the `sempai` facade crate, which already
  depends on both. Normalization functions live in `crates/sempai/src/normalise/`
  and convert from `sempai_yaml` types to `sempai_core::Formula`.
- **Option C:** Place normalization in a new `sempai_core::normalise` module
  but define the conversion traits such that the actual `From` impls live in
  `sempai` or `sempai_yaml`.

**Resolution:** Option B is the most pragmatic. The normalization module lives
in `crates/sempai/src/normalise/` and the `Formula` type itself lives in
`sempai_core`. This avoids circular dependencies and keeps the canonical type
in the core crate.

### Step 8: Update documentation (Stage F)

- Update
  [docs/sempai-query-language-design.md](../sempai-query-language-design.md):
  - Add an implementation note recording the normalization mapping.
  - Document the `pattern-not-inside` → `Not(Inside(...))` lowering.
  - Document the constraint-preservation strategy.
  - Record the crate-placement decision (Formula in `sempai_core`,
    normalization in `sempai`).
- Update [docs/users-guide.md](../users-guide.md):
  - Document that `compile_yaml(...)` now returns real query plans for valid
    search rules.
  - List the semantic constraint error codes users may encounter.
- Update [docs/roadmap.md](../roadmap.md):
  - Mark 4.1.5 done.

### Step 9: Run full gate sequence (Stage G)

```plaintext
set -o pipefail; cargo test -p sempai_core --all-targets --all-features 2>&1 | tee /tmp/4-1-5-sempai-core-test.log
set -o pipefail; cargo test -p sempai --all-targets --all-features 2>&1 | tee /tmp/4-1-5-sempai-test.log
set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-5-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-5-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-5-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-5-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-5-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-5-make-test.log
```

## Dependency graph

```plaintext
Step 1 (Formula types in sempai_core)
  ├─→ Step 2 (normalization functions in sempai)
  │     └─→ Step 3 (normalization unit tests)
  │           └─→ Step 5 (BDD scenarios)
  │                 └─→ Step 6 (wire into Engine)
  └─→ Step 4 (constraint unit tests)
        └─→ Step 5
              └─→ Step 6
                    └─→ Step 8 (docs)
                          └─→ Step 9 (gates)
```

Step 7 (dependency resolution) is performed during Step 2 and determines the
final module placement.

## Test plan

### Unit tests (sempai_core)

- Formula type construction and equality.
- `Decorated` wrapper with and without metadata.
- `Atom` variants.

### Unit tests (sempai — normalization)

- Legacy → Formula mapping for each `LegacyFormula` variant.
- v2 Match → Formula mapping for each `MatchFormula` variant.
- Paired equivalence tests (legacy ↔ v2).
- `LegacyValue::String` vs `LegacyValue::Formula` handling.
- `LegacyClause::Constraint` → `Formula::Constraint`.
- Decorated v2 formulas carry metadata.
- Deep nesting.

### Unit tests (sempai — semantic constraints)

- `InvalidNotInOr` positive and negative cases.
- `MissingPositiveTermInAnd` positive and negative cases.
- Nested constraint violations.
- `Inside`/`Anywhere` not counted as positive terms.
- `Constraint` alone in `And` does not trigger violation.

### BDD tests (sempai)

- Happy paths: legacy pattern, v2 match, paired equivalence.
- Unhappy paths: negation in disjunction, missing positive term.
- Edge cases: empty rules array, `ProjectDependsOn` passthrough.

### Integration (engine-level)

- `compile_yaml(...)` returns `Ok(Vec<QueryPlan>)` for valid rules.
- `compile_yaml(...)` returns `Err(DiagnosticReport)` with correct codes for
  invalid rules.
- Plans carry accessible `Formula` values.

## Validation and acceptance

- All tests in `cargo test -p sempai_core`, `cargo test -p sempai`, and
  `make test` pass.
- `make check-fmt`, `make lint`, `make markdownlint`, and `make nixie` pass.
- `docs/roadmap.md` shows 4.1.5 as `[x]`.
- `docs/users-guide.md` documents the new `compile_yaml(...)` behaviour.
- `docs/sempai-query-language-design.md` contains implementation notes.

## Idempotence and recovery

All steps are re-runnable. If the implementation hits a blocker:

- If the canonical `Formula` type needs to change, update `formula.rs` and
  re-run all normalization and constraint tests.
- If the crate placement causes circular dependencies, switch to Option B
  (normalization in `sempai`) per Step 7.
- If semantic constraint logic is incorrect, update `normalise.rs` and re-run
  the constraint test suite.

## Artefacts and notes

Expected new files:

- `crates/sempai-core/src/formula.rs` — canonical `Formula`, `Atom`,
  `Decorated<T>`.
- `crates/sempai/src/normalise.rs` (or `normalise/mod.rs` + sub-modules) —
  normalization functions.
- `crates/sempai/src/normalise/legacy.rs` — legacy lowering.
- `crates/sempai/src/normalise/v2.rs` — v2 lowering.
- `crates/sempai/src/normalise/constraints.rs` — semantic constraint checks.
- `crates/sempai-core/src/tests/formula_tests.rs` — formula type tests.
- `crates/sempai/src/tests/normalise_tests.rs` — normalization unit tests.
- `crates/sempai/src/tests/constraint_tests.rs` — constraint check unit tests.
- `crates/sempai/tests/features/formula_normalization.feature` — BDD
  scenarios.

Expected modified files:

- `crates/sempai-core/src/lib.rs` — new `formula` module export.
- `crates/sempai/src/engine.rs` — wire normalization, update `QueryPlan`.
- `crates/sempai/src/lib.rs` — re-export formula types if needed.
- `crates/sempai/src/tests/behaviour.rs` — new BDD step definitions.
- `docs/sempai-query-language-design.md` — implementation notes.
- `docs/users-guide.md` — user-visible behaviour change.
- `docs/roadmap.md` — mark 4.1.5 done.

## Interfaces and dependencies

### Public API additions (sempai_core)

```rust
// crates/sempai-core/src/formula.rs

pub enum Formula { ... }
pub enum Atom { ... }
pub struct Decorated<T> { ... }
```

### Public API additions (sempai)

```rust
// crates/sempai/src/normalise.rs (or mod.rs)

pub fn normalise_search_principal(
    principal: &SearchQueryPrincipal,
) -> Result<Formula, DiagnosticReport>;

pub fn validate_formula_constraints(
    formula: &Formula,
) -> Result<(), DiagnosticReport>;
```

### Modified API (sempai)

```rust
// crates/sempai/src/engine.rs

pub struct QueryPlan {
    rule_id: String,
    language: Language,
    formula: Option<Formula>,
}

impl QueryPlan {
    pub fn formula(&self) -> Option<&Formula>;
}
```

## Practice documentation

The following project guidance documents are relevant to this milestone.
Implementors should consult them for conventions, patterns, and constraints.

### Testing

- [docs/rstest-bdd-users-guide.md](../rstest-bdd-users-guide.md) — BDD
  framework usage: feature files, step definitions, fixture injection,
  `Slot<T>` state, `ScenarioState`, data tables, and `#[scenario]`
  binding. All behavioural tests in this milestone use `rstest-bdd`
  v0.5.0.
- [docs/rust-testing-with-rstest-fixtures.md](../rust-testing-with-rstest-fixtures.md)
  — `rstest` fixture injection, `#[case]` parameterised tests,
  `#[values]` combinatorial tests, `#[from]` and `#[with]` overrides.
  Unit tests in the normalization and constraint modules should use
  `rstest` parameterised cases for the mapping tables.
- [docs/reliable-testing-in-rust-via-dependency-injection.md](../reliable-testing-in-rust-via-dependency-injection.md)
  — dependency injection patterns for testable Rust code. Normalization
  functions are pure and need no DI, but the engine integration path
  should remain injectable for future backend substitution.
- [docs/rust-doctest-dry-guide.md](../rust-doctest-dry-guide.md) —
  DRY doctest patterns, `concat!()` for multi-line literals, hidden
  setup lines, and `no_run` annotations. All new public types and
  functions require Rustdoc examples.

### Design and architecture

- [docs/sempai-query-language-design.md](../sempai-query-language-design.md)
  — primary design reference for the normalised formula model, operator
  mappings, semantic constraints, and the parser-to-validation pipeline.
  Implementation decisions must be recorded here.
- [docs/weaver-design.md](../weaver-design.md) — system architecture,
  the observe/act/verify command model, JSONL protocol, and the
  Double-Lock safety harness. Relevant for understanding how query
  plans flow through the daemon.
- [docs/semgrep-language-reference/semgrep-operator-precedence.md](../semgrep-language-reference/semgrep-operator-precedence.md)
  — Semgrep operator precedence ladder and Pratt binding powers.
  Defines the normalization model that this milestone implements.
- [docs/semgrep-language-reference/semgrep-legacy-vs-v2-guidance.md](../semgrep-language-reference/semgrep-legacy-vs-v2-guidance.md)
  — legacy-to-v2 equivalence table. Paired test fixtures should
  mirror the equivalences documented here.

### Code quality

- [docs/complexity-antipatterns-and-refactoring-strategies.md](../complexity-antipatterns-and-refactoring-strategies.md)
  — Cyclomatic and Cognitive Complexity metrics, the "Bumpy Road"
  antipattern, extract-method and dispatcher-pattern refactoring
  strategies. Relevant for keeping the recursive normalization and
  constraint-walking functions simple and well-factored.
- [AGENTS.md](../../AGENTS.md) — code style, 400-line file limit,
  en-GB spelling, commit gating (`make check-fmt`, `make lint`,
  `make test`), module-level `//!` comments, and Rustdoc requirements.

### Configuration

- [docs/ortho-config-users-guide.md](../ortho-config-users-guide.md)
  — OrthoConfig usage for CLI/env/file configuration merging.
  Not directly exercised in normalization but relevant for
  understanding how `EngineConfig` reaches the engine.

## Revision note

Initial draft created on 2026-04-12. No revisions yet. Implements roadmap
item 4.1.5 from [docs/roadmap.md](../roadmap.md).
