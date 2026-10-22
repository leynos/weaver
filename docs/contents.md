# Documentation contents

[Documentation contents](contents.md) is the central index for documentation in
`docs/`. It groups long-lived reference material by audience and purpose so
readers can find the source of truth without scanning the whole tree.

- [Architectural decision record (ADR) 0001: Plugin capability model and
  `act extricate`](adrs/adr-0001-plugin-capability-model-and-act-extricate.md)
  - Decision record for plugin capability declarations and extrication
    behaviours.
- [Architectural decision record (ADR) 0002: Query language for feature
  extraction in
  Weaver](adrs/adr-0002-query-language-for-feature-extraction-in-weaver.md)
  - Decision record for the Semgrep-style query surface and Tree-sitter
    execution strategy.
- [Architectural decision record (ADR) 0003: Semgrep-compatible query engine
  strategy](adrs/adr-0003-sempai-semgrep-compatible-query-engine.md)
  - Decision record for hybrid execution strategy and compatibility routing.
- [Architectural decision record (ADR) 0004: Plugin routing and refusal
  semantics](adrs/adr-0004-plugin-routing-refusal-semantics.md)
  - Decision record for deterministic routing, refusal diagnostics, and
    rollback guarantees.
- [Architectural decision record (ADR) 0005: Verification trust boundary for
  Weaver](adrs/adr-0005-verification-trust-boundary.md)
  - Decision record for the formal-verification boundary between Weaver-owned
    kernels and trusted external tools.
- [Architectural decision record (ADR) 0006: Plugin execution and
  orchestration
  strategy](adrs/adr-0006-plugin-execution-and-orchestration-strategy.md)
  - Decision record for one-shot JSONL execution and broker-owned orchestration.
- [Architectural decision record (ADR) 0007: Agent-native command
  surface](adrs/adr-0007-agent-native-command-surface.md)
  - Decision record for the human-friendly, agent-native 0.1.0 command
    contract and OrthoConfig dependency boundary.
- [Architectural decision record (ADR) 0008: Workspace-scoped daemon
  tenancy](adrs/adr-0008-workspace-scoped-daemon-tenancy.md)
  - Proposed decision for canonical workspace identity beneath one per-user
    daemon.
- [Architectural decision record (ADR) 0009: Workspace-scoped language-server
  lifecycle](adrs/adr-0009-workspace-scoped-language-server-lifecycle.md)
  - Proposed decision for workspace-owned server pools, execution identity,
    and explicit Rust toolchain selection.
- [Architectural decision record (ADR) 0010: Workspace-local
  concurrency](adrs/adr-0010-workspace-local-concurrency.md)
  - Proposed decision for workspace-local coordination, mutation isolation,
    bounded admission, and overload behaviour.
- [Architectural decision record (ADR) 0011: Sempai query input
  syntax](adrs/adr-0011-sempai-query-input-syntax.md)
  - Proposed decision for bare positive queries, explicit rich expressions,
    and equivalent argument, file, and standard-input sources.
- [Architectural decision record (ADR) 0012: Versioned selector
  streams](adrs/adr-0012-versioned-selector-streams.md)
  - Proposed decision for deterministic selector JSONL, typed stream input,
    source identity, and stale-selector refusal.
- [Architectural decision record (ADR) 0013: Adopt the shared CV-005 contract
  library](adrs/adr-0013-adopt-the-shared-cv005-contract-library.md)
  - Accepted decision to run the pinned shared `cv005-contracts` check instead
    of a local copy of the CV-005 contract.
- [OrthoConfig consumer boundary matrix](orthoconfig-consumer-boundary.md)
  - Generated matrix classifying every command-contract roadmap task as an
    OrthoConfig consumer, temporary wrapper, pending dependency, or deliberate
    divergence.
- [Archived prototype roadmap](archive/prototype-roadmap.md)
  - Historical completed and superseded roadmap tasks from the pre-ADR-0007
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
- [`adrs/`](adrs/)
  - Architectural decision records capturing accepted and proposed design
    decisions.
- [`execplans/`](execplans/)
  - Living execution plans used to scope, implement, and verify discrete
    deliverables.
- [`rfcs/`](rfcs/)
  - Requests for comments covering proposed design contracts before
    implementation.
- [RFC 0001: Local daemon observability](rfcs/rfc-0001-local-daemon-observability.md)
  - Proposed observability contract for the single-user `weaverd` daemon and
    its `weaver` CLI partner.
- [RFC 0002: Multi-workspace daemon and language-server
  lifecycle](rfcs/rfc-0002-multi-workspace-daemon-and-language-server-lifecycle.md)
  - Proposed architecture for safe multi-repository tenancy, language-server
    reuse, Rust toolchain selection, and concurrent request execution.
- [RFC 0003: Sempai query-to-selector vertical
  slice](rfcs/rfc-0003-sempai-query-to-selector-vertical-slice.md)
  - Proposed delivery path from positive structural queries to versioned
    selector streams and explicit downstream actuation.
- [Ortho-config user's guide](ortho-config-users-guide.md)
  - Operational guide for configuration layering and ortho-config usage.
- [Ortho-config v0.9.0 migration guide](ortho-config-v0-9-0-migration-guide.md)
  - Current Weaver adoption notes for the published ortho-config release.
- [Ortho-config v0.8.0 migration guide](ortho-config-v0-8-0-migration-guide.md)
  - Historical Weaver-specific adoption notes for the v0.8.0 upgrade.
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
- [Weaver `act refactor` rename position migration guide](weaver-act-refactor-rename-position-migration-guide.md)
  - Migration notes for moving `act refactor` rename from `offset=` to
    `--position LINE:COL`.
- [Weaver LSP host v0.1.0 migration guide](weaver-lsp-host-v0.1.0-migration-guide.md)
  - Migration details for Language Server Protocol (LSP) host capability and
    API changes.
