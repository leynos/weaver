# Implement daemon capability resolution for `rename-symbol`

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document must be maintained in accordance with `AGENTS.md` at the
repository root.

Implementation began after approval and is now complete.

## Purpose / big picture

Roadmap item 5.2.4 closes the gap left after 5.2.2 and 5.2.3. The Python rope
plugin and the Rust rust-analyzer plugin both now declare the shared
`rename-symbol` capability, but `weaverd` still routes `act refactor` by an
operator-supplied `--provider` string. After this change, the daemon will
resolve `rename-symbol` by capability, target language, and an explicit routing
policy instead of forcing the operator to know the plugin inventory.

The user-visible outcome is that `weaver act refactor --refactoring rename`
selects the correct provider for supported languages even when `--provider` is
omitted, while an explicit `--provider` remains available as a compatibility
override for now. Routing outcomes become deterministic for success, fallback,
and refusal cases, and every routing decision is emitted with a
machine-readable rationale so tests and future tooling can inspect why a
provider was selected or refused.

Observable success for the eventual implementation:

- Running the Python flow without `--provider` selects `rope` for a
  `*.py` target and still applies the returned diff through the existing
  Double-Lock path.
- Running the Rust flow without `--provider` selects `rust-analyzer` for a
  `*.rs` target and still applies the returned diff through the existing
  Double-Lock path.
- Running the command against an unsupported language or an incompatible
  explicit provider exits non-zero with a deterministic structured refusal.
- JavaScript Object Notation (JSON) mode exposes a machine-readable
  capability-resolution payload that includes at least the requested
  capability, inferred language, selected or refused provider, selection mode,
  and candidate evaluation reasons.
- Human-readable mode does not degrade into raw JSON noise; any new structured
  routing payload is either rendered cleanly by the command-line interface
  (CLI) or otherwise surfaced in a deliberate, documented form.
- `docs/weaver-design.md`, `docs/users-guide.md`, and `docs/roadmap.md`
  reflect the shipped behaviour once implementation is complete.

## Constraints

- The `rename-symbol` capability contract defined in
  `crates/weaver-plugins/src/capability/` is already complete. This roadmap
  item resolves providers in the daemon; it must not redefine the contract
  schema introduced by 5.2.1.
- Keep the CLI command shape stable. `--refactoring rename`, `offset`, and
  `new_name` remain the operator-facing inputs. `--provider` may become
  optional, but it must not be removed in 5.2.4 because 5.2.6 is the roadmap
  item that deprecates legacy provider-specific paths.
- Preserve synchronous execution. Do not introduce async runtimes, async
  traits, or background work queues.
- Preserve the existing safety-critical commit path. Successful plugin output
  must continue to flow through `act apply-patch` and the Double-Lock safety
  harness.
- Respect the repository's file-size rule. Current hotspots are
  `crates/weaverd/src/dispatch/act/refactor/mod.rs` at 398 lines and
  `crates/weaverd/src/dispatch/act/refactor/tests.rs` at 419 lines. Any
  implementation that touches these files must split code rather than grow them
  further.
- Behavioural tests must use `rstest-bdd` v0.5.0 patterns already used in the
  workspace, including mutable fixtures named exactly `world`.
- Comments and documentation must use en-GB-oxendict spelling.
- Lint suppressions remain a last resort. If unavoidable, use tightly scoped
  `#[expect(..., reason = "...")]` rather than `#[allow(...)]`.
- No new external dependencies should be added for this item. Reuse existing
  workspace crates, including `weaver-syntax` if its language-detection helper
  is suitable.
- Any design decision taken during implementation must be recorded in
  `docs/weaver-design.md`, not only in this ExecPlan.
- The final implementation must be validated with unit tests and behavioural
  tests, and the full workspace gates `make check-fmt`, `make lint`, and
  `make test` must pass. Because this item also updates Markdown documents,
  `make fmt`, `make markdownlint`, and `make nixie` must also pass.

## Tolerances (exception triggers)

