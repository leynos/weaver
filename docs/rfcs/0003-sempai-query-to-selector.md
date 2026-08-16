# RFC 0003: Sempai query-to-selector vertical slice

## Preamble

- **RFC number:** 0003
- **Status:** Proposed
- **Created:** 2026-08-16

## 1. Summary

This RFC proposes a deliberately narrow first executable Sempai slice:

```text
positive structural query
    -> versioned selector records
    -> ordinary shell pipeline
    -> an explicit Weaver consumer or actuator
```

The first public producer is `weaver symbols list`. A bare `--query` value is a
positive host-language structural pattern. Rich Sempai expressions use
`--expr`. File and standard-input counterparts support scripts and later
heredoc workflows without embedding shell syntax in Sempai.

The slice compiles positive patterns into the existing canonical Sempai formula
model and executes them initially through an explicitly labelled
`weaver-syntax` compatibility adapter for Rust, Python, and TypeScript. It does
not wait for the complete Semgrep-compatible matcher, every query operator, Go,
optional HashiCorp Configuration Language (HCL), or deep ellipsis.

Matches are emitted as deterministic JSON Lines (JSONL) selector records.
Every selector contains the source identity needed for a downstream command to
reject stale ranges. Mutation commands consume selector streams through an
explicit typed input such as `--selectors -`; they do not depend on hidden
daemon state.

Two architectural decisions refine the public command contract:

- [ADR 011](../adr-011-sempai-query-input-syntax.md) defines `--query`,
  `--expr`, their file forms, and YAML rule inputs; and
- [ADR 012](../adr-012-versioned-selector-streams.md) defines selector JSONL,
  standard-input ownership, deterministic ordering, and stale-source refusal.

## 2. Problem

Sempai has good horizontal foundations but no executable user loop. The facade
can parse, normalize, and validate YAML rules into canonical formula plans.
`Engine::compile_dsl` and `Engine::execute`, however, still return
`NOT_IMPLEMENTED`.

The live roadmap currently reflects the same inversion. It places the public
query command after language profiles, token rewriting, a new pattern
intermediate representation, node matching, bounded ellipsis, deep ellipsis,
boolean execution, constraints, focus projection, a raw Tree-sitter escape
hatch, and execution controls. That sequence postpones the smallest useful
product until most of the full engine exists.

Meanwhile, `weaver-syntax` already provides a narrower structural matcher. It
can compile positive patterns with metavariables, parse Rust, Python, and
TypeScript through Tree-sitter, walk source syntax trees, unify repeated
captures, and return all matching ranges. Its syntax and semantics are not the
complete Sempai contract, but they are enough to prove the first vertical path
if the compatibility boundary remains explicit.

The missing product is therefore not another schema layer. It is this:

```sh
weaver symbols list \
  --lang rust \
  --query 'fn $NAME($...ARGS) { ... }' \
  --json
```

followed by a stable stream that a human, `jq`, another Weaver command, or an
agent can consume.

## 3. Current state

The current implementation has the following relevant properties:

- [`sempai::Engine`](../../crates/sempai/src/engine.rs) compiles YAML search
  rules but stubs both one-liner compilation and execution.
- [`sempai_core::formula`](../../crates/sempai-core/src/formula.rs) already
  provides the canonical query model shared by YAML and one-liner inputs.
- [`sempai_core::DiagnosticReport`](../../crates/sempai-core/src/diagnostic.rs)
  already supports stable `E_SEMPAI_*` codes, byte spans, notes, and several
  diagnostics in one report.
- [`sempai_core::Match`](../../crates/sempai-core/src/match_result.rs) already
  represents a URI, match span, optional focus span, and named captures.
- [`weaver-syntax::Pattern`](../../crates/weaver-syntax/src/pattern.rs) and its
  [matcher](../../crates/weaver-syntax/src/matcher/mod.rs) already provide a
  positive structural matching subset for Rust, Python, and TypeScript.
- The current one-liner design calls for Logos tokenization, a recovering
  Chumsky parser, Pratt precedence, labelled productions, and delimiter-based
  recovery, but the `sempai-dsl` crate does not yet exist.
- ADR 007 already establishes noun-verb commands, universal `--json`, selector
  pipelines, and capability-first actuation.

