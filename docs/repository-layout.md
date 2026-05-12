# Weaver repository layout

This document defines the current repository layout for Weaver and separates
implemented components from planned components.

System semantics and architecture are defined in
[weaver-design.md](weaver-design.md). Implementation sequencing is defined in
[roadmap.md](roadmap.md). Sempai query-language architecture and crate planning
are defined in
[sempai-query-language-design.md](sempai-query-language-design.md).
Documentation conventions are defined in
[documentation-style-guide.md](documentation-style-guide.md) and `AGENTS.md`.
ADR 007 defines the pre-0.1.0 command-surface reset and the boundary between
Weaver-owned semantic code-editing behaviour and reusable command-contract
machinery that Weaver consumes from OrthoConfig.

## Layout goals

The repository layout is designed to:

- keep runtime roles clear across command-line interface (CLI), daemon, and
  backend crates,
- separate security-critical concerns such as sandboxing and safety locks,
- support testability and deterministic quality gates, and
- make planned work discoverable without obscuring implemented ownership.

In this document, Language Server Protocol (LSP) and Continuous Integration
(CI) are expanded here and then used by acronym in later sections.

## Top-level structure

```plaintext
/
├── crates/
│   ├── weaver-build-util/
│   ├── weaver-cli/
│   ├── weaver-config/
│   ├── weaver-e2e/
│   ├── weaver-graph/
│   ├── weaver-lsp-host/
│   ├── weaver-plugin-rope/
│   ├── weaver-plugin-rust-analyzer/
│   ├── weaver-plugins/
│   ├── weaver-sandbox/
│   ├── weaver-syntax/
│   ├── weaverd/
│   ├── sempai-core/
│   ├── sempai/
│   └── weaver-cards/
├── docs/
├── test_expect/
├── .github/workflows/
├── Cargo.toml
├── Cargo.lock
├── Makefile
├── rust-toolchain.toml
├── LICENSE
└── README.md
```

## Implemented components

### Core workspace crates

| Crate                         | Responsibility                                                                                       | Status      |
| ----------------------------- | ---------------------------------------------------------------------------------------------------- | ----------- |
| `weaver-cli`                  | CLI entrypoint, command parsing, daemon lifecycle commands, and JSON Lines (JSONL) request streaming | Implemented |
| `weaverd`                     | Daemon orchestration, command dispatch, and write-operation safety harness                           | Implemented |
| `weaver-config`               | Shared configuration schema and loading for client and daemon                                        | Implemented |
| `weaver-lsp-host`             | Language server lifecycle, capability detection, and semantic operations                             | Implemented |
| `weaver-syntax`               | Tree-sitter parsing and structural search or rewrite functionality                                   | Implemented |
| `weaver-graph`                | Relational graph layer with LSP-backed call hierarchy provider                                       | Implemented |
| `weaver-sandbox`              | Sandbox boundary for external tools and plugin execution                                             | Implemented |
| `weaver-plugins`              | Plugin protocol, lifecycle management, and broker integration                                        | Implemented |
| `weaver-plugin-rope`          | Python specialist plugin integration                                                                 | Implemented |
| `weaver-plugin-rust-analyzer` | Rust specialist plugin integration                                                                   | Implemented |
| `weaver-build-util`           | Shared build-time utilities used across crates                                                       | Implemented |
| `weaver-e2e`                  | End-to-end test support crate and integration scaffolding                                            | Implemented |
| `sempai-core`                 | Sempai data model, diagnostics, and planning intermediate representation (IR) types                  | Implemented |
| `sempai`                      | Sempai facade crate with stable public API, re-exports from `sempai-core`, and stub `Engine`         | Implemented |
| `weaver-cards`                | Stable JSONL schemas for `observe get-card` symbol card requests and responses                       | Implemented |

_Table 1: Implemented crate boundaries and responsibilities._

### Implemented shared directories

| Path                 | Purpose                                                                                               | Status      |
| -------------------- | ----------------------------------------------------------------------------------------------------- | ----------- |
| `docs/`              | Design docs, architectural decision records (ADRs), roadmap, migration guides, and reference material | Implemented |
| `test_expect/`       | Golden and expectation artefacts used by test suites                                                  | Implemented |
| `.github/workflows/` | CI workflows and automation policy                                                                    | Implemented |