- Scope: if implementation requires changes to more than 14 files or roughly
  800 net lines, stop and escalate.
- Interface: if satisfying the acceptance criteria requires a breaking change to
  the public `weaver-cli` command syntax or to the public `weaver-plugins`
  request/response contract, stop and escalate.
- Protocol: if machine-readable rationale cannot be delivered cleanly through an
  additive daemon/CLI protocol change, stop and escalate with options before
  inventing an ad hoc prose format.
- Configuration: if making routing truly policy-driven appears to require more
  than one bounded additive configuration surface in `weaver-config`, stop and
  escalate.
- Dependencies: if a new crate dependency appears necessary, stop and escalate.
- Iterations: if `make lint` or `make test` still fail after 5 repair loops,
  stop and escalate with the failing logs and current hypothesis.
- Ambiguity: if roadmap 5.2.4 is interpreted as "remove `--provider`
  immediately" rather than "make provider selection daemon-resolved while
  keeping compatibility", stop and confirm before proceeding.

## Risks

- Risk: the current runtime abstraction takes a provider name directly, which
  means capability resolution cannot be inserted cleanly without refactoring
  the internal seam between the handler and the sandbox runner. Severity: high
  Likelihood: high Mitigation: introduce a dedicated resolution type and split
  "resolve" from "execute" inside `crates/weaverd/src/dispatch/act/refactor/`.

- Risk: machine-readable rationale may tempt an implementation that dumps raw
  JSON onto human stderr output. Severity: high Likelihood: medium Mitigation:
  treat rationale as a first-class payload shape, then update the CLI renderer
  so JSON mode stays machine-readable and human mode stays legible.

- Risk: language inference from file paths can be ambiguous for unsupported or
  extensionless files. Severity: medium Likelihood: medium Mitigation: make the
  unsupported-language path explicit and deterministic rather than guessing;
  cover it with both unit and BDD tests.

- Risk: `crates/weaverd/src/dispatch/act/refactor/tests.rs` already violates
  the stated 400-line limit, so adding more routing assertions there would
  worsen repository health. Severity: medium Likelihood: high Mitigation: split
  the unit tests into focused modules before adding new routing coverage.

- Risk: there is currently no dedicated plugin-routing policy surface in
  `weaver-config`. Severity: medium Likelihood: medium Mitigation: start with a
  bounded daemon-local policy object if it satisfies the roadmap acceptance
  criteria; only widen configuration scope if the lack of an additive config
  field blocks "policy-driven" behaviour.

## Progress

- [x] (2026-03-11) Reviewed `AGENTS.md`, the roadmap entry, the execplans
  skill, and project memory notes relevant to plugin routing and test gates.
- [x] (2026-03-11) Inspected the current `act refactor` handler, plugin
  manifests, plugin registry, CLI daemon-output protocol, and prior 5.2.1 to
  5.2.3 ExecPlans.
- [x] (2026-03-11) Drafted this ExecPlan.
- [x] (2026-03-13) Obtained approval for this ExecPlan.
- [x] (2026-03-13) Extracted the refactor routing and tests into focused
  modules that keep the handler within the repository line-budget rule.
- [x] (2026-03-13) Implemented deterministic `rename-symbol` capability
  resolution in `weaverd`.
- [x] (2026-03-13) Emitted machine-readable routing rationale and updated the
  CLI human renderer.
- [x] (2026-03-13) Added unit and `rstest-bdd` behavioural coverage for
  success, refusal, and override paths.
- [x] (2026-03-13) Updated `docs/weaver-design.md` and `docs/users-guide.md`
  with the shipped behaviour and rationale format.