These foundations should be composed before Sempai grows another horizontal
layer.

## 4. Goals and non-goals

### 4.1. Goals

- Make one positive Sempai query return real matches through the public CLI.
- Preserve one canonical formula and match model across YAML and one-liners.
- Keep the common one-line query free from nested quotation.
- Build the recovering Logos and Chumsky parser without making it a tollbooth
  for bare positive patterns.
- Reuse `weaver-syntax` only behind an explicit compatibility adapter.
- Emit deterministic, bounded, versioned selector records.
- Make selector output safe to carry through `jq`, files, and standard input.
- Ensure actuators reject stale selectors before planning or committing edits.
- Preserve human-readable output while making JSONL the pipeline contract.
- Leave a measured replacement path from the compatibility adapter to the full
  Sempai matcher.

### 4.2. Non-goals

- Claim full Semgrep compatibility in the first executable slice.
- Execute taint, join, or extract modes.
- Implement Go or optional HCL before the first positive query works.
- Implement ordinary ellipsis, deep ellipsis, every `where` constraint, or raw
  Tree-sitter query input in the compatibility executor.
- Apply `fix` directly inside Sempai.
- Let parser recovery silently change the query that executes.
- Define one mutation policy for every actuator.
- Share hidden selector state between producer and consumer.
- Make a probabilistic confidence score where the matcher has no calibrated
  probability model.

## 5. Terminology and invariants

**Pattern query:** A positive host-language structural pattern supplied through
`--query` or `--query-file`.

**Expression query:** A Sempai expression supplied through `--expr` or
`--expr-file`.

**Query plan:** A validated canonical Sempai formula plus language and
provenance.

**Selector record:** A versioned, self-contained description of one match and
its source identity.

**Compatibility executor:** The first positive-pattern executor backed by
`weaver-syntax`, with explicit subset metadata.

**Selector consumer:** A command that validates and reads selector records
without necessarily mutating.

**Selector actuator:** A mutation command that resolves validated selectors
into a shared mutation plan.

The following invariants are normative:

1. Parse recovery may produce additional diagnostics and a partial syntax tree,
   but no query with error-severity diagnostics is executed.
2. A selector is self-contained. Correct consumption does not require daemon
   memory from the producing command.
3. Every selector identifies the source contents or workspace revision against
   which its ranges were calculated.
4. A mutating consumer validates source identity before planning and again at
   the shared mutation engine's commit boundary.
5. Multi-record output has deterministic order.
6. A backend or compatibility fallback is visible in machine provenance.
7. Zero matches are a successful query result, not a parser or execution error.
8. No command guesses whether a multi-match mutation is intended.

## 6. Proposed design

### 6.1. Query input model

The facade gains distinct compilation entry points for the two public input
kinds:

```rust,no_run
pub enum QuerySource<'a> {
    Pattern(&'a str),
    Expression(&'a str),
    RuleYaml(&'a str),
}

impl Engine {
    pub fn compile(
        &self,
        rule_id: &str,
        language: Language,
        source: QuerySource<'_>,
    ) -> Result<QueryPlan, DiagnosticReport>;
}
```

A narrower `compile_pattern` convenience method may accompany `compile` if it
improves the stable facade. The important boundary is that a bare pattern is
lowered directly to `Formula::Atom(Atom::Pattern(...))`. The implementation
must not manufacture an escaped `pattern("...")` string and send it through
the expression parser.

`compile_dsl` may remain as a compatibility convenience for expression input,
but the public API and CLI must retain the input kind.

### 6.2. Bare pattern queries

The common path is:

```sh
weaver symbols list --lang rust --query 'fn $NAME($...ARGS) { ... }'
```

The complete argument is host-language pattern text. It is not shell code and
does not undergo Sempai expression parsing. Shell quoting only protects the
argument from the user's shell.

The first compatibility subset supports:

- ordinary metavariables such as `$NAME`;
- anonymous metavariables such as `$_`; and
- the subset of metavariable ellipsis that can be translated safely to the
  existing `weaver-syntax` representation.

The adapter rejects unsupported or lexically unsafe constructs with stable
diagnostics. It must not reinterpret unsupported public syntax as a different
internal pattern.

