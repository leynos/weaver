# [Contents index](contents.md) - Central index for all documentation files in `docs/`

- [Architectural decision record (ADR) 001: Plugin capability model and
  `act extricate`](adr-001-plugin-capability-model-and-act-extricate.md)
  - Decision record for plugin capability declarations and extrication
    behaviours.
- [Architectural decision record (ADR) 002: Query language for feature
  extraction in
  Weaver](adr-002-query-language-for-feature-extraction-in-weaver.md)
  - Decision record for the Semgrep-style query surface and Tree-sitter
    execution strategy.
- [Architectural decision record (ADR) 003: Semgrep-compatible query engine
  strategy](adr-003-sempai-semgrep-compatible-query-engine.md)
  - Decision record for hybrid execution strategy and compatibility routing.
- [Architectural decision record (ADR) 004: Plugin routing and refusal
  semantics](adr-004-plugin-routing-refusal-semantics.md)
  - Decision record for deterministic routing, refusal diagnostics, and
    rollback guarantees.
- [Architectural decision record (ADR) 005: Verification trust boundary for
  Weaver](adr-005-verification-trust-boundary.md)
  - Decision record for the formal-verification boundary between Weaver-owned
    kernels and trusted external tools.
- [Architectural decision record (ADR) 006: Plugin execution and
  orchestration
  strategy](adr-006-plugin-execution-and-orchestration-strategy.md)
  - Decision record for one-shot JSONL execution and broker-owned orchestration.
- [Architectural decision record (ADR) 007: Agent-native command
  surface](adr-007-agent-native-command-surface.md)
  - Decision record for the human-friendly, agent-native 0.1.0 command
    contract and OrthoConfig dependency boundary.
- [Archived prototype roadmap](archive/prototype-roadmap.md)
  - Historical completed and superseded roadmap tasks from the pre-ADR-007
    command grammar, preserving task numbers `1` through `11`.
- [Building an error-recovering parser with Chumsky](building-an-error-recovering-parser-with-chumsky.md)
  - Practical parser-construction guidance for resilient parsing workflows.
- [Code complexity guide](complexity-antipatterns-and-refactoring-strategies.md)
  - Patterns for identifying complexity anti-patterns and planning
    refactoring.
- [Developer's guide](developers-guide.md)
  - Weaver developer's guide covering setup, architecture, and contribution
    guidelines.
- [Documentation style guide](documentation-style-guide.md)
  - Writing, formatting, grammar, and structure standards for repository docs.
- [Formal verification methods in Weaver](formal-verification-methods-in-weaver.md)
  - Recommended Kani, Verus, and testing strategy for Weaver's transactional
    and routing invariants.
- [`execplans/`](execplans/)
  - Living execution plans used to scope, implement, and verify discrete
    deliverables.
- [`rfcs/`](rfcs/)
  - Requests for comments covering proposed design contracts before
    implementation.
- [RFC 0001: Local daemon observability](rfcs/0001-o11y.md)
  - Proposed observability contract for the single-user `weaverd` daemon and
    its `weaver` CLI partner.
- [Ortho-config user's guide](ortho-config-users-guide.md)
  - Operational guide for configuration layering and ortho-config usage.
- [Ortho-config v0.8.0 migration guide](ortho-config-v0-8-0-migration-guide.md)
  - Weaver-specific adoption notes for the current ortho-config upgrade.
- [Ortho-config v0.6.0 migration guide](ortho-config-v0-6-0-migration-guide.md)
  - Migration notes and compatibility guidance for ortho-config changes.
- [Pratt parser design for DDlog expressions](pratt-parser-for-ddlog-expressions.md)
  - Pratt parsing design, AST modelling, and integration notes for DDlog.
- [Reliable testing in Rust via dependency injection](reliable-testing-in-rust-via-dependency-injection.md)
  - Testing strategies for deterministic Rust verification with injected
    dependencies.
- [Repository layout](repository-layout.md)
  - Implemented and planned repository structure with ownership mapping.
- [Roadmap](roadmap.md)
  - Phase, step, and task-level implementation plan for Weaver.
- [RSTest behaviour-driven testing guide](rstest-bdd-users-guide.md)
  - Behaviour-driven testing patterns using `rstest` in Rust.
- [Rust doctest DRY guide](rust-doctest-dry-guide.md)
  - Patterns for concise, maintainable Rust documentation tests.
- [Rust parser testing comprehensive guide](rust-parser-testing-comprehensive-guide.md)
  - End-to-end parser testing guidance for `logos`, `chumsky`, and `rowan`.
- [Rust extricate actuator plugin technical design](rust-extricate-actuator-plugin-technical-design.md)
  - Proposed design for Rust `extricate-symbol` orchestration, plugin
    transactions, and semantic verification workflow.
- [Rust testing with `rstest` fixtures](rust-testing-with-rstest-fixtures.md)
  - Shared fixture and parameterization patterns for Rust test suites.
- [Sempai query language technical design](sempai-query-language-design.md)
  - Proposed architecture for Semgrep-compatible parsing and Tree-sitter
    execution in Weaver.
- [Semgrep language reference directory](semgrep-language-reference/)
  - Semgrep grammar notes, schema references, precedence model, and examples.
- [User interface (UI) gap analysis](ui-gap-analysis.md)
  - Command-line interface (CLI) discoverability and usability gap inventory
    with priority mapping.
- [User's guide](users-guide.md)
  - End-user guide for Weaver configuration, daemon lifecycle, and commands.
- [Weaver design document](weaver-design.md)
  - System architecture, safety model, and phased delivery rationale.
- [Weaver LSP host v0.1.0 migration guide](weaver-lsp-host-v0-1-0-migration-guide.md)
  - Migration details for Language Server Protocol (LSP) host capability and
    API changes.
