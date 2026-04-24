# 4.1.5 Implement normalization into canonical Formula model

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

After this change, Sempai will normalize both legacy `pattern*` syntax and v2
`match` syntax into a single canonical `Formula` model defined in
`sempai_core`.  Semantic constraint checks will reject structurally invalid
formulas — specifically `InvalidNotInOr` (negated branches inside disjunction)
and `MissingPositiveTermInAnd` (conjunctions with no positive match-producing
term) — with deterministic diagnostics using the existing `E_SEMPAI_*` error
codes.

Observable user-facing behaviour after implementation:

- `sempai::Engine::compile_yaml` no longer returns `NOT_IMPLEMENTED` for valid
  search-mode rules.  Instead, it returns `Vec<QueryPlan>` containing the
  normalized canonical formula for each rule.
- Paired legacy and v2 rule fixtures that express the same logical query
  normalize to structurally equivalent `Formula` values.
- Semantically invalid formulas (negated terms in `pattern-either`/`any`,
  conjunctions without positive terms in `patterns`/`all`) emit deterministic
  `E_SEMPAI_INVALID_NOT_IN_OR` or `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`
  diagnostics with accurate `primary_span` locations.
- `r2c-internal-project-depends-on` rules continue to parse successfully
  without normalization failure (they produce a degenerate formula or are gated
  appropriately).

Observable completion evidence:

```plaintext
set -o pipefail; cargo test -p sempai_core 2>&1 | tee /tmp/4-1-5-sempai-core-test.log
set -o pipefail; cargo test -p sempai_yaml 2>&1 | tee /tmp/4-1-5-sempai-yaml-test.log
set -o pipefail; cargo test -p sempai 2>&1 | tee /tmp/4-1-5-sempai-test.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-5-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-5-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-5-make-test.log
```

Because this milestone updates Markdown documentation:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-5-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-5-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-5-make-nixie.log
```

## Constraints

- Align with the canonical formula model specified in
  `docs/sempai-query-language-design.md` §"Normalised formula model":
  `Formula::Atom`, `Formula::Not`, `Formula::Inside`, `Formula::Anywhere`,
  `Formula::And`, `Formula::Or`, with `Decorated<T>` wrapper carrying `where`,
  `as`, and `fix` metadata.
- Preserve the crate boundary split:
  - `sempai_core` owns the `Formula`, `Atom`, `Decorated`, and `WhereClause`
    types. `Decorated<T>` carries the fields `node: T`, `where_clauses:
    Vec<WhereClause>`, `as_name: Option<String>`, `fix: Option<String>`, and
    `span: Option<SourceSpan>`.
  - `sempai_yaml` owns the parsed `LegacyFormula` and `MatchFormula` types.
  - `sempai` (facade) wires parsing → normalization → validation → plan
    construction. Normalization lives in `crates/sempai/src/normalize.rs` and
    semantic validation in `crates/sempai/src/semantic_check.rs`.
- Maintain the stable diagnostics contract from 4.1.2:
  `code`, `message`, `primary_span`, and `notes` remain the only emitted fields.
- Use `DiagnosticReport::validation_error(...)` for semantic constraint
  violations.
- Enforce `InvalidNotInOr` and `MissingPositiveTermInAnd` as documented in
  `docs/semgrep-language-reference/semgrep-operator-precedence.md`.
- Do not implement pattern atom compilation, Tree-sitter matching, or DSL
  parsing as part of this work (those belong to 4.1.6+ and 4.2.x).
- Do not modify the public signature of `sempai_yaml::parse_rule_file`.
- Keep files below 400 lines.
- Every new module must begin with a `//!` module comment.
- New public items must carry Rustdoc with examples where feasible.
- Tests must use `rstest-bdd` v0.5.0 for BDD scenarios covering happy,
  unhappy, and edge paths.
- Record design decisions in
  `docs/sempai-query-language-design.md`.
- Update `docs/users-guide.md` with any user-visible behaviour change.
- Mark roadmap item 4.1.5 done in `docs/roadmap.md` only after all gates pass.

## Tolerances (exception triggers)

- Scope: if implementation requires more than 18 net file touches outside the
  `crates/sempai*` directories and the three required docs (`roadmap.md`,
  `users-guide.md`, `sempai-query-language-design.md`), stop and escalate.
