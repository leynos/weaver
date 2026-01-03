# Deliver the weaver-graph call hierarchy provider

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

No `PLANS.md` file exists in the repository root, so this document stands on
its own.

## Purpose / Big Picture

Implement the relational intelligence layer so Weaver can build a call graph
from Language Server Protocol (LSP) `textDocument/callHierarchy` data. Success
is visible when the weaver-graph crate can return a graph of nodes and edges
for a symbol, and when the daemon-facing interface (or any public API) can
surface that graph to callers with clear errors for unsupported or empty
results. Behavioural tests must exercise both happy and unhappy paths using
`rstest-bdd` v0.2.0.

## Progress

- [x] 2026-01-02 Drafted ExecPlan for weaver-graph call hierarchy provider.
- [ ] Inventory current call hierarchy and graph-related code for gaps.
- [ ] Implement or complete the weaver-graph crate API and LSP provider.
- [ ] Add unit tests for graph data structures and LSP mapping.
- [ ] Add rstest-bdd behavioural tests covering success and failure cases.
- [ ] Update `docs/weaver-design.md` and `docs/users-guide.md`.
- [ ] Run `make check-fmt`, `make lint`, and `make test` successfully.
- [ ] Mark the Phase 2 roadmap entry as done.

## Surprises & Discoveries

- None yet.

## Decision Log

- Decision: Use Language Server Protocol (LSP) `textDocument/callHierarchy` as
  the minimum viable product (MVP) provider for the call graph. Rationale:
  Matches the Phase 2 roadmap requirement and reuses existing LSP
  infrastructure. Date/Author: 2026-01-02 / plan author.

## Outcomes & Retrospective

Not started yet.

## Context and Orientation

The Phase 2 roadmap item lives in `docs/roadmap.md` and calls for creating the
`weaver-graph` crate with an LSP-backed provider. The design context for call
graphs is documented in `docs/weaver-design.md` (see the call graph and
provider strategy section). The call hierarchy capability is part of the LSP
surface in `crates/weaver-lsp-host`, while user-facing behaviour is documented
in `docs/users-guide.md` under `observe call-hierarchy`.

Key files and modules to understand or edit:

- `crates/weaver-graph/src/lib.rs` and its modules for graph data types.
- `crates/weaver-graph/src/provider.rs` for the provider interface and LSP
  adapter.
- `crates/weaver-lsp-host/src/host.rs`, `server.rs`, and `capability.rs` for
  call hierarchy support and capability negotiation.
- `crates/weaver-graph/tests` for behavioural tests and
  `crates/weaver-graph/src/tests.rs` (or `src/tests/`) for unit tests.
- `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md` for
  documentation updates.

Definitions used in this plan:

- Call hierarchy: LSP API (`textDocument/prepareCallHierarchy`,
  `callHierarchy/incomingCalls`, `callHierarchy/outgoingCalls`) that returns
  callers and callees for a symbol at a position.
- Call graph: A graph of nodes (symbols) and edges (call relationships) with
  provenance describing which provider produced each edge.

## Plan of Work

First, confirm the current state of the workspace. If the `weaver-graph` crate
or call hierarchy support is missing, create or add it; if it already exists,
identify gaps against the roadmap requirement. Review how `weaver-lsp-host`
exposes call hierarchy calls and decide how to adapt it to a
`CallHierarchyClient` trait so the graph provider can be tested in isolation.

Next, implement or complete the `weaver-graph` API. Define public types for
nodes, edges, and the graph itself, including identifiers that are stable and
human readable (for example, `path:line:column:name`). Keep the graph
bidirectional to enable efficient `callers_of` and `callees_of` queries.
Implement a provider trait that can build graphs from a start position and
accept a depth limit. The LSP provider should: prepare call hierarchy items at
the requested position, map items to nodes, traverse incoming and outgoing
calls up to the requested depth, deduplicate nodes, and record edges with
source metadata. Return semantic errors for missing symbols, missing capability
support, and upstream LSP failures. Ensure each module has a `//!` comment and
public APIs have rustdoc examples.

Add unit tests that validate graph structure behaviour (node IDs, edge
creation, deduplication, depth limits, callers/callees queries). For
behavioural tests, add a new Gherkin feature file and use `rstest-bdd` to
exercise the provider against a stub call hierarchy client. Include scenarios
for a simple happy path (single caller and callee), a no-results path (symbol
not found), and a provider error path (simulated LSP failure or unsupported
capability). Use `rstest` fixtures for shared setup, keep tests deterministic,
and prefer `mockall` or a small in-memory stub implementing the client trait.

Update documentation to reflect the implemented behaviour. In
`docs/weaver-design.md`, document the concrete graph data model and the LSP
provider strategy, including provenance on edges. In `docs/users-guide.md`,
confirm the `observe call-hierarchy` output schema matches the implemented
graph (nodes and edges, and any fields such as source or confidence). Finally,
mark the Phase 2 roadmap item as done in `docs/roadmap.md`.

## Concrete Steps

