# 7.2.3 Emit SARIF run 0 for accepted Type-1 and Type-2 pairs

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

## Purpose / big picture

The requested outcome is a token-pass clone detector that emits SARIF run 0 for
accepted Type-1 and Type-2 clone pairs, with stable fingerprints and accurate
source spans. After implementation, a consumer should be able to run the clone
detector on a stable fixture corpus twice and receive byte-identical SARIF for
unchanged inputs, with accepted pairs represented in `runs[0]` and rejected or
filtered pairs absent from that run.

The intended observable behaviour is:

1. Accepted Type-1 pairs appear in SARIF run 0 with deterministic pair
   identity, deterministic result ordering, and primary plus secondary spans
   that point at the cloned regions.
2. Accepted Type-2 pairs do the same, but remain distinguishable from Type-1
   pairs by rule or metadata chosen in the design document.
3. Rejected candidates, malformed pairs, or pairs missing valid spans do not
   silently produce misleading SARIF results.
4. Unit tests and `rstest-bdd` v0.5.0 behavioural tests cover happy paths,
   unhappy paths, and edge cases around ordering, normalization, and span
   stability.
5. The clone detector design document records the final emission decisions, and
   the relevant roadmap entry is only marked done after implementation plus all
   required gates pass.

This plan is intentionally blocked by a repository mismatch discovered during
drafting: the current checkout is the Weaver workspace, whose `docs/roadmap.md`
does not contain the requested clone-detector 7.2.3 item, and the referenced
Whitaker design documents are absent. The user prompt is treated as
authoritative for the feature intent, but implementation must not begin until
Stage 0 reconciles the source material.

## Constraints

1. The current checkout does not contain the requested Whitaker roadmap entry
   or the cited files `docs/whitaker-clone-detector-design.md` and
   `docs/whitaker-dylint-suite-design.md`. The implementer must treat source
   reconciliation as a hard prerequisite, not as optional housekeeping.
2. Do not invent the final SARIF contract from memory. The authoritative run 0
   mapping, rule taxonomy, and fingerprint shape must come from the intended
   clone-detector design document or an explicit user decision captured in that
   document.
3. The prompt explicitly requires unit tests and behavioural tests using
   `rstest-bdd` v0.5.0. The implementation must follow the fixture and `world`
   conventions described in `docs/rust-testing-with-rstest-fixtures.md` and
   `docs/rstest-bdd-users-guide.md`.
4. The prompt explicitly requires `make check-fmt`, `make lint`, and
   `make test` to succeed. Because Markdown and design documents are also in
   scope, the repository rules additionally require `make fmt`,
   `make markdownlint`, and `make nixie`.
5. Every validation command must be run via the repository's required
   `pipefail` plus `tee` pattern so failures are inspectable after truncation.
6. Design decisions taken while implementing this feature must be copied into
   the authoritative design document rather than left only in code comments or
   commit messages.
7. The relevant roadmap entry must not be marked done until code, tests,
   documentation, and validation are all complete.
8. If the correct repository is confirmed and it is also Rust-based, preserve
   the root `AGENTS.md` rules that already apply here: small files, module
   comments, rustdoc on public APIs, strict Clippy, and en-GB-oxendict prose.
9. This plan must remain self-contained. If later revisions identify the
   correct file paths for the clone detector, they must be written back into
   this document before implementation proceeds further.

## Tolerances

- If Stage 0 cannot identify the intended roadmap entry, design document, and
  implementation surface within one focused reconciliation pass, stop and ask
  the user whether the wrong repository or branch is checked out.
- If the correct implementation requires changing the public SARIF contract
  beyond what the design document describes for run 0, stop and escalate before
  writing code.
- If stable pair fingerprints cannot be derived from already-normalized pair
  identity plus spans, stop and document the missing prerequisite rather than
  improvising an unstable hash.
- If span conversion from the internal region type to SARIF regions is
  ambiguous, stop and capture the competing interpretations in the design doc
  before proceeding.
