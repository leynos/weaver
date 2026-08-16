# Architectural decision record (ADR) 012: Versioned selector streams

## Status

Proposed.

## Date

2026-08-16.

## Context and problem statement

ADR 007 requires observe-style resource commands to compose with mutation
commands through ordinary UNIX pipelines. A source range by itself is not a
safe cross-process selector. Files may change after a query, record order may
vary, capture text may be truncated, and a consumer cannot infer which backend
or source revision produced the range.

The public machine renderer also needs one collection shape. A JSON array
requires complete buffering, while opaque daemon handles cannot be inspected,
filtered, persisted, or replayed. An untyped `--from-stdin` flag does not tell
the command contract which schema owns standard input.

[RFC 0003](rfcs/0003-sempai-query-to-selector.md) contains the complete
query-to-actuation proposal.

## Decision drivers

- Preserve ordinary shell composition and `jq -c` filtering.
- Stream bounded records without buffering the complete result set.
- Make every selector self-contained and replayable.
- Carry source identity so stale ranges are refused.
- Keep backend and compatibility provenance visible.
- Give standard input an explicit schema owner.
- Define deterministic record ordering.
- Keep zero-, one-, and many-match mutation policy explicit.
- Avoid invented probabilistic confidence values.
- Preserve ADR 007's universal `--json` switch.

## Options considered

### Option A: Versioned JSONL records and `--selectors <path|->`

Emit one selector object per line under `--json`. Consumers accept the same
records from a file or standard input through a typed `--selectors` option.

### Option B: One JSON array

Buffer and emit the complete collection as one JSON value.

### Option C: Opaque daemon selector handles

Emit compact identifiers which consumers resolve through retained daemon state.

### Option D: Untyped `--from-stdin`

Stream JSONL, but let each command infer what standard input contains.

| Topic | JSONL selectors | JSON array | Opaque handles | Untyped stdin |
| ----- | --------------- | ---------- | -------------- | ------------- |
| Streaming | Yes | No | Yes | Yes |
| `jq -c` composition | Natural | Extra iteration | No | Natural |
| Replay after restart | Yes | Yes | No | Format-dependent |
| Stale-source evidence | Embedded | Embedded | Hidden | Format-dependent |
| Input schema discoverability | Explicit | Explicit | Handle protocol | Weak |

_Table 1: Selector stream alternatives._

## Decision statement

In the context of agent and human pipelines carrying structural matches between
independent processes, facing mutable source, bounded output, backend
replacement, and ambiguous standard-input ownership, Weaver decides for
versioned self-contained JSONL selector records consumed through
`--selectors <path|->`, and against arrays, opaque daemon handles, or untyped
standard input, to achieve streaming composition, replayability, deterministic
validation, and stale-source refusal, accepting per-record schema overhead and
the need to define command-specific cardinality policy.

## Decision outcome and proposed direction

Adopt option A.

For a multi-match resource command, universal `--json` selects deterministic
JSONL. Each line is one complete `weaver.selector.v1` object. Human output
remains the default outside `--json`.

A selector-aware consumer accepts:

```text
--selectors <path>
--selectors -
```

The second form assigns standard input to the selector stream. Generic
`--from-stdin` is not the canonical public flag for selectors.

A representative pipeline is:

```sh
weaver symbols list --lang rust --query 'fn $NAME($...ARGS)' --json \
  | jq -c 'select(.captures.NAME.text | startswith("old_"))' \
  | weaver symbols rename --selectors - --new-name run --dry-run
```

## Selector version 1 contract

Every record contains, directly or through versioned nested objects:

- selector schema identity;
- deterministic selector identity;
- workspace identity;
- source URI and language;
- complete match span;
- optional focus span;
- named captures with spans and syntax node kinds;
- optional, bounded capture text;
- source content digest;
- workspace revision where available;
- query kind and digest;
- capability and provider identity; and
- compatibility-subset provenance where a temporary executor produced it.

The selector identifier must include enough source and range identity that two
different source revisions cannot accidentally share one identifier.

No field claims statistical confidence unless a calibrated model produced it.
Deterministic engines report match evidence, execution route, and provenance
instead.

## Ordering and completion

The producer sorts records by:

1. canonical URI;
2. start byte;
3. end byte; and
4. selector identifier.

Zero matches emit zero records and return success. A consumer decides whether
zero input is acceptable for its operation.

A successful stream contains selector records only. It has no header, trailer,
or summary line. Schema identity appears in every record so a filtered or
partitioned stream remains self-describing.

If production fails before completion, the command returns a non-zero exit
class and a structured error on stderr. The first implementation must avoid
emitting partial selector success before a compile or execution failure can
still invalidate the complete result. A later explicitly partial protocol
requires another decision.

## Diagnostic separation

Under `--json`:

- selector records go to stdout;
- structured errors go to stderr;
- non-fatal bounded warnings go to stderr; and
- localized prose never appears inside protocol identifiers.

A query with parser error diagnostics emits no selectors, even when Chumsky
recovery produced a partial formula.

## Source identity and stale refusal

A selector describes the source snapshot against which it was calculated.
Consumers validate its digest or workspace revision before using its focus,
span, or captures.

A mutation command performs two checks:

1. selector validation before actuator planning; and
2. expected-base comparison in the shared mutation engine immediately before
   commit.

A stale selector returns a structured refusal containing expected and observed
identity plus guidance to rerun the query. `--force` cannot silently discard
the stale-source precondition.

## Cardinality and ambiguity

The selector protocol does not choose mutation cardinality.

Every actuator documents its policy for:

- zero selectors;
- exactly one selector;
- several selectors; and
- duplicate or overlapping selectors.

Mutation defaults should refuse ambiguous several-match intent. A command may
provide an explicit all-matches policy when applying the same operation is safe
and meaningful. Filtering the stream is also an explicit policy.

## Capture and focus targeting

Consumers use the focus span when present and the complete match span as the
documented fallback. A command may accept an explicit capture name to select a
capture span.

Missing focus or capture data never causes a consumer to choose a different
nearby node heuristically. It either follows the documented fallback or
returns a structured refusal.

## Standard-input ownership

A command accepts at most one input whose path is `-`. Command metadata declares
the expected stream schema. Invalid combinations fail before daemon startup and
enumerate the conflicting options.

This rule leaves later patch, regex replacement, and LSP operations free to
consume the same selector protocol without each inventing a different stdin
switch.

## Relationship to ADR 007

This ADR refines ADR 007's selector and pipeline contract:

- multi-record `--json` results use JSONL;
- typed selector input is `--selectors <path|->`; and
- selectors carry source identity and provenance.

The noun-verb command grammar, human renderer, universal `--json`, and
capability-first actuation decisions remain unchanged.

## Consequences

- Pipeline records are inspectable, persistable, filterable, and replayable.
- Every line carries some repeated schema and source metadata.
- Consumers can reject stale or cross-workspace records without hidden state.
- A query producer can remain independent from a later actuator process.
- Multi-record output semantics become part of the public protocol.

## Known risks and limitations

- Repeated source metadata increases large-stream size.
- A consumer reading a file long after production will often encounter stale
  selectors, by design.
- Capture text omission can require the consumer to reread source after
  validating its digest.
- JSONL users must preserve one complete object per line.

## Outstanding decisions

- Select the stable digest algorithm and textual encoding.
- Define whether a workspace revision is mandatory when a content digest is
  available.
- Define overlap policy for the first regex-substitution consumer.
- Decide whether selector files gain an optional sidecar index without changing
  the line protocol.