_Table 2: Implemented shared directories and their roles._

## Planned components

Planned components are listed in `docs/roadmap.md` and are not yet fully
implemented in this repository snapshot. The 0.1.0 command-surface reset means
planned public commands should use the resource-first grammar from ADR 007 even
when older designs mention `observe`, `act`, or `verify`.

| Planned component                                                          | Intended location                                                          | Roadmap reference                                      |
| -------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------ |
| Weaver command-surface adapter for semantic code-editing metadata          | `crates/weaver-cli/`, `crates/weaverd/`, and Weaver-owned metadata modules | Roadmap 13.1; depends on OrthoConfig 5.2.3, 6.1, 7.2.7 |
| Resource-first command handlers and examples                               | `crates/weaver-cli/`, `crates/weaverd/`, and `docs/`                       | Roadmap 13.2 through 15.3                              |
| Human and machine renderer integration                                     | `crates/weaver-cli/` and shared output modules                             | Roadmap 13.3; depends on OrthoConfig 7.2 and 8.1       |
| Structured context, capability, and skill surfaces                         | `crates/weaver-cli/`, `crates/weaverd/`, and future skill manifests        | Roadmap 13.4; depends on OrthoConfig 6.2 and 6.3       |
| Sempai YAML, DSL, and Tree-sitter backend crates                           | `crates/sempai-yaml/`, `crates/sempai-dsl/`, and `crates/sempai-ts/`       | Roadmap 17 and Sempai query language technical design  |
| Sempai selector integration in resource commands                           | `crates/weaver-cli/`, `crates/weaverd/`, and Sempai crates                 | Roadmap 14.3, 15.2, and 17                             |
| Rust `symbol.move` or `symbol.extract` actuator flow                       | `crates/weaver-plugin-rust-analyzer/` and `crates/weaverd/`                | Roadmap 15.1 and 18.1                                  |
| Rust extricate plugin overlay and Rust Analyzer (RA) orchestration modules | `crates/weaver-plugin-rust-analyzer/src/lsp/` and related plugin modules   | Proposed in Rust extricate actuator technical design   |
| Plugin capability metadata for symbol movement and extraction              | `crates/weaver-plugins/src/manifest/mod.rs`                                | Roadmap 18.1 and 18.2                                  |
| `srgn` specialist plugin                                                   | `crates/weaver-plugin-srgn/` (expected new crate)                          | Roadmap 18.1                                           |
| `jedi` specialist plugin                                                   | `crates/weaver-plugin-jedi/` (expected new crate)                          | Roadmap 18.2                                           |
| Static analysis provider for `weaver-graph`                                | `crates/weaver-graph/` provider modules                                    | Roadmap 17.2 and prototype archive graph phases        |
| Jobs, profiles, delivery, and feedback command support                     | `crates/weaver-cli/`, `crates/weaverd/`, and state/config modules          | Roadmap 16; depends on OrthoConfig 9.1 through 9.3     |
| Onboarding and explicit interactive review flows                           | `crates/weaver-cli/` and `crates/weaverd/` command handlers                | Roadmap 20.1                                           |
| Dynamic analysis ingestion provider                                        | `crates/weaver-graph/` provider modules                                    | Prototype archive advanced agent-support phases        |

_Table 3: Planned components and their expected repository placement._

## Layout governance rules

- Add new implementation units to the crate that owns the domain behaviour,
  rather than by technical layer alone.
- Add new plugins as dedicated crates under `crates/` and register them through
  `weaver-plugins` and daemon runtime configuration.
- Keep `docs/repository-layout.md` and `docs/roadmap.md` synchronized when
  planned components move to implemented status.
- Keep `docs/contents.md` updated when new documentation artefacts are added.

## Relationship to implementation planning

`docs/roadmap.md` defines delivery order and scope. This file defines ownership
and expected locations for implementation artefacts. Any roadmap item that adds
or renames a crate should update this layout document in the same change.