### 6.3. Recovering expression parser

Rich formulas use the expression input kind:

```sh
weaver symbols list \
  --lang rust \
  --expr 'pattern("foo($X)") and not(regex("test"))'
```

The `sempai-dsl` crate uses Logos to produce spanned tokens and Chumsky to parse
them. Pratt parsing owns prefix and infix precedence. Recovery anchors include
`)`, `}`, and `,`, with nested-delimiter recovery where appropriate.

Parser implementation requirements are:

- every token retains its byte span;
- productions have human-readable labels;
- string and raw-string errors identify the opening delimiter;
- unexpected operators name the missing expression or delimiter;
- recovery collects more than one independent error where possible;
- recovered nodes retain an explicit recovered or missing state;
- semantic validation may inspect recovered structure for better diagnostics;
- compilation returns an error report when any error-severity parse diagnostic
  remains; and
- the executor never receives a recovered formula as if it were authoritative.

The first executable expression subset may be only a positive `pattern(...)`
atom. The parser may recognize later operators before their executor exists,
but a syntactically valid unsupported expression returns a stable unsupported
diagnostic rather than a misleading empty result.

### 6.4. Compatibility execution

The first `Engine::execute` implementation accepts a canonical positive pattern
plan for Rust, Python, or TypeScript and delegates through a narrow adapter to
`weaver-syntax`.

The adapter owns:

- translation between public Sempai metavariables and internal placeholders;
- validation that translation is safe for the selected language and snippet;
- conversion from `weaver-syntax` match ranges and captures to
  `sempai_core::Match`;
- source and query digest calculation;
- deterministic sorting;
- match and capture-text limits; and
- explicit provider provenance such as `weaver-syntax-compat-v1`.

It does not leak `weaver-syntax`'s internal `$$$NAME` spelling into the public
contract.

The compatibility adapter is temporary. Full Sempai plan execution replaces it
operator by operator. Both paths must pass the same selector schema fixtures so
backend replacement does not change pipeline shape.

### 6.5. Selector record version 1

`weaver symbols list --json` emits one JSON object per line. A representative
record is:

```json
{
  "schema": "weaver.selector.v1",
  "selector_id": "sel_...",
  "workspace_id": "ws_...",
  "uri": "file:///workspace/src/lib.rs",
  "language": "rust",
  "span": {
    "start_byte": 128,
    "end_byte": 176,
    "start": {"line": 8, "column": 1},
    "end": {"line": 10, "column": 2}
  },
  "focus": null,
  "captures": {
    "NAME": {
      "kind": "node",
      "node_kind": "identifier",
      "span": {
        "start_byte": 131,
        "end_byte": 139,
        "start": {"line": 8, "column": 4},
        "end": {"line": 8, "column": 12}
      },
      "text": "dispatch"
    }
  },
  "source": {
    "digest": "sha256:...",
    "workspace_revision": "rev_..."
  },
  "query": {
    "kind": "pattern",
    "digest": "sha256:..."
  },
  "provider": {
    "capability": "symbol.query",
    "id": "weaver-syntax-compat-v1",
    "compatibility": "positive-pattern-subset"
  }
}
```

The exact field spelling is ratified by schema fixtures, but version 1 must
contain:

- schema identity;
- stable selector identity;
- workspace and URI identity;
- language;
- complete match span;
- optional focus span;
- bounded captures with spans and node kinds;
- source content digest and workspace revision where available;
- query kind and digest; and
- capability and provider provenance.

The schema does not emit an invented floating-point confidence value.
Deterministic structural matches instead report evidence and compatibility
provenance. A future calibrated matcher may add a separately versioned score.

Capture text is optional and bounded. Omission includes a reason so consumers
can distinguish a deliberate cap from a missing capture.

### 6.6. Streaming semantics

Machine output follows these rules:

- stdout contains only selector JSONL records;
- records sort by canonical URI, start byte, end byte, then selector identity;
- zero matches produce zero stdout records and exit successfully;
- syntax, validation, execution, or schema errors produce no selectors;
- structured diagnostics go to stderr with a stable non-zero exit class;
- warnings that do not invalidate matches go to stderr and remain bounded;
- no summary object is mixed into the selector stream; and
- `jq -c` and other line-preserving filters may transform or reduce the stream.