Run these commands from the repository root (`/root/repo`). The `rg` commands
are safe to re-run.

1. Inventory current implementation and call hierarchy wiring.

   rg "weaver-graph" -n rg "callHierarchy" -n crates/weaver-lsp-host rg
   "call-hierarchy" -n docs

2. If the crate does not exist, create it and add it to the workspace.

   cargo new crates/weaver-graph --lib

   Then update `Cargo.toml` (workspace members and shared dependencies).

3. Implement or adjust the weaver-graph modules:

   - `crates/weaver-graph/src/lib.rs`
   - `crates/weaver-graph/src/node.rs`, `edge.rs`, `graph.rs`, `error.rs`
   - `crates/weaver-graph/src/provider.rs` for the LSP provider

4. Ensure `weaver-lsp-host` exposes call hierarchy operations and capabilities
   if missing. Touch the following only if required by the inventory step:

   - `crates/weaver-lsp-host/src/host.rs`
   - `crates/weaver-lsp-host/src/server.rs`
   - `crates/weaver-lsp-host/src/capability.rs`
   - `crates/weaver-lsp-host/src/errors.rs`

5. Add tests:

   - Unit tests in `crates/weaver-graph/src/tests.rs` or
     `crates/weaver-graph/src/tests/`.
   - Behavioural tests in `crates/weaver-graph/tests/behaviour.rs` with a
     feature file at `crates/weaver-graph/tests/features/weaver_graph.feature`.
   - Add `rstest-bdd` and `rstest-bdd-macros` as dev-dependencies in
     `crates/weaver-graph/Cargo.toml` using workspace versions.

6. Update documentation:

   - `docs/weaver-design.md`
   - `docs/users-guide.md`
   - `docs/roadmap.md`

7. Format and validate documentation (required after doc changes):

   set -o pipefail make fmt 2>&1 | tee /tmp/weaver-fmt.log make markdownlint
   2>&1 | tee /tmp/weaver-markdownlint.log

   Run `make nixie` only if a Mermaid diagram was edited.

8. Run the Rust quality gates:

   set -o pipefail make check-fmt 2>&1 | tee /tmp/weaver-check-fmt.log make
   lint 2>&1 | tee /tmp/weaver-lint.log make test 2>&1 | tee
   /tmp/weaver-test.log

## Validation and Acceptance

Acceptance requires all of the following:

- New unit tests for weaver-graph pass, and at least one test fails before the
  implementation and passes after.
- New `rstest-bdd` scenarios for call graph behaviour pass, covering both
  success and failure cases.
- `make check-fmt`, `make lint`, and `make test` all succeed.
- `docs/users-guide.md` accurately describes the call hierarchy output schema
  and any new CLI behaviour.
- `docs/weaver-design.md` records the graph model and provider decisions.
- The Phase 2 roadmap item is marked as done.

## Idempotence and Recovery

All commands and tests above are safe to re-run. If a test fails midway, fix
the underlying issue and re-run only the failing test or the full `make test`
suite. If formatting or linting fails, apply fixes and re-run the same command
until it passes. Keep documentation edits small so `make fmt` can be applied
multiple times without drift.

## Artifacts and Notes

Example Gherkin snippet for the behavioural test (stored in the feature file):

    Feature: Call graph via LSP call hierarchy

      Scenario: Build a call graph for a symbol with callers and callees
        Given a call hierarchy client with a simple call chain
        When a call graph is built from "main" with depth 2
        Then the graph includes nodes "main" and "helper"
        And the graph includes an edge from "main" to "helper"

Example JSON payload for `observe call-hierarchy` (update to match actual
output schema):

    {"nodes":[{"id":"/src/lib.rs:10:0:main"}],"edges":[{"caller":"n1","callee":"n2"}]}

## Interfaces and Dependencies

The weaver-graph crate must expose a minimal public API that other crates can
consume:

- `weaver_graph::CallGraph` with `add_node`, `add_edge`, `callers_of`,
  `callees_of`, `find_by_name`, `node_count`, and `edge_count`.
- `weaver_graph::CallNode` and `weaver_graph::CallEdge` with stable `NodeId`
  strings and optional call-site metadata.
- `weaver_graph::CallGraphProvider` with:

    fn build_graph(&mut self, position: &SourcePosition, depth: u32)
        -> Result<CallGraph, GraphError>;
    fn callers_graph(&mut self, position: &SourcePosition, depth: u32)
        -> Result<CallGraph, GraphError>;
    fn callees_graph(&mut self, position: &SourcePosition, depth: u32)
        -> Result<CallGraph, GraphError>;

- `weaver_graph::CallHierarchyClient` trait implemented by an adapter over
  `weaver-lsp-host` so the LSP provider can run without owning the host.

Dependencies and versions:

- `lsp-types` from the workspace for call hierarchy types.
- `rstest-bdd` and `rstest-bdd-macros` v0.2.0 for behavioural tests.
- `mockall` if mocking is required; otherwise use a small in-memory stub.

Ensure each module starts with a `//!` comment and keep files under 400 lines
by splitting modules if necessary.