- Interface: if normalization requires a breaking change to the public
  signature of `sempai::Engine::compile_yaml` (beyond replacing the
  `NOT_IMPLEMENTED` stub with a real return), stop and escalate.
- Dependencies: if a new external crate dependency beyond what is already in
  the workspace is required, stop and escalate.
- Iterations: if tests still fail after 5 attempts at a given fix, stop and
  escalate.
- Ambiguity: if multiple valid interpretations of the normalization rules
  exist for a concrete fixture and the choice materially affects downstream
  behaviour, stop and present options with trade-offs.

## Risks

- Risk: The `where` clause in `MatchFormula::Decorated` is currently stored as
  raw `serde_json::Value`.  Normalization must interpret this structure without
  full schema coverage, which could lead to incomplete handling. Severity:
  medium Likelihood: medium Mitigation: Preserve `where_clauses` as opaque
  `Value` slices inside `Decorated<Formula>` initially.  Only interpret
  `focus`, `metavariable-regex`, and `metavariable-pattern` when they are
  encountered.  All other clauses are stored verbatim and validated later in
  4.2.x.

- Risk: The `LegacyClause::Constraint(Value)` variant in `patterns` arrays
  carries metavariable constraints that are not formula nodes.  Normalization
  must distinguish formula items from constraint items. Severity: low
  Likelihood: high Mitigation: Map `LegacyClause::Constraint` to `WhereClause`
  entries attached to the enclosing `And` formula's `Decorated` wrapper,
  preserving them as opaque JSON for later semantic interpretation.

- Risk: Span propagation from parsed models to the canonical formula may be
  lossy if `LegacyFormula` and `MatchFormula` do not carry spans today.
  Severity: low Likelihood: medium Mitigation: Where spans are not available
  from the parser, use the enclosing rule span as a fallback.  Add a design-doc
  note that span precision will improve when the parser propagates spans to
  individual formula nodes.

- Risk: `r2c-internal-project-depends-on` rules have no formula body and cannot
  meaningfully normalize. Severity: low Likelihood: certain Mitigation:
  Represent as a dedicated `Formula::Atom(Atom::DependencyCheck)` variant or
  skip normalization for these rules and emit a degenerate plan. Decision to be
  taken in Stage B.

## Progress

- [ ] Stage A: Research and specification (no code changes).
- [ ] Stage B: Define `Formula`, `Atom`, `Decorated`, `WhereClause` types in
      `sempai_core`.
- [ ] Stage C: Implement normalization functions `normalize_legacy` and
      `normalize_match`.
- [ ] Stage D: Implement semantic validation (`InvalidNotInOr`,
      `MissingPositiveTermInAnd`).
- [ ] Stage E: Wire normalization into `Engine::compile_yaml` and produce real
      `QueryPlan` values.
- [ ] Stage F: Add unit tests, BDD scenarios, and paired legacy/v2 fixture
      coverage.
- [ ] Stage G: Update documentation, roadmap, and user's guide.
- [ ] Stage H: Run all quality gates and finalize.

## Surprises & discoveries

(None yet.)

## Decision log

(None yet.)

## Outcomes & retrospective

(To be completed after implementation.)

## Context and orientation

The Sempai query pipeline currently stops at a `NOT_IMPLEMENTED` placeholder
after successfully parsing and mode-validating a YAML rule file.  The parser
produces `LegacyFormula` and `MatchFormula` abstract syntax trees (ASTs) in
`sempai_yaml`, which are structurally distinct but semantically equivalent for
the subset they share.

The normalization step must lower both representations into the design
document's canonical `Formula` enum, then run semantic constraint checks before
constructing a `QueryPlan`.

### Key files and modules

- `crates/sempai-core/src/lib.rs` — exports `DiagnosticReport`, `SourceSpan`,
  `DiagnosticCode`, etc.
- `crates/sempai-core/src/diagnostic.rs` — diagnostic types and constructors.
- `crates/sempai-yaml/src/model.rs` — `LegacyFormula`, `MatchFormula`,
  `LegacyClause`, `LegacyValue`, `SearchQueryPrincipal`, `Rule`.