- [x] (2026-03-13) Ran `make fmt`, `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, and `make test`.
- [x] (2026-03-13) Marked roadmap entry 5.2.4 as done after all gates passed.

## Surprises & Discoveries

- Discovery: `crates/weaverd/src/dispatch/act/refactor/mod.rs` already maps the
  CLI-facing `rename` request to the internal `rename-symbol` operation and
  rewrites `offset` to `position`. The missing work in 5.2.4 is provider
  resolution, not capability-contract translation.

- Discovery: the daemon runtime seam is currently
  `RefactorPluginRuntime::execute(&self, provider, request)`. Because it
  accepts a provider string directly, the resolver must either run before the
  runtime call or the runtime abstraction must be split into resolution and
  execution phases.

- Discovery: the daemon response envelope only has `stream` and `exit` message
  kinds today, and the CLI only understands those two. Machine-readable routing
  rationale therefore needs either an additive protocol extension or a very
  deliberate structured payload inside the existing stream channel.

- Discovery: there is no existing config field in `weaver-config` for
  refactor-plugin routing policy. The only current policy-like config surface
  is the Language Server Protocol (LSP) capability matrix, which is unrelated
  to plugin selection.

- Discovery: `crates/weaverd/src/dispatch/act/refactor/tests.rs` is already
  419 lines long, so test growth must begin with a split rather than more
  in-place additions.

## Decision Log

- Decision: treat 5.2.4 as daemon-side resolution work, not another plugin
  migration. Rationale: 5.2.2 and 5.2.3 already completed the rope and
  rust-analyzer manifest/runtime handshake pieces; the remaining gap is that
  `weaverd` still trusts a raw `--provider` argument. Date: 2026-03-11.

- Decision: preserve `--provider` as an explicit compatibility override in
  5.2.4 while making daemon-driven selection the default when it is omitted.
  Rationale: this satisfies the roadmap's requirement for language-aware,
  policy-driven selection without pre-empting the later migration and
  deprecation work in 5.2.6. Date: 2026-03-11.

- Decision: introduce a first-class resolution result type that captures both
  the selected provider and the reasoning used to reach that outcome.
  Rationale: deterministic routing and machine-readable rationale are easier to
  test and document when they are represented as data rather than reconstructed
  from log strings. Date: 2026-03-11.

- Decision: prefer an additive daemon/CLI payload for routing rationale over
  prose-only stderr. Rationale: the acceptance criteria explicitly require
  machine-readable rationale, and the CLI already has a rendering layer that
  can be extended to keep human output usable. Date: 2026-03-11.

## Context and orientation

The relevant code lives under `crates/weaverd/src/dispatch/act/refactor/`.
Today the handler parses `--provider`, `--refactoring`, and `--file`, reads the
target file, rewrites `rename` into `rename-symbol`, and then calls the runtime
with the operator-supplied provider string. The runtime is backed by
`PluginRunner<SandboxExecutor>` and registers two actuator manifests through
`manifests.rs`:

- `rope` for `python`, declaring `CapabilityId::RenameSymbol`
- `rust-analyzer` for `rust`, declaring `CapabilityId::RenameSymbol`

The plugin registry already supports the lookups needed for resolution:
`find_for_capability()` and `find_for_language_and_capability()` in
`crates/weaver-plugins/src/registry/mod.rs`.

The current missing pieces are:

- There is no language inference step for `act refactor`.
- There is no policy object that turns `(capability, language, explicit
  provider?)` into a deterministic provider choice or refusal.
- There is no structured rationale payload for that decision.
- The CLI does not yet know how to render such a payload cleanly.

The likely files touched by implementation are:

- `crates/weaverd/src/dispatch/act/refactor/mod.rs`
- `crates/weaverd/src/dispatch/act/refactor/manifests.rs`
- one or more new sibling modules such as
  `crates/weaverd/src/dispatch/act/refactor/resolution.rs` and
  `crates/weaverd/src/dispatch/act/refactor/rationale.rs`
- `crates/weaverd/src/dispatch/act/refactor/tests.rs`, likely split into
  smaller modules
- `crates/weaverd/src/dispatch/act/refactor/behaviour.rs`
- `crates/weaverd/tests/features/refactor.feature` or a new focused feature file
- `crates/weaverd/src/dispatch/response.rs`
- `crates/weaver-cli/src/daemon_output.rs`
- `crates/weaver-cli/src/output/mod.rs`
- `crates/weaver-cli/src/output/models.rs`
- `docs/weaver-design.md`
- `docs/users-guide.md`
- `docs/roadmap.md`

## Implementation plan

### Milestone 1: Create a bounded routing surface in `weaverd`

Start by making room for the work. `mod.rs` is already at 398 lines and the
unit-test file is already over the repository's line budget. Extract the new
routing logic into focused modules before adding behaviour:

- Move capability-resolution types and helpers into a new sibling module such
  as `resolution.rs`.
- Split the refactor unit tests into smaller modules, for example one module
  for request-shape tests and one for routing-policy tests.
- Keep `manifests.rs` as the manifest-construction home; do not re-expand
  manifest code back into `mod.rs`.

The target state after this milestone is that `mod.rs` is only orchestration:
parse arguments, infer language, resolve provider, build `PluginRequest`,
execute the selected plugin, and forward diff output.

### Milestone 2: Implement deterministic `rename-symbol` resolution

Introduce a data model that the handler and tests can reason about directly.
The names may vary, but the plan expects three concepts:

- A resolution input value, containing:
  - the effective capability (`rename-symbol`)
  - the target file path
  - the inferred language
  - any explicit provider override from `--provider`
- A resolution outcome value, containing:
  - the selected provider name when routing succeeds
  - a refusal code when routing fails
  - the policy source that produced the outcome
  - candidate-by-candidate evaluation details
- A bounded policy object that turns the input into the outcome.

For 5.2.4, the default policy should be small and explicit:

- Python `rename-symbol` defaults to `rope`
- Rust `rename-symbol` defaults to `rust-analyzer`
- Unsupported or unknown languages refuse deterministically
- An explicit `--provider` is honoured only when that provider exists, supports
  `rename-symbol`, and supports the inferred language; otherwise it produces a
  deterministic refusal rather than silently falling back

Infer the language from the target file path. Reuse an existing helper if that
keeps the dependency surface unchanged and the mapping is deterministic. Do not
guess from plugin names or free-text arguments.

### Milestone 3: Surface machine-readable routing rationale

The acceptance criteria require more than an internal decision object. The
daemon must emit a machine-readable rationale that downstream consumers can
inspect. Implement this as data, not prose.

The cleanest additive shape is a dedicated routing event payload, for example:

```json
{
  "status": "ok",
  "type": "CapabilityResolution",
  "details": {
    "capability": "rename-symbol",
    "language": "python",
    "selected_provider": "rope",
    "selection_mode": "automatic",
    "outcome": "selected",
    "candidates": [
      {
        "provider": "rope",
        "accepted": true,
        "reason": "matched_language_and_capability"
      },
      {
        "provider": "rust-analyzer",
        "accepted": false,
        "reason": "unsupported_language"
      }
    ]
  }
}
```

The exact field names can change, but the payload must remain stable enough for
tests and documentation to refer to it.

Two implementation rules matter here:

- The payload must be emitted for both successful selection and deterministic
  refusal.
- Human mode must remain readable. If the daemon adds a new message kind or a
  new structured stream payload, update the CLI reader and human renderer in
  the same change so users do not see unexplained JSON blobs.

### Milestone 4: Wire the selected provider into plugin execution

Once resolution exists, use it to drive plugin execution:

- Apply language inference and resolution before invoking the runtime.
- Build the existing contract-conforming `PluginRequest` exactly as today for
  `rename-symbol`.
- Execute the chosen provider and preserve the existing diff-to-apply-patch
  handoff.
- Route deterministic refusal outcomes through the same response-writing path
  used for other structured failures.

If the current `RefactorPluginRuntime` trait cannot support this cleanly, split
it into resolution and execution methods or introduce a new internal runtime
input type. This is an internal seam, so refactoring it is acceptable as long
as the operator-facing CLI and the plugin protocol remain stable.

### Milestone 5: Add tests that prove selection, refusal, and rationale

Add both unit and behavioural coverage in `weaverd`. Roadmap 5.2.5 will add
broader end-to-end and shared-contract coverage, but 5.2.4 still needs direct
proof of the resolver behaviour.

Unit tests should cover at least:

- Python file without `--provider` selects `rope`.
- Rust file without `--provider` selects `rust-analyzer`.
- Explicit `--provider rope` for a Rust file is refused deterministically.
- Unsupported language refuses deterministically.
- If multiple candidates exist, the policy order is deterministic.
- The structured rationale includes the selected or refused provider and the
  candidate rejection reasons.

BDD scenarios using `rstest-bdd` v0.5.0 should cover at least:

- Successful Python rename routed by language without `--provider`.
- Successful Rust rename routed by language without `--provider`.
- Refusal when the file extension maps to an unsupported language.
- Refusal when an explicit provider conflicts with the inferred language.
- Emission of machine-readable rationale on both a success path and a refusal
  path.

Prefer a new focused feature file if that keeps the existing refactor feature
readable. Do not keep growing one giant behaviour file.

### Milestone 6: Update documentation and roadmap when the code is done

Once the implementation and tests are green, update the docs in the same change:

- `docs/weaver-design.md`
  Record the final daemon-side routing policy, the rationale payload shape, and
  any compatibility decision about `--provider`.
- `docs/users-guide.md`
  Explain that `rename` routing is now language-aware, describe whether
  `--provider` is optional or override-only, show at least one provider-less
  example, and document the structured rationale visible in JSON mode.
- `docs/roadmap.md`
  Mark 5.2.4 as done only after all tests and gates pass.

## Validation

Implementation is not complete until all of the following commands pass. Run
them with `set -o pipefail` and `tee`, because this environment truncates long
output:

```sh
set -o pipefail; make fmt 2>&1 | tee /tmp/5-2-4-fmt.log
set -o pipefail; make markdownlint 2>&1 | tee /tmp/5-2-4-markdownlint.log
set -o pipefail; make nixie 2>&1 | tee /tmp/5-2-4-nixie.log
set -o pipefail; make check-fmt 2>&1 | tee /tmp/5-2-4-check-fmt.log
set -o pipefail; make lint 2>&1 | tee /tmp/5-2-4-lint.log
set -o pipefail; make test 2>&1 | tee /tmp/5-2-4-test.log
```

In addition to the full gates, capture the most relevant targeted evidence
while iterating:

```sh
cargo test -p weaverd dispatch::act::refactor -- --nocapture
cargo test -p weaverd refactor_behaviour -- --nocapture
```

Expected observable evidence after implementation:

- Provider-less Python and Rust rename scenarios pass.
- The refusal scenarios return status `1` without modifying the target file.
- The emitted routing rationale is stable enough for exact-field assertions in
  unit tests.
- The workspace gates above all exit successfully.

## Outcomes & Retrospective

Completed on 2026-03-13. `weaverd` now resolves `rename-symbol` by capability,
inferred language, and optional explicit provider override before plugin
execution. The handler was split into orchestration plus dedicated
`arguments.rs`, `resolution.rs`, `contract_tests.rs`, and `resolution_tests.rs`
modules so the routing seam is explicit and the line budget stays healthy.

Routing rationale now ships as a structured `CapabilityResolution` envelope on
the existing daemon stream. The payload includes the requested capability,
inferred language when known, optional requested provider, optional selected
provider, selection mode, outcome, stable refusal code, and
candidate-by-candidate reason codes. `weaver-cli` preserves that payload in
JSON mode and renders the same information as concise routing text in human
mode.

The implementation is proven by unit routing tests, request-contract tests, and
behavioural scenarios covering automatic Python routing, automatic Rust
routing, unsupported-language refusal, and explicit provider mismatch refusal.
Follow-on work intentionally deferred to roadmap items 5.2.5 and 5.2.6: broader
end-to-end coverage and migration/deprecation guidance for legacy
provider-specific paths.
