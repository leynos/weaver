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

An ordinary pipe introduces a second hazard. A producer may emit several valid
selector records and then fail. The downstream process receives end-of-file,
but POSIX pipes do not communicate the producer's exit status to that process.
Without an in-band completion signal, an actuator cannot distinguish a complete
filtered result from a plausible-looking prefix of a failed query.

The public machine renderer also needs one collection shape. A JSON array
requires complete buffering, while opaque daemon handles cannot be inspected,
filtered, persisted, or replayed. An untyped `--from-stdin` flag does not tell
the command contract which schema owns standard input.

[RFC 0003](rfcs/0003-sempai-query-to-selector.md) contains the complete
query-to-actuation proposal.

## Decision drivers

- Preserve ordinary shell composition and `jq -c` filtering.
- Stream bounded records without buffering the complete result set in memory.
- Make every selector self-contained and replayable.
- Detect a truncated or failed upstream selector stream before actuation.
- Carry source identity so stale ranges are refused.
- Keep backend and compatibility provenance visible.
- Give standard input an explicit schema owner.
- Define deterministic record ordering.
- Keep zero-, one-, and many-match mutation policy explicit.
- Avoid invented probabilistic confidence values.
- Preserve ADR 007's universal `--json` switch.

## Options considered

### Option A: Completed JSONL streams and `--selectors <path|->`

Emit one selector object per line under `--json`, followed by a terminal
completion record. Consumers accept the same stream from a file or standard
input through a typed `--selectors` option and refuse input without a valid
completion record.

### Option B: Unframed JSONL records

Emit selectors only and rely on end-of-file plus shell `pipefail` behaviour.

### Option C: One JSON array

Buffer and emit the complete collection as one JSON value.

### Option D: Opaque daemon selector handles

Emit compact identifiers which consumers resolve through retained daemon state.

### Option E: Untyped `--from-stdin`

Stream JSONL, but let each command infer what standard input contains.

| Topic | Completed JSONL | Unframed JSONL | JSON array | Opaque handles |
| ----- | --------------- | --------------- | ---------- | -------------- |
| Streaming | Yes | Yes | No | Yes |
| Truncation detection | Yes | No | Yes | Session-bound |
| `jq -c` composition | Natural with pass-through | Natural | Extra iteration | No |
| Replay after restart | Yes | Yes | Yes | No |
| Stale-source evidence | Embedded | Embedded | Embedded | Hidden |
| Input schema discoverability | Explicit | Explicit | Explicit | Handle protocol |

_Table 1: Selector stream alternatives._

## Decision statement

In the context of agent and human pipelines carrying structural matches between
independent processes, facing mutable source, bounded output, backend
replacement, upstream failure, and ambiguous standard-input ownership, Weaver
decides for versioned self-contained JSONL selector records terminated by an
in-band completion record and consumed through `--selectors <path|->`, and
against unframed streams, arrays, opaque daemon handles, or untyped standard
input, to achieve streaming composition, replayability, truncation detection,
deterministic validation, and stale-source refusal, accepting protocol control
records, per-record schema overhead, and the need to define command-specific
cardinality policy.

## Decision outcome and proposed direction

Adopt option A.

For a multi-match resource command, universal `--json` selects deterministic
JSONL. The stream contains zero or more `weaver.selector.v1` records followed
by exactly one `weaver.selector-stream-end.v1` record. Human output remains the
default outside `--json`.

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
  | jq -c '
      if .schema == "weaver.selector.v1" then
        select(.captures.NAME.text | startswith("old_"))
      else
        .
      end
    ' \
  | weaver symbols rename --selectors - --new-name run --dry-run
```

The conditional preserves the completion record while filtering selectors.
A filter that drops or corrupts the completion record causes the consumer to
refuse the stream.

## Selector record version 1 contract

Every selector record contains, directly or through versioned nested objects:

- selector schema identity;
- stream identity and monotonically increasing sequence number;
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

## Completion record version 1 contract

The terminal record contains:

- `schema: "weaver.selector-stream-end.v1"`;
- the stream identity shared by every selector record;
- `complete: true`;
- the producer's emitted selector count;
- the query digest;
- the workspace identity or bounded workspace set;
- producer capability and provider provenance; and
- any bounded truncation metadata caused by an intentional match cap.

The emitted count describes the producer's unfiltered output. A line-oriented
filter may deliberately remove selectors, so consumers do not compare the
received selector count against that value. The completion record attests that
the producer reached a successful terminal state, not that intermediate tools
preserved every selector.

Records after the completion record are invalid. More than one completion
record is invalid. A stream with mismatched stream identities is invalid.

## Ordering and completion

The producer sorts selector records by:

1. canonical URI;
2. start byte;
3. end byte; and
4. selector identifier.

Sequence numbers follow this order. The completion record always comes last.

Zero matches emit only the completion record and return success. A consumer
decides whether zero selectors are acceptable for its operation.

A fatal compile or execution failure may leave a partial prefix in a pipe, but
it does not emit a completion record. A selector-aware actuator reads or spools
the entire bounded stream, validates successful completion, and only then
plans a mutation. It never mutates while selector input is still arriving.

This in-band rule remains necessary even when a shell enables `pipefail`,
because the downstream process cannot portably inspect another process's exit
status.

## Diagnostic separation

Under `--json`:

- selector and completion records go to stdout;
- structured errors go to stderr;
- non-fatal bounded warnings go to stderr; and
- localized prose never appears inside protocol identifiers.

A query with parser error diagnostics emits no completion record and no usable
selector stream, even when Chumsky recovery produced a partial formula.

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

This rule leaves later patch, regex replacement, and Language Server Protocol
(LSP) operations free to consume the same selector protocol without each
inventing a different standard-input switch.

## Relationship to ADR 007

This ADR refines ADR 007's selector and pipeline contract:

- multi-record `--json` results use completed JSONL selector streams;
- typed selector input is `--selectors <path|->`;
- selector-aware actuators validate completion before planning; and
- selectors carry source identity and provenance.

The noun-verb command grammar, human renderer, universal `--json`, and
capability-first actuation decisions remain unchanged.

## Consequences

- Pipeline records are inspectable, persistable, filterable, and replayable.
- Every line carries some repeated schema and source metadata.
- A terminal control record makes truncated upstream output detectable.
- Consumers can reject stale or cross-workspace records without hidden state.
- A query producer can remain independent from a later actuator process.
- Mutation consumers must read the complete bounded stream before acting.
- Filters must preserve the completion record.
- Multi-record output semantics become part of the public protocol.

## Known risks and limitations

- Repeated source metadata increases large-stream size.
- The completion record complicates naive `jq 'select(...)'` filters.
- A consumer reading a file long after production will often encounter stale
  selectors, by design.
- Capture text omission can require the consumer to reread source after
  validating its digest.
- JSONL users must preserve one complete object per line.
- The completion marker detects truncation but does not protect against a
  malicious intermediate process which fabricates protocol records.

Generated help and examples must show completion-preserving filters. A future
`weaver selectors filter` helper may make common transformations less
error-prone without replacing ordinary JSONL tools.

## Outstanding decisions

- Select the stable digest algorithm and textual encoding.
- Define whether a workspace revision is mandatory when a content digest is
  available.
- Define the maximum in-memory selector set before a consumer spools to disk.
- Define overlap policy for the first regex-substitution consumer.
- Decide whether selector files gain an optional sidecar index without changing
  the line protocol.
