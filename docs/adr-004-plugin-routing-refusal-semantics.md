# Architectural decision record (ADR) 004: Plugin routing and refusal semantics

## Status

Proposed.

## Date

2026-03-22.

## Context and problem statement

Weaver already has a capability model for plugin-backed `act` operations, but
provider selection still needs a precise contract for what happens when no
provider matches, when policy rejects a request, or when a plugin returns a
partial result that must still pass through the safety harness.

That gap matters because the user-facing behaviour has to stay deterministic.
If routing and refusal semantics are implicit, tests cannot make stable
assertions about the selected provider, refusal diagnostics, or rollback
guarantees.

## Decision drivers

- Keep provider selection deterministic.
- Make refusal diagnostics stable and machine-readable.
- Preserve rollback guarantees for plugin-produced edits.
- Keep routing behaviour testable from the daemon boundary.

## Requirements

### Functional requirements

- Resolve a provider from language, capability, and policy.
- Emit structured refusal diagnostics when no provider is eligible.
- Preserve a single shared commit path for accepted edits.

### Technical requirements

- Use stable refusal codes and reason fields.
- Keep routing decisions observable in logs and tests.
- Treat successful plugin output as input to the existing transaction path.

## Options considered

### Option A: Deterministic routing with explicit refusal

Route requests to the first eligible provider and refuse when none matches.

### Option B: Best-effort fallback

Try one provider, then silently fall back to another provider or a degraded
path.

### Option C: Permissive acceptance

Accept any provider output and repair the result later if verification fails.

## Decision outcome / proposed direction

Adopt deterministic routing with explicit refusal.

Weaver should choose one provider that satisfies the request constraints and
return a structured refusal when no provider qualifies. Accepted output still
flows through the shared transaction and verification path so rollback
behaviour stays consistent.

## Goals and non-goals

### Goals

- Make routing outcomes predictable.
- Keep refusal diagnostics stable across releases.
- Preserve the existing safety-harness contract.

### Non-goals

- Guarantee that every provider can satisfy every request.
- Hide refusal conditions behind generic failures.
- Replace the transaction path with provider-specific commit logic.

## Migration plan

1. Add stable refusal codes and reason strings.
2. Make provider resolution emit the selected provider and policy rationale.
3. Add tests for no-match, policy-reject, and accepted-edit paths.
4. Feed successful plugin output back through the existing verification path.

## Known risks and limitations

- Providers can still fail after selection, so the commit path needs its own
  verification.
- Overly broad fallback logic would weaken determinism.
- Routing changes need careful regression coverage because callers may depend
  on refusal codes.

## Architectural rationale

Deterministic routing keeps the plugin layer composable without making the
daemon's behaviour opaque. The refusal contract is part of the public
orchestration boundary, not an internal implementation detail.