- If implementation sprawls beyond roughly 15 net files after the correct code
  surface is identified, pause and re-plan to keep the change atomic.
- If the behavioural tests require a new fixture harness or helper crate rather
  than extending an existing test world, document why before proceeding.

## Risks

- Risk: repository mismatch. The largest current risk is not technical but
  contextual: the requested feature appears to belong to a different codebase.
  Mitigation: make reconciliation the first explicit stage and do not spend
  implementation effort until the mismatch is resolved.
- Risk: unstable fingerprints. Pair identities often drift if they depend on
  discovery order, absolute paths, or non-canonical span rendering. Mitigation:
  build fingerprints from a canonical ordered pair identity and normalized span
  data only.
- Risk: incorrect SARIF span projection. Many internal span types are half-open
  or byte-based, while SARIF regions are line-and-column based. Mitigation:
  write focused unit tests for span conversion before wiring full result
  emission.
- Risk: overloading run 0 with too much semantics. If accepted pair emission is
  mixed with later classifier or suppressions work, deterministic output will
  suffer. Mitigation: keep this milestone limited to accepted Type-1 and Type-2
  token-pass pairs only.
- Risk: behaviour tests become thin wrappers over unit tests. Mitigation: use
  `rstest-bdd` scenarios to exercise the user-visible command or report surface
  end to end, not just helper functions.

## Progress

- [x] (2026-03-28 00:00Z) Reviewed `AGENTS.md`, the execplans skill, the
      testing guides, and existing execplan conventions in `docs/execplans/`.
- [x] (2026-03-28 00:00Z) Verified that the current checkout does not contain
      the requested clone-detector roadmap item or the referenced Whitaker
      design documents.
- [x] (2026-03-28 00:00Z) Drafted this ExecPlan with the repository mismatch
      captured as a hard Stage 0 gate.
- [ ] Stage 0: Reconcile the correct roadmap, design docs, and implementation
      surface for the clone detector.
- [ ] Stage 1: Locate the accepted-pair model and the prerequisite from 7.1.1.
- [ ] Stage 2: Lock the SARIF run 0 contract in code and in the design doc.
- [ ] Stage 3: Add failing unit tests and `rstest-bdd` scenarios for emission,
      rejection, stability, and span edge cases.
- [ ] Stage 4: Implement run 0 emission for accepted Type-1 and Type-2 pairs.
- [ ] Stage 5: Update the design document and mark the roadmap item done.
- [ ] Stage 6: Run `make fmt`, `make markdownlint`, `make nixie`,
      `make check-fmt`, `make lint`, and `make test` with logged output.

## Surprises & Discoveries

- The current `docs/roadmap.md` is a Weaver roadmap whose section 7.2 is
  `observe graph-slice`, not a clone detector pipeline. This is reproducible
  with `sed -n '760,900p' docs/roadmap.md`.
- The requested files `docs/whitaker-clone-detector-design.md` and
  `docs/whitaker-dylint-suite-design.md` are absent from this checkout. This is
  reproducible with these searches:

  ```plaintext
  fd -a 'whitaker.*design.*\.md|clone.*design.*\.md' docs
  fd -a 'dylint.*suite.*design.*\.md' docs
  ```

- A repository-wide search for `clone detector`, `SARIF`, `Type-1`, and
  `Type-2` did not reveal an existing clone-detector implementation surface in
  this workspace.
- The root workspace does already pin `rstest-bdd = "0.5.0"`, so if the
  intended code is Rust and belongs in this repository after all, no dependency
  change should be needed for the required behaviour tests.

## Decision Log

- Decision: treat the user prompt as authoritative for the requested feature,
  but treat the current repository contents as authoritative for whether
  implementation can safely start. Rationale: ignoring the mismatch would make
  the plan unsound. Date/Author: 2026-03-28, agent.