- `crates/sempai-yaml/src/lib.rs` — public re-exports and `parse_rule_file`.
- `crates/sempai/src/engine.rs` — `Engine::compile_yaml` with the
  `NOT_IMPLEMENTED` placeholder.
- `crates/sempai/src/mode_validation.rs` — mode-gating pass.
- `crates/sempai/src/tests/behaviour.rs` — existing BDD test world.
- `crates/sempai/tests/features/sempai_engine.feature` — existing BDD
  scenarios.
- `docs/sempai-query-language-design.md` — canonical `Formula` specification.
- `docs/semgrep-language-reference/semgrep-operator-precedence.md` — semantic
  constraint definitions.
- `docs/semgrep-language-reference/semgrep-legacy-vs-v2-guidance.md` —
  normalization guidance ("Option B: Compatibility parser").

### Mapping from parsed types to canonical Formula

The design document specifies these constructors:

```rust
pub enum Formula {
    Atom(Atom),
    Not(Box<Decorated<Formula>>),
    Inside(Box<Decorated<Formula>>),
    Anywhere(Box<Decorated<Formula>>),
    And(Vec<Decorated<Formula>>),
    Or(Vec<Decorated<Formula>>),
}

pub enum Atom {
    Pattern(PatternAtom),
    Regex(RegexAtom),
    TreeSitterQuery(TreeSitterQueryAtom),
}

pub struct Decorated<T> {
    pub node: T,
    pub where_clauses: Vec<WhereClause>,
    pub as_name: Option<String>,
    pub fix: Option<String>,
    pub span: Option<SourceSpan>,
}
```

Legacy-to-canonical mapping:

| Legacy                                   | Canonical                           |
| ---------------------------------------- | ----------------------------------- |
| `pattern: "..."` (string)                | `Formula::Atom(Atom::Pattern(...))` |
| `pattern-regex: "..."`                   | `Formula::Atom(Atom::Regex(...))`   |
| `patterns: [...]`                        | `Formula::And([...])`               |
| `pattern-either: [...]`                  | `Formula::Or([...])`                |
| `pattern-not: ...`                       | `Formula::Not(Box<...>)`            |
| `pattern-inside: ...`                    | `Formula::Inside(Box<...>)`         |
| `pattern-not-inside: ...`                | `Formula::Not(Inside(...))`         |
| `pattern-not-regex: "..."`               | `Formula::Not(Atom(Regex(...)))`    |
| `semgrep-internal-pattern-anywhere: ...` | `Formula::Anywhere(Box<...>)`       |

v2-to-canonical mapping:

| v2 (`match`)                         | Canonical                           |
| ------------------------------------ | ----------------------------------- |
| `"..."` (string shorthand)           | `Formula::Atom(Atom::Pattern(...))` |
| `pattern: "..."`                     | `Formula::Atom(Atom::Pattern(...))` |
| `regex: "..."`                       | `Formula::Atom(Atom::Regex(...))`   |
| `all: [...]`                         | `Formula::And([...])`               |
| `any: [...]`                         | `Formula::Or([...])`                |
| `not: ...`                           | `Formula::Not(Box<...>)`            |
| `inside: ...`                        | `Formula::Inside(Box<...>)`         |
| `anywhere: ...`                      | `Formula::Anywhere(Box<...>)`       |
| `Decorated { where, as, fix, .. }`   | `Decorated<Formula>` wrapper[^d]    |

[^d]: The canonical `Decorated<Formula>` exposes the fields `where_clauses`,
    `as_name`, `fix`, and `span: Option<SourceSpan>` in addition to the inner
    `node`.

## Plan of work

### Stage A: Research and specification (no code changes)

Review the current crate boundaries and confirm that the formula types belong
in `sempai_core`.  Confirm the exact atom types needed for this milestone
(pattern string and regex string; `TreeSitterQuery` can remain as a stub).

Go/no-go: understanding is sufficient to define types with confidence.

### Stage B: Define canonical Formula types in `sempai_core`

Add a new module `crates/sempai-core/src/formula.rs` (< 200 lines) containing:

- `Formula` enum — `Atom`, `Not`, `Inside`, `Anywhere`, `And`, `Or`.
- `Atom` enum — `Pattern(PatternAtom)`, `Regex(RegexAtom)`,
  `TreeSitterQuery(TreeSitterQueryAtom)`.
