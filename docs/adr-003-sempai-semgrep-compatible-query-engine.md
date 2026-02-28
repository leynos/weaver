# Architectural decision record (ADR) 003: Semgrep-compatible query engine strategy

## Status

Proposed.

## Date

2026-02-28.

## Context and problem statement

ADR 002 proposes a Semgrep-style query surface for feature extraction in
Weaver. The next decision is execution strategy: whether to implement matching
fully in-house, adopt an existing Rust matcher as the primary engine, or use a
hybrid approach.

This decision must preserve support for Rust, Python, Go, and TypeScript while
maintaining predictable behaviour in daemon workflows.

## Decision drivers

- Delivery speed for a usable, reliable query engine.
- Behavioural clarity for Semgrep-style operators and captures.
- Integration and embedding in Rust with stable operational characteristics.
- Ability to evolve semantics without destabilizing user-facing syntax.
- Testability across mandatory language corpora.

## Requirements

### Functional requirements

- Execute the Semgrep-style subset selected by ADR 002.
- Provide deterministic capture behaviour for extraction pipelines.
- Support clear diagnostics when a rule cannot be executed as written.

### Technical requirements

- Run inside Weaver's Rust architecture without external service dependence.
- Keep performance acceptable for CLI and daemon use.
- Preserve explicit fallback paths for unsupported constructs.

## Options considered

### Option A: Sempai-native matcher only

Implement all matching semantics directly in Weaver with no external execution
backend.

Benefits:

- Full control over semantics and diagnostics.
- No backend translation layer.

Risks:

- Higher implementation effort and slower time to stable coverage.
- Greater maintenance burden for edge-case matching behaviour.

### Option B: ast-grep as the sole engine and user-facing syntax

Adopt ast-grep syntax and execution model directly.

Benefits:

- Mature Rust matcher with broad practical coverage.
- Lower implementation effort for core matching behaviour.

Risks:

- Diverges from Semgrep-compatible user surface already scoped for Weaver.
- Requires migration of Semgrep-style rule expectations.

### Option C: Hybrid execution with Semgrep-compatible front-end

Keep Semgrep-compatible parsing and normalization in Weaver. Route compatible
rules to ast-grep-backed execution paths where semantics align, and run
Weaver-native matching or explicit fallbacks for unsupported constructs.

Benefits:

- Preserves Semgrep-compatible surface while reducing reinvention.
- Allows gradual hardening with explicit compatibility boundaries.

Risks:

- Additional complexity in rule routing and diagnostics.
- Requires explicit conformance tests to avoid silent semantic drift.

## Option comparison

| Option | Delivery speed | Semantic control | Compatibility fit | Complexity |
| ------ | -------------- | ---------------- | ----------------- | ---------- |
| A      | Low            | High             | High              | Medium     |
| B      | High           | Medium           | Low               | Low        |
| C      | Medium         | High             | High              | High       |

_Table 1: Trade-offs for Semgrep-compatible engine implementation strategies._

## Decision outcome / proposed direction

Adopt Option C: hybrid execution.

Weaver keeps a Semgrep-compatible front-end and internal normalized formula
representation. Execution is routed as follows:

1. Use ast-grep-backed execution for rules that map cleanly.
2. Use Weaver-native matching for supported Semgrep-style constructs that do
   not map cleanly.
3. Return explicit unsupported-feature diagnostics when neither path can execute
   a rule safely.

## Goals and non-goals

### Goals

- Preserve Semgrep-compatible authoring for users and automation.
- Reuse mature Rust matching infrastructure where it is semantically safe.
- Make backend routing observable through diagnostics and tests.

### Non-goals

- Bit-for-bit compatibility with all Semgrep behaviour in the first release.
- Full ast-grep syntax exposure as the primary user language.
- Hidden fallbacks that change behaviour without explicit reporting.

## Migration plan

1. Define a rule-capability matrix for routing eligibility.
2. Implement routing with explicit reason codes.
3. Add conformance tests for mapped and non-mapped operators.
4. Add regression suites for capture boundaries and negation behaviour.
5. Document compatibility boundaries in user-facing reference docs.

## Known risks and limitations

- Routing misclassification could yield incorrect matches if not tested.
- Capture boundary differences may appear on edge cases.
- Debugging complexity increases with multiple execution paths.

## Outstanding decisions

- Which Semgrep-style operators are guaranteed in the first public milestone.
- Whether deep matching is first-class or fallback-only in the first release.
- Whether rule authors can force a backend selection for diagnostics.

## Architectural rationale

The hybrid direction matches Weaver's broader architecture: composable layers,
explicit fallbacks, and practical delivery without sacrificing long-term
control of user-facing behaviour.