Human output may summarize and decorate matches, but it is not the stable
pipeline protocol.

### 6.7. Selector consumption and actuation

A selector-aware command accepts an explicit typed source:

```sh
weaver symbols list --lang rust --query 'fn $NAME($...ARGS)' --json \
  | jq -c 'select(.captures.NAME.text | startswith("old_"))' \
  | weaver symbols rename --selectors - --new-name run
```

`--selectors <path>` reads versioned selector JSONL from a file.
`--selectors -` reads it from standard input. Generic `--from-stdin` is not the
canonical selector flag because it does not identify the expected schema.

A consumer validates:

- selector schema version;
- workspace identity;
- URI and language;
- range ordering and bounds;
- source digest or workspace revision;
- capture and focus references;
- provider compatibility where relevant; and
- command-specific zero-, one-, and many-match policy.

The first mutation consumer should be `weaver symbols rename`, because an LSP
rename naturally consumes a focus or match position. The shared selector
contract remains suitable for later regex substitution, patch templating, and
other LSP operations.

Mutation commands default to refusal when several selectors would make intent
ambiguous. An explicit command-specific option may authorize applying the same
operation to all matches. Filtering the stream is also an explicit policy.

### 6.8. Stale-source handling

A selector is an optimistic reference into mutable source. Its content digest
and workspace revision are preconditions, not informational decorations.

A selector consumer checks source identity before planning. The shared mutation
engine later checks expected base digests immediately before commit. If source
changed, the command returns a structured stale-selector or stale-base refusal
with the affected path, expected identity, observed identity, and a next action
to rerun the query.

`--force` does not erase this precondition. Any override must identify the
specific newly accepted source version.

### 6.9. Heredocs and multiline input

Heredoc support is input plumbing, not a second grammar. File-form flags accept
`-` for standard input:

```sh
weaver symbols list --lang rust --expr-file - --json <<'SEMPAI'
pattern("foo($X)")
and not(
  inside(pattern("tests::$X"))
)
SEMPAI
```

The same parser handles argument, file, and standard-input expression text.
Diagnostics retain byte spans and source labels appropriate to the input
origin.

Because one process owns standard input once, mutually exclusive CLI validation
rejects combinations that request more than one standard-input payload.

## 7. Delivery sequence

### 7.1. Plateau A: positive query library path

- Add the query-source distinction to the facade.
- Implement direct positive-pattern lowering.
- Add the recovering DSL lexer and parser with a positive atom.
- Execute positive patterns through the compatibility adapter.
- Convert matches into `sempai_core::Match`.
- Prove Rust, Python, and TypeScript fixtures.

Observable result: a library caller compiles and executes a positive query.

### 7.2. Plateau B: public selector stream

- Add the ADR 011 input flags to `weaver symbols list`.
- Add the ADR 012 JSONL schema and deterministic renderer.
- Include source identity and provider provenance.
- Add human diagnostics and stable machine diagnostics.
- Prove zero, one, many, malformed, unsupported, and bounded cases.

Observable result: the public CLI emits useful selector records.

### 7.3. Plateau C: pipeline handoff

- Add a selector stream validator shared by consumers.
- Accept `--selectors <path>|-` on the first selector-aware actuator.
- Reject stale, malformed, cross-workspace, and unsupported selectors.
- Prove `symbols list | jq -c | symbols rename --dry-run`.
- Commit only through the shared mutation engine from phase 16.

Observable result: the same selector record crosses a process boundary and
drives a safe mutation plan without hidden state.

### 7.4. Plateau D: replace the compatibility executor

- Land the full Sempai language profiles and pattern intermediate
  representation.
- Replace recursive backtracking where bounded dynamic programming is required.
- Add boolean, context, constraint, focus, and escape-hatch semantics.
- Route eligible plans through the full executor.
- Keep selector schema and ordering conformance unchanged.
- Remove the compatibility adapter when no released subset depends on it.

Observable result: backend maturity increases without changing the user pipe.

## 8. Alternatives considered

### 8.1. Complete the full Sempai backend before exposing a command

