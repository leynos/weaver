# Sempai v0.1 → v0.2 migration guide

## Summary of breaking change

- `Engine::compile_yaml` now returns `Ok(Vec<QueryPlan>)` for valid search
  rules instead of a `NOT_IMPLEMENTED` diagnostic.
- Semantic validation now surfaces deterministic `E_SEMPAI_*` diagnostics.

## Before (v0.1)

```rust
let result = engine.compile_yaml(yaml);
let err = result.expect_err("normalization not implemented");
assert_eq!(err.diagnostics()[0].code(), DiagnosticCode::NotImplemented);
```

## After (v0.2)

```rust
let plans = engine.compile_yaml(yaml)?; // Ok(Vec<QueryPlan>)
let plan = &plans[0];
let formula = plan.formula(); // &Decorated<Formula>
```

## New failure modes

- `E_SEMPAI_YAML_PARSE` — malformed YAML
- `E_SEMPAI_SCHEMA_INVALID` — invalid rule schema, such as an unsupported
  language
- `E_SEMPAI_INVALID_NOT_IN_OR` — negated branch inside disjunction
- `E_SEMPAI_MISSING_POSITIVE_TERM_IN_AND` — conjunction lacks a positive term
- `E_SEMPAI_UNSUPPORTED_MODE` — extract, taint, join, or unknown modes

## Code updates checklist

- Replace tests expecting `NotImplemented` with success-path assertions over
  `plan.formula()`.
- Assert explicit `DiagnosticCode` values on failures.

## Examples

Minimal success-path assertion:

```rust
use sempai::{Engine, EngineConfig};
use sempai_core::formula::{Atom, Formula};

let yaml = concat!(
    "rules:\n",
    "  - id: demo.rule\n",
    "    message: demo\n",
    "    languages: [rust]\n",
    "    severity: ERROR\n",
    "    pattern: foo($X)\n",
);

let engine = Engine::new(EngineConfig::default());
let plans = engine.compile_yaml(yaml)?;
let plan = plans.first().expect("expected one query plan");
let formula = plan.formula();

assert!(
    matches!(&formula.node, Formula::Atom(Atom::Pattern(p)) if p.text == "foo($X)")
);
# Ok::<(), sempai::DiagnosticReport>(())
```

Negated disjunction branch failure:

```rust
use sempai::{DiagnosticCode, Engine, EngineConfig};

let yaml = concat!(
    "rules:\n",
    "  - id: demo.invalid.not.in.or\n",
    "    message: invalid not in or\n",
    "    languages: [rust]\n",
    "    severity: ERROR\n",
    "    pattern-either:\n",
    "      - pattern: foo($X)\n",
    "      - pattern-not: bar($Y)\n",
);

let engine = Engine::new(EngineConfig::default());
let err = engine.compile_yaml(yaml).expect_err("semantic validation should fail");
let first = err.diagnostics().first().expect("expected diagnostic");

assert_eq!(first.code(), DiagnosticCode::ESempaiInvalidNotInOr);
```