- `PatternAtom` — wraps a pattern string (the raw host-language snippet).
- `RegexAtom` — wraps a regex string.
- `TreeSitterQueryAtom` — stub wrapping a query string (for escape hatch).
- `Decorated<T>` — generic wrapper with `node: T`, `where_clauses:
  Vec<WhereClause>`, `as_name: Option<String>`, `fix: Option<String>`, `span:
  Option<SourceSpan>`.
- `WhereClause` — initially wraps `serde_json::Value` for opaque constraint
  storage; later milestones interpret specific clause types.

Add `pub mod formula;` and re-export from `crates/sempai-core/src/lib.rs`.

Go/no-go: `cargo doc -p sempai_core` builds, and `make lint` passes.

### Stage C: Implement normalization functions

Add a new module `crates/sempai/src/normalize.rs` (or split into
`normalize/legacy.rs` and `normalize/match_v2.rs` if size requires) with:

- `pub(crate) fn normalize_search_principal(principal: &SearchQueryPrincipal)
  -> Result<Decorated<Formula>, DiagnosticReport>`
- Internal helpers:
  - `fn normalize_legacy(formula: &LegacyFormula) -> Decorated<Formula>`
  - `fn normalize_legacy_clause(clause: &LegacyClause) -> ...`
  - `fn normalize_match(formula: &MatchFormula) -> Decorated<Formula>`

The normalization must handle:

1. Atomic patterns and regex strings → `Atom::Pattern` / `Atom::Regex`.
2. `pattern-not-inside` → `Not(Inside(...))` composite.
3. `pattern-not-regex` → `Not(Atom(Regex(...)))` composite.
4. `LegacyClause::Constraint(Value)` → `WhereClause` entries attached to the
   enclosing `And`'s `Decorated` wrapper.
5. `MatchFormula::Decorated` → `Decorated<Formula>` with `where_clauses`,
   `as_name`, and `fix` mapped through.
6. `SearchQueryPrincipal::ProjectDependsOn` → produce a placeholder
   `Formula::Atom(Atom::Pattern(...))` or a new `Atom::DependencyCheck` variant
   (decision to be taken here and recorded in the decision log).

Go/no-go: unit tests show that paired legacy and v2 YAML fixtures produce
structurally equivalent `Formula` values.

### Stage D: Implement semantic validation

Add a new module `crates/sempai/src/semantic_check.rs` (< 200 lines) with:

- `pub(crate) fn validate_formula(formula: &Decorated<Formula>)
  -> Result<(), DiagnosticReport>`
- Internal checks:
  - `check_no_not_in_or(formula)` — walks `Or` branches and rejects any
    that are `Formula::Not`.  Emits `E_SEMPAI_INVALID_NOT_IN_OR`.
  - `check_positive_term_in_and(formula)` — walks `And` branches and
    ensures at least one is a positive term (not `Not`, `Inside`, or
    `Anywhere`).  Emits `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`.

Both checks must be recursive: they apply to nested sub-formulas as well.

Go/no-go: semantically invalid fixtures produce the correct diagnostic codes,
and valid fixtures pass without error.

### Stage E: Wire normalization into Engine::compile_yaml

Modify `crates/sempai/src/engine.rs`:

1. After `validate_supported_modes(&file)?`, iterate search rules.
2. For each rule, call `normalize_search_principal(rule.principal())`.
3. Call `validate_formula(...)` on the normalized result.
4. Construct a `QueryPlan` containing the normalized formula and rule metadata.
5. Return `Ok(plans)` instead of `Err(DiagnosticReport::not_implemented(...))`.

Update `QueryPlan` to hold the `Decorated<Formula>` (replace the `_plan: ()`
placeholder).

Go/no-go: existing BDD scenario "Engine compile_yaml keeps a post-parse
placeholder for valid YAML" must be updated to expect success (or a new
behaviour) instead of `NOT_IMPLEMENTED`.

### Stage F: Add tests

Unit tests in `crates/sempai-core/src/tests/`:

- `formula_tests.rs` — construction and equality of Formula variants.
- Structural equivalence assertion helpers.

Unit tests in `crates/sempai/src/tests/`:

- `normalize_tests.rs` — paired legacy/v2 YAML inputs that must produce
  equivalent Formula outputs.