This keeps one pure implementation path but delays all user evidence until the
largest horizontal layer is complete. It also fails to test whether the command
and selector contracts are useful before committing to every operator.

Rejected for the first slice.

### 8.2. Expose `weaver-syntax` directly as `--pattern`

This is the smallest implementation, but it creates a second public query
language and leaks temporary syntax into the 0.1.0 contract.

Rejected. The compatibility adapter sits behind the Sempai surface.

### 8.3. Treat `--query` as the full expression DSL

This makes the word query maximally general, but it forces nested quotation on
the common positive-pattern case and makes the recovering Pratt parser a
prerequisite for the smallest useful query.

Rejected by ADR 011.

### 8.4. Auto-detect bare patterns and rich expressions

Auto-detection saves one flag but creates ambiguous error ownership and risks a
future expression keyword changing the interpretation of an existing pattern.

Rejected by ADR 011.

### 8.5. Emit one JSON array

An array is easy to deserialize but prevents natural streaming, increases peak
memory, and makes ordinary line-oriented filters clumsier.

Rejected by ADR 012.

### 8.6. Keep selectors in daemon memory

Opaque handles can be compact, but they do not survive process restart, cannot
be inspected or filtered, and couple a consumer to the producer's daemon
session.

Rejected by ADR 012.

## 9. Risks and mitigations

### 9.1. Compatibility semantics drift

The temporary executor may not match future Sempai semantics exactly.

Mitigation: publish the supported subset, include provider provenance, reject
unsupported constructs, and run identical selector fixtures against both
backends before replacing the adapter.

### 9.2. Unsafe token translation

A naive public-to-internal metavariable rewrite may alter string literals,
comments, or unsupported host-language regions.

Mitigation: use language-aware boundaries where available, reject uncertain
translations, and never silently fall back to raw text replacement.

### 9.3. Parser recovery executes unintended meaning

A recovered syntax tree may differ from the user's intended query.

Mitigation: recovery is diagnostic-only while error-severity diagnostics
remain. Execution requires an unrecovered, validated formula.

### 9.4. Stale selectors target shifted code

A pipeline may pause while another agent or editor changes source files.

Mitigation: source digests and workspace revisions travel with each selector;
consumers and the shared mutation engine enforce them.

### 9.5. Selector records become oversized

Capture text and many matches can flood agent context or pipes.

Mitigation: match caps, capture-text caps, deterministic truncation diagnostics,
and optional text omission are part of the first stream contract.

### 9.6. Two input syntaxes confuse users

Bare patterns and expressions have different quoting and capabilities.

Mitigation: use explicit flags, parallel file forms, generated examples, and
enumerating conflict errors. Never auto-detect.

## 10. Acceptance criteria

The RFC is satisfied when all of the following are true:

1. `Engine` compiles a positive pattern into the canonical formula model.
2. The Logos and Chumsky expression parser returns stable, spanned diagnostics
   and can report more than one recoverable error.
3. Parse recovery never causes an erroneous query to execute.
4. `Engine::execute` returns real Rust, Python, and TypeScript matches for the
   documented compatibility subset.
5. `weaver symbols list --query` emits deterministic human results.
6. `weaver symbols list --query --json` emits one valid
   `weaver.selector.v1` record per match.
7. Zero matches produce no selector records and a successful query exit.
8. Selector records include source identity and visible backend provenance.
9. `--query-file -` and `--expr-file -` accept multiline standard input.
10. Invalid flag combinations enumerate the accepted alternatives.
11. A selector validator accepts producer output after `jq -c` filtering.
12. The first actuator accepts `--selectors -`, rejects stale records, and
    produces a shared mutation plan under `--dry-run`.
13. Full Sempai executor fixtures can replace the compatibility adapter without
    changing selector schema or ordering.
14. Documentation distinguishes implemented, compatibility-subset, parse-only,
    unsupported, and planned behaviour.

## 11. Outstanding decisions

- Select the stable digest algorithm and selector identifier derivation.
- Decide whether query file diagnostics use filesystem paths or file URIs.
- Decide whether a future `--require-match` producer option is useful, while
  retaining successful zero-match semantics by default.
- Select the first non-rename selector consumer after the LSP rename pilot.
