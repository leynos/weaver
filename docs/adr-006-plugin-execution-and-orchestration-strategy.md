# Architectural decision record (ADR) 006: Plugin execution and orchestration strategy

## Status

Proposed

## Date

2026-03-22

## Outstanding Decisions

- Which plugin categories, if any, need streaming behaviour instead of
  one-shot JSON Lines (JSONL)?
- Which team owns the broker-side timeout and payload-size defaults?
- Which plugin lifecycle guarantees must remain stable before acceptance?

## Context and Problem Statement

Weaver needs a concrete execution strategy for plugins and other external
helpers. The broker must keep ownership of command orchestration, but the
plugin boundary still needs to be simple enough for testing, debugging, and
sandbox control.

The design also needs to explain why Weaver uses one-shot JSON Lines (JSONL)
payloads rather than long-lived streaming sessions for plugin execution.

## Decision Drivers

- Keep plugin execution replaceable.
- Keep broker ownership in Weaver.
- Keep request and response shapes deterministic.
- Minimize long-lived plugin state.

## Requirements

### Functional Requirements

- Execute plugin work through `weaver-plugins`.
- Pass requests as a single JSONL payload over stdio.
- Preserve a single shared commit path for accepted edits.

### Technical Requirements

- Keep plugins short-lived and isolated.
- Keep the broker responsible for validation and orchestration.
- Avoid exposing plugin internals through the public command surface.

## Options Considered

### Option A: One-shot JSONL over stdio with broker ownership

The broker writes one request, reads one response, and owns orchestration.

### Option B: Long-lived plugin sessions

Keep plugins alive across multiple requests and reuse their in-memory state.

### Option C: Inline execution

Call plugin code directly from the daemon process instead of spawning a plugin
process.

## Decision Outcome / Proposed Direction

Adopt one-shot JSONL over stdio with broker ownership in `weaver-plugins`.

The broker should own request validation, execution selection, and final
handoff into the transaction path. Plugins remain implementation details and do
not control commit behaviour directly.

## Goals and Non-Goals

### Goals

- Keep the execution model predictable.
- Keep plugin lifecycles easy to sandbox.
- Keep orchestration logic centralized in the broker.

### Non-goals

- Maintain per-plugin sessions across requests.
- Expose plugin protocol details in the CLI contract.
- Move commit responsibility out of Weaver's safety harness.

## Migration plan

1. Keep plugin requests to one JSONL line in and one JSONL line out.
2. Make broker validation and routing explicit.
3. Preserve the safety-harness handoff for accepted edits.
4. Add tests that exercise refusal, success, and rollback paths.

## Known Risks and Limitations

- One-shot execution means each request pays a process startup cost.
- Broker bugs can affect multiple plugins if orchestration is centralized.
- Streaming state is unavailable, so plugins must be stateless or rehydrate
  their inputs each time.

## Architectural Rationale

One-shot brokered execution keeps the plugin boundary narrow and auditable. It
fits Weaver's command-line transport model and preserves the broker's control
over commit-sensitive behaviour.