- `semantic_check_tests.rs` — fixtures for `InvalidNotInOr` and
  `MissingPositiveTermInAnd` with nested and non-nested variants.

BDD scenarios in `crates/sempai/tests/features/sempai_engine.feature`:

- Happy path: valid legacy search rule normalizes and compiles successfully.
- Happy path: valid v2 match search rule normalizes and compiles successfully.
- Happy path: paired legacy and v2 rules produce equivalent plans.
- Unhappy path: `pattern-either` with negated branch emits
  `E_SEMPAI_INVALID_NOT_IN_OR`.
- Unhappy path: `patterns` with only constraints (no positive term) emits
  `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`.
- Unhappy path: `any` with `not` branch emits `E_SEMPAI_INVALID_NOT_IN_OR`.
- Unhappy path: `all` with no positive term emits
  `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`.
- Edge: dependency-only rule compiles without normalization error.
- Edge: deeply nested formula normalizes without stack overflow or error.
- Edge: v2 `Decorated` formula preserves `where`, `as`, and `fix` metadata.

Go/no-go: `cargo test -p sempai_core`, `cargo test -p sempai` both pass.

### Stage G: Update documentation

1. `docs/sempai-query-language-design.md` — add implementation note recording
   that normalization is now live, noting the decision on `WhereClause`
   representation and `ProjectDependsOn` handling.
2. `docs/users-guide.md` — update the Sempai section to reflect that
   `compile_yaml` now returns compiled plans for valid search rules instead of
   `NOT_IMPLEMENTED`.
3. `docs/roadmap.md` — mark item 4.1.5 as `[x]` done.

Go/no-go: `make markdownlint`, `make fmt`, and `make nixie` pass.

### Stage H: Final quality gates

Run the full quality gate suite:

```plaintext
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-5-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-5-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-5-make-test.log
```

Go/no-go: all three pass with zero warnings or failures.

## Concrete steps

Working directory: `/home/user/project`

### Stage B commands

```sh
# After creating formula.rs and updating lib.rs:
set -o pipefail; cargo doc -p sempai_core 2>&1 | tee /tmp/4-1-5-cargo-doc.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-5-stage-b-lint.log
```

Expected: zero warnings, documentation builds cleanly.

### Stage C–D commands

```sh
# After implementing normalization and semantic checks:
set -o pipefail; cargo test -p sempai 2>&1 | tee /tmp/4-1-5-stage-cd-test.log
```

Expected: all new tests pass; existing tests either pass or are updated.

### Stage E commands

```sh
# After wiring into engine:
set -o pipefail; cargo test -p sempai 2>&1 | tee /tmp/4-1-5-stage-e-test.log
set -o pipefail; cargo test -p sempai_core 2>&1 | tee /tmp/4-1-5-stage-e-core.log
```

Expected: existing BDD scenarios updated, all pass.

### Stage H commands

```sh
set -o pipefail; make check-fmt 2>&1 | tee /tmp/4-1-5-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/4-1-5-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/4-1-5-make-test.log
set -o pipefail; make fmt 2>&1 | tee /tmp/4-1-5-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/4-1-5-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/4-1-5-make-nixie.log
```

Expected: all pass cleanly.

## Validation and acceptance

Acceptance is proven when:

1. Running `make test` passes, including new tests in `sempai_core` and
   `sempai`.
2. A paired fixture demonstrates that legacy `patterns: [...]` with
   `pattern-inside` normalizes to the same `Formula` shape as v2
   `match: { all: [...], inside: ... }`.
3. A fixture with `pattern-either: [pattern-not: ...]` emits
   `E_SEMPAI_INVALID_NOT_IN_OR`.
4. A fixture with `patterns: [pattern-not: ..., metavariable-regex: ...]`
   (only constraints, no positive pattern) emits
   `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND`.
5. A valid rule file with a single search rule returns `Ok(Vec<QueryPlan>)`
   with `len() == 1` from `Engine::compile_yaml`.
6. `make check-fmt`, `make lint`, `make markdownlint`, and `make nixie` all
   pass.

Quality criteria:

- Tests: `make test` — all workspace tests pass (including new normalization
  tests).