- Decision: make source reconciliation a hard Stage 0 gate. Rationale: the
  referenced roadmap and design material are not present, and the execplans
  guidance requires escalation when the task conflicts with observable project
  conventions. Date/Author: 2026-03-28, agent.
- Decision: do not mark any roadmap entry done during this planning-only turn.
  Rationale: the prompt asks for a plan, not implementation, and the current
  roadmap does not contain the requested item. Date/Author: 2026-03-28, agent.
- Decision: require red-green-refactor even for the SARIF emission layer.
  Rationale: stable fingerprints and spans are precisely the sort of detail
  that drift without explicit failing tests. Date/Author: 2026-03-28, agent.

## Outcomes & Retrospective

This draft does not deliver the feature. It delivers a safe implementation plan
for the requested feature while making the current blocker explicit. The main
value of this revision is preventing work from being carried out against the
wrong roadmap and wrong design sources.

Success for a later implementation revision will mean:

1. The intended clone-detector code surface is identified and named in this
   document.
2. Accepted Type-1 and Type-2 token-pass pairs produce deterministic SARIF run
   0 results.
3. Unaccepted or invalid pairs are handled explicitly and tested.
4. The design document records the final emission decisions.
5. The roadmap item is marked done only after all gates pass.

## Context and orientation

The current repository is a Rust workspace named Weaver. Its top-level crates
are listed in `Cargo.toml`, and they include `weaver-cli`, `weaverd`,
`weaver-graph`, `weaver-syntax`, `weaver-lsp-host`, `sempai`, and
`weaver-cards`. There is no `whitaker` crate or clone-detector crate in the
current workspace member list.

Before doing any feature work, reproduce the mismatch so the next implementer
starts from facts rather than assumptions:

```plaintext
sed -n '760,900p' docs/roadmap.md
fd -a 'whitaker.*design.*\.md|clone.*design.*\.md|dylint.*suite.*design.*\.md' docs
rg -n "clone detector|clone-detector|SARIF|Type-1|Type-2|token pass|accepted pair" .
```

At the time this plan was drafted, those commands showed that the current
`docs/roadmap.md` section 7.2 is about `observe graph-slice`, the Whitaker
design docs are absent, and the workspace search did not reveal an existing
clone-detector implementation surface.

If the wrong repository or branch is checked out, stop here and obtain the
correct sources. If the prompt is intentionally asking for future work to be
planned inside this repository, add the missing authoritative documents first,
then revise this ExecPlan so the later stages name the actual files and types.

## Plan of work

### Stage 0: Reconcile the source material

Find the intended roadmap item, the clone detector design document, the Dylint
suite design document, and the prerequisite from 7.1.1. If the current checkout
is wrong, switch to the correct repository or branch before making any other
changes. If the checkout is right but the docs are merely missing, add or
restore those docs first so the emission contract is anchored to something
reviewable.

Completion criteria for Stage 0:

1. `docs/roadmap.md` contains the requested clone-detector 7.2.3 item.
2. The design document section for SARIF emission run 0 is present.
3. The Dylint suite design doc is present if it constrains diagnostics or
   report shape.
4. This ExecPlan is revised with the exact file paths, types, and commands that
   apply to the real implementation surface.

### Stage 1: Locate or establish the accepted-pair domain model

Once the correct codebase is available, identify the types that represent:

1. token-pass candidate pairs,
2. accepted Type-1 and Type-2 pairs,
3. the stable identity or fingerprint inputs from prerequisite 7.1.1, and
4. the span or region type used to point at both members of a pair.

Do not start by writing SARIF JSON directly in the command layer. First, find
the narrowest internal seam where an accepted pair becomes "ready to report".
That seam is where run 0 emission should hang.

By the end of Stage 1, this document should name the concrete modules, for
example an owning crate such as `crates/<clone-detector>/src/token_pass.rs`,
`crates/<clone-detector>/src/sarif/`, and any existing report model or output
writer.

### Stage 2: Lock the SARIF run 0 contract before coding

Read the authoritative SARIF run 0 section and answer these questions in the
design document before implementation:

1. Does run 0 emit one result per accepted pair, and how are Type-1 versus
   Type-2 represented: distinct rules, distinct properties, or both?
2. Which location is primary, and where does the paired location live:
   `locations`, `relatedLocations`, or another approved SARIF field?
3. Which fingerprint fields are mandatory for stability:
   `fingerprints`, `partialFingerprints`, or both?
4. What text goes into the result message, and must it be normalized for byte
   stability?
5. What deterministic ordering rule applies across results and across locations
   inside each result?

Capture the answers in the design document and copy the decision summary into
this ExecPlan's `Decision Log`.

### Stage 3: Add failing tests first

Add focused unit tests around the smallest stable seams, then add behavioural
tests against the public report surface.

Unit tests should cover at least:

1. accepted Type-1 pair maps to one SARIF result in run 0,
2. accepted Type-2 pair maps to one SARIF result in run 0,
3. pair order does not change the emitted fingerprint,
4. result ordering is deterministic across multiple accepted pairs,
5. span conversion is correct at line starts, line ends, multi-line regions,
   and any half-open or byte-based edge cases,
6. rejected or invalid pairs do not emit misleading run 0 results.

Behaviour tests using `rstest-bdd` v0.5.0 should cover at least:

1. a happy Type-1 emission scenario,
2. a happy Type-2 emission scenario,
3. an unhappy scenario where a rejected pair is omitted from run 0,
4. an unhappy scenario where a malformed span or missing prerequisite produces
   the documented failure mode,
5. an edge scenario proving repeat runs over unchanged inputs are stable.

Follow the `world` fixture convention from `docs/rstest-bdd-users-guide.md` and
keep the behaviour steps focused on observable report output rather than on
internal helper state.

### Stage 4: Implement deterministic run 0 emission

Implement the emission in small, testable pieces:

1. add or reuse a canonical pair-identity helper that sorts or otherwise
   normalizes the two clone sides so discovery order cannot affect output,
2. derive the run 0 fingerprint from canonical pair identity, normalized pair
   classification, and normalized span data,
3. convert internal spans to SARIF regions using one dedicated helper rather
   than scattering conversions across serializers,
4. map accepted Type-1 and Type-2 pairs into SARIF result values,
5. ensure non-accepted pairs never reach the emitter,
6. sort emitted results deterministically before serialization.

If the codebase already has a SARIF writer, prefer extending it with a
run-specific helper such as `emit_run0_result(...)` instead of building JSON by
hand in the token pass. If no SARIF abstraction exists yet, introduce one small
module dedicated to run 0 so later runs can compose on top of it without
rewriting the same normalization logic.

### Stage 5: Update documentation as part of the same change

Once tests pass, update the authoritative clone detector design document to
record the final decisions for:

1. result shape,
2. fingerprint inputs,
3. span mapping rules,
4. deterministic ordering rules, and
5. the behaviour for rejected or malformed pairs.

Then update the roadmap entry to mark 7.2.3 done. Do not do this earlier.

### Stage 6: Run the full validation gates

Run all required commands with logged output:

```plaintext
set -o pipefail; make fmt 2>&1 | tee /tmp/7-2-3-make-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/7-2-3-make-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/7-2-3-make-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/7-2-3-make-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/7-2-3-make-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/7-2-3-make-test.log
```

If one fails, inspect the corresponding log, fix the issue, and rerun the
failing command before re-running the full set if the failure could have had
cross-cutting effects.

## Validation notes for the future implementer

When the correct repo and code surface are available, keep the validation loop
strict and observable:

1. Start with the new unit and BDD tests failing for the missing run 0 output.
2. Implement the smallest slice that makes one scenario pass.
3. Expand to the unhappy paths and edge cases.
4. Only then run the full repository gates.

If public APIs or serialization helpers are added, also run doctests or crate
docs as required by the repository's lint setup so `make lint` does not fail on
missing or broken documentation.
