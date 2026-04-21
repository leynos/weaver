# Architectural decision record (ADR) 005: Verification trust boundary for Weaver

## Status

Proposed

## Date

2026-03-22

## Outstanding Decisions

- Which Weaver-owned invariants are inside the verified kernel for phase one?
- Which external tools remain trusted inputs rather than verified components?
- Which filesystem assumptions must be documented before the ADR can be
  accepted?

## Context and Problem Statement

Formal verification is only useful if Weaver states which parts of the system
are actually being proved. The tool depends on external language servers,
Tree-sitter grammars, sandboxing support, and filesystem behaviour that Weaver
does not own.

Without an explicit trust boundary, proof claims would blur Weaver-owned
invariants with assumptions about external components.

## Decision Drivers

- Keep verification claims honest.
- Avoid proving third-party behaviour as if Weaver owned it.
- Make proof obligations small enough to maintain.
- Keep the boundary clear in docs and proof harnesses.

## Requirements

### Functional Requirements

- Identify the verified kernel explicitly.
- Identify trusted external components explicitly.
- Preserve the ability to add new proofs without reclassifying the whole
  system.

### Technical Requirements

- Keep trust-boundary language consistent across design, roadmap, and proof
  docs.
- Allow proof harnesses to depend on modelled filesystem and tool assumptions.

## Options Considered

### Option A: Verify the orchestration kernel only

Prove Weaver-owned transaction, routing, and policy invariants while treating
external tools as trusted inputs.

### Option B: Verify everything end-to-end

Try to prove the semantics of Tree-sitter, LSP servers, sandbox internals, and
filesystem behaviour alongside Weaver code.

### Option C: Leave the boundary implicit

Write proofs and docs without distinguishing Weaver-owned behaviour from
external assumptions.

## Decision Outcome / Proposed Direction

Adopt Option A.

Weaver will prove the orchestration kernel only: transaction ordering, path
policy, capability routing, refusal semantics, and bounded counters. External
tools remain trusted inputs, and the docs must state the assumptions those
tools introduce.

## Goals and Non-Goals

### Goals

- Keep verification scope explicit.
- Prevent overclaiming about external tools.
- Make proof obligations easier to review.

### Non-goals

- Prove the correctness of third-party language servers.
- Prove the correctness of Tree-sitter or sandbox internals.
- Remove all runtime assumptions from the system.

## Migration plan

1. Name the verified kernel in the design and roadmap docs.
2. List trusted external tools and filesystem assumptions explicitly.
3. Make proof harnesses refer to the modelled assumptions by name.
4. Keep future-proof work inside the documented boundary.

## Known Risks and Limitations

- The boundary can drift if docs and code are not updated together.
- Proofs can appear stronger than they are if assumptions are omitted.
- External tool updates may change behaviour without changing Weaver code.

## Architectural Rationale

An explicit boundary keeps the assurance story accurate. Weaver can prove what
it owns and document what it does not.