- Lint/typecheck: `make check-fmt && make lint` — zero warnings.
- Documentation: `make markdownlint && make nixie` — zero errors.

Quality method:

- Run the commands listed in "Concrete steps" Stage H.
- Verify that the BDD feature file contains at minimum 10 scenarios covering
  the happy, unhappy, and edge paths described above.

## Idempotence and recovery

All steps are idempotent.  Re-running `make test` after implementation produces
the same results.  No destructive filesystem operations are performed.  If a
stage fails partway through, the previous successful stage's state remains
intact and the failed stage can be retried.

## Artifacts and notes

Key paired fixture example (to be created):

```yaml
# Legacy form
rules:
  - id: paired.legacy
    message: find foo inside bar
    languages: [python]
    severity: WARNING
    patterns:
      - pattern: foo($X)
      - pattern-inside: |
          def bar():
              ...
```

```yaml
# v2 form (equivalent)
rules:
  - id: paired.v2
    message: find foo inside bar
    languages: [python]
    severity: WARNING
    match:
      all:
        - pattern: foo($X)
        - inside:
            pattern: |
              def bar():
                  ...
```

Both must normalize to:

```plaintext
Formula::And([
    Decorated { node: Formula::Atom(Atom::Pattern("foo($X)")), ... },
    Decorated { node: Formula::Inside(
        Box(Decorated { node: Formula::Atom(Atom::Pattern("def bar():\n    ...")), ... })
    ), ... },
])
```

## Interfaces and dependencies

### New types in `crates/sempai-core/src/formula.rs`

```rust
/// Canonical normalized query formula.
///
/// All legacy and v2 syntaxes are lowered into this shared representation
/// before semantic validation and plan compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Formula {
    /// A leaf pattern or regex atom.
    Atom(Atom),
    /// Negation: the inner formula must not match.
    Not(Box<Decorated<Formula>>),
    /// Context constraint: the anchor must be inside a match of the inner.
    Inside(Box<Decorated<Formula>>),
    /// Context constraint: the inner must match somewhere in scope.
    Anywhere(Box<Decorated<Formula>>),
    /// Conjunction: all branches must match.
    And(Vec<Decorated<Formula>>),
    /// Disjunction: at least one branch must match.
    Or(Vec<Decorated<Formula>>),
}

/// A leaf atom in the formula tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    /// A host-language pattern snippet.
    Pattern(PatternAtom),
    /// A regex pattern.
    Regex(RegexAtom),
    /// A raw Tree-sitter query (escape hatch).
    TreeSitterQuery(TreeSitterQueryAtom),
}

/// A pattern snippet atom containing a host-language code fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternAtom {
    pub text: String,
}

/// A regex atom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexAtom {
    pub pattern: String,
}

/// A raw Tree-sitter query atom (stub for 4.2.x).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeSitterQueryAtom {
    pub query: String,
}

/// Wraps a formula node with optional decorator metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decorated<T> {
    /// The core formula or atom.
    pub node: T,
    /// Optional `where` constraint clauses.
    pub where_clauses: Vec<WhereClause>,
    /// Optional alias binding name.
    pub as_name: Option<String>,
    /// Optional fix template text.
    pub fix: Option<String>,
    /// Source span for diagnostic anchoring.
    pub span: Option<SourceSpan>,
}

/// An opaque `where` constraint clause preserved for later interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereClause {
    /// The raw JSON value of the constraint.
    pub raw: serde_json::Value,
}
```

### New functions in `crates/sempai/src/normalize.rs`

```rust
/// Normalizes a parsed search principal into the canonical formula model.
pub(crate) fn normalize_search_principal(
    principal: &SearchQueryPrincipal,
    rule_span: Option<&SourceSpan>,
) -> Result<Decorated<Formula>, DiagnosticReport>;
```

### New functions in `crates/sempai/src/semantic_check.rs`

```rust
/// Validates semantic constraints on a normalized formula.
pub(crate) fn validate_formula(
    formula: &Decorated<Formula>,
) -> Result<(), DiagnosticReport>;
```

### Dependencies

No new external crate dependencies are required.  `serde_json` (already a
workspace dependency) is needed in `sempai_core` for `WhereClause::raw`. This
requires adding `serde_json` to `sempai_core`'s `[dependencies]` section.
