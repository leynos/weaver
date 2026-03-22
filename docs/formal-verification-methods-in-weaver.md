# Formal verification methods in Weaver

## Executive summary

Weaver already isolates the parts of the system where formal methods can add
the most value. The design document defines a Double-Lock safety harness, an
in-memory `VerificationContext`, trait-based syntactic and semantic locks, and
an atomic transaction path with rollback.[^1][^2] The roadmap then requires all
`act` write paths to pass that harness before commit, leave the filesystem
untouched on failure, and apply multi-file edits atomically.[^3]

That structure points to a narrow and practical verification plan:

1. Use **Kani** for small, deterministic kernels in `weaverd`,
   `weaver-plugins`, and, later, `weaver-graph` and Sempai.
2. Use **Verus** only for a very small proof-oriented kernel after the
   transaction and routing contracts stop moving.
3. Keep ordinary testing, behaviour-driven development (BDD), snapshot tests,
   end-to-end tests, and property-based testing for anything that depends on
   external tools or broad behavioural coverage.[^4][^5]

The highest-return phase 1 investment is therefore not "verify Weaver" in the
large. It is to verify the write-safety envelope, patch guardrails, and
capability-routing invariants that Weaver owns directly. Kani's current scope
also excludes concurrency-heavy Rust patterns, which further supports keeping
the initial proof effort inside bounded, deterministic control-plane logic
rather than the daemon's general async orchestration.[^6]

## Current state

Weaver is already a Cargo workspace. The root `Cargo.toml` enumerates workspace
members for `weaver-cli`, `weaverd`, `weaver-config`, `weaver-lsp-host`,
`weaver-sandbox`, `weaver-plugins`, `weaver-graph`, `weaver-syntax`,
`weaver-cards`, `weaver-e2e`, `sempai`, and `sempai-core`.[^7] The repository
layout document assigns the write-operation safety harness to `weaverd`, plugin
protocol and broker logic to `weaver-plugins`, and end-to-end support to
`weaver-e2e`.[^8]

That means Weaver does not need a structural reorganization before formal work
begins. The current crate boundaries already fit a narrow verification effort.

The existing testing stack is also strong enough to support additive formal
methods rather than replacement. The workspace already uses `assert_cmd`,
`insta`, `mockall`, `rstest`, and `rstest-bdd`, and the root `Makefile` already
runs formatting, linting, documentation, and tests across the
workspace.[^7][^9] The design document also describes configurable syntactic
and semantic lock doubles for safety-harness tests, and it describes
`weaver-syntax` as using BDD scenarios and snapshot testing for parser and
validation output.[^1][^4]

The missing pieces are scope and tooling. The top-level `Makefile` currently
has no `kani`, `kani-full`, or `verus` targets, and the GitHub Actions workflow
has no dedicated formal-verification jobs.[^9][^10]

## Recommended verification stack

- **Unit, BDD, snapshot, integration, and end-to-end tests** remain the main
  behavioural safety net.
- **Property-based testing** should cover generated spaces such as path
  normalization, routing-table stability, matching injectivity, and parser
  round-trip constraints where that style is a better fit than example-driven
  tests.[^11][^12]
- **Kani** should provide exhaustive bounded checking for transaction ordering,
  patch application, path guardrails, capability routing, and small graph or
  matching invariants.[^6]
- **Verus** should prove only the smallest set of "must always hold"
  invariants, such as transactional safety, rollback-state preservation, and
  capability-selection soundness.[^13][^14]

The practical rule is simple: use Kani where the production implementation is
already a bounded state machine, use Verus only where a proof-only model is
more maintainable than verifying production code directly, and keep ordinary
tests for everything that crosses trust boundaries.

## Highest-priority proof targets

### Double-Lock transaction kernel

The Double-Lock transaction kernel is the clearest first target. The roadmap
already requires every `act` write path to pass the safety harness before
commit, fail without filesystem writes when verification fails, and apply
multi-file edits atomically.[^3] The design document defines the corresponding
model: `VerificationContext` stores original and modified contents in memory,
`EditTransaction` orchestrates read, edit, validate, and commit,
`SyntacticLock::validate` and `SemanticLock::validate` provide the lock
interfaces, and rollback restores content from the verification context under
stated assumptions.[^1][^2]

That is an excellent Kani target because it is a finite state machine with
explicit success and failure modes. The first harnesses should check:

1. commit is unreachable unless both locks pass,
2. syntactic failure and semantic failure are terminal non-committing states,
3. prepare, commit, and rollback preserve the expected file set for bounded
   create, modify, and delete traces, and
4. lock failure never performs a filesystem write in the verified model.[^6]

### `act apply-patch`

`act apply-patch` is the next strongest Kani target because both the roadmap
and design document define its semantics sharply. The roadmap requires
modify/create/delete operations, fuzzy matching, line-ending normalization,
path traversal checks, and lock-guarded atomic application.[^15] The design
document specifies Git-style patch input over standard input (STDIN), ordered
cursor-based matching, exact matching first and fuzzy matching second, path
normalization against workspace escape, binary patch rejection, and one
safety-critical route through `ContentTransaction` before commit.[^5]

The first practical harnesses should check:

1. ordered `SEARCH`/`REPLACE` blocks never match behind the cursor,
2. failure of any unmatched block aborts the whole command,
3. exact and normalized-fuzzy matching agree on bounded equivalence classes,
4. path normalization never returns a path outside the workspace root, and
5. no lock outcome can commit a partially prepared patch set.[^5][^6]

### Capability routing and refusal diagnostics

The next highest-return target is plugin selection and orchestration
correctness, not plugin semantic correctness. The design and roadmap already
claim that `weaver-plugins` owns the broker and protocol, uses one-shot JSON
Lines (JSONL) payloads, routes successful plugin output back through the
transaction path, and requires deterministic refusal diagnostics and rollback
guarantees.[^8][^16]

The first Kani obligations in this area should cover:

1. manifest-schema helper invariants,
2. language and capability compatibility checks,
3. "selected provider must advertise the requested capability" properties,
4. deterministic refusal-code enumeration, and
5. refusal-or-commit state machines for plugin output that re-enters the
   safety harness.[^16]

### Later graph and matching guardrails

Once the Jacquard work lands, bounded formal checks should extend to graph
budgets and matching guardrails. The roadmap already calls for strict caps on
graph-slice size and token budgets, plus duplicate-name guardrails and default
injective assignment behaviour.[^17][^18] Those are good targets for bounded
counter and matching invariants, not for proving heuristic quality.

### Later Sempai kernels

Sempai becomes a worthwhile formal-method target only after the planned parser
and backend crates exist. At that stage, deterministic language normalization,
metavariable unification, bounded deep-ellipsis search, and matcher invariants
become reasonable candidates for property-based tests plus bounded Kani
harnesses.[^11][^12]

## Contracts that need to be explicit

Formal methods should not become gates until three contracts are stated plainly.

### Atomicity contract

The design document says transactions use two-phase commit with rollback and
that rollback is best-effort under catastrophic failure.[^1][^2] The formal
contract therefore needs to say exactly what assumptions are in scope.

The preferred contract is:

- under normal filesystem assumptions, the transaction commits all changes or
  restores the original state, and
- catastrophic storage or operating-system failures are outside the verified
  model.

### Semantic-lock contract

The semantic lock already compares diagnostics against a pre-edit baseline, but
the documentation should define:

- whether only errors or all severities count,
- how provider-specific severity quirks are normalized,
- whether missing diagnostics become semantic failure or backend-unavailable,
- whether the baseline is per-file or cross-file, and
- whether any nondeterministic provider noise is tolerated.

### Trust boundary

The verified kernel should be called out explicitly:

- transaction ordering,
- path policy,
- capability routing,
- refusal semantics, and
- bounded graph or matching counters.

Trusted or unverified components should also be named explicitly:

- parser correctness,
- Language Server Protocol (LSP) server correctness,
- plugin semantic correctness,
- operating-system and filesystem semantics beyond the chosen model, and
- sandbox enforcement details.[^4][^5]

## Repository layout and tooling

The repository changes for phase 1 should stay small:

```text
.
├── Makefile
├── scripts/
│   ├── install-kani.sh
│   ├── install-verus.sh
│   └── run-verus.sh
├── tools/
│   ├── kani/
│   │   └── VERSION
│   └── verus/
│       ├── VERSION
│       └── SHA256SUMS
├── verus/
│   ├── weaver_proofs.rs
│   ├── transaction_kernel.rs
│   ├── capability_routing.rs
│   └── apply_patch_paths.rs
└── crates/
    ├── weaverd/
    │   └── src/
    │       ├── safety_harness/
    │       │   └── kani.rs
    │       └── dispatch/act/apply_patch/
    │           └── kani.rs
    └── weaver-plugins/
        └── src/
            └── kani.rs
```

Kani harnesses should live next to the production code they verify, guarded
with `#[cfg(kani)]`. That avoids widening public APIs purely for proof access.
Verus proofs should live outside Cargo under `verus/` because the verifier has
its own installation and execution model.[^13][^14]

## Build and CI integration

The recommended `Makefile` additions are:

```make
.PHONY: kani kani-full verus formal formal-pr formal-nightly

kani: ## Run Kani smoke harnesses
	cargo kani -p weaverd --harness verify_transaction_lock_order_smoke
	cargo kani -p weaverd --harness verify_apply_patch_path_guardrails_smoke
	cargo kani -p weaver-plugins --harness verify_capability_resolution_smoke

kani-full: ## Run all Kani harnesses
	cargo kani -p weaverd
	cargo kani -p weaver-plugins

verus: ## Run Verus proofs
	VERUS_BIN="$(VERUS_BIN)" scripts/run-verus.sh

formal-pr: kani

formal-nightly: kani-full verus

formal: formal-pr
```

The Continuous Integration (CI) pipeline should keep the current `build-test`
job intact and add new jobs in stages:

1. `kani-smoke` on every pull request (PR) once the first smoke harnesses land,
2. `verus-proofs` only after the first proof set stabilizes, and
3. a scheduled or manually dispatched slow suite for `kani-full` and the Verus
   proofs.[^10]

The installation flow should stay reproducible and pinned. Kani supports a
Cargo-based installation flow followed by `cargo kani setup`, while Verus is
installed through pinned binary releases and a wrapper script.[^6][^13][^14]

## How formal methods fit the existing tests

Formal methods should extend the existing test stack rather than compete with
it.

- BDD remains the right tool for operator-visible behaviour and error
  reporting.
- Snapshot tests remain the right tool for stable renderer and payload shapes.
- Integration and end-to-end tests remain the right tool for real plugin and
  LSP interactions.
- Property-based tests remain the right tool for larger generated spaces.
- Kani remains the right tool for exhaustive checking within bounded spaces.
- Verus remains the right tool for the smallest proof-only
  contracts.[^4][^11][^12]

That layering keeps the assurance story honest and maintainable.

## Implementation sequence

### Phase 1: infrastructure

1. add `tools/kani/VERSION`,
2. add `tools/verus/VERSION` and `tools/verus/SHA256SUMS`,
3. add `scripts/install-kani.sh`,
4. add `scripts/install-verus.sh`,
5. add `scripts/run-verus.sh`,
6. add `make kani`, `make kani-full`, and `make verus`, and
7. add a `kani-smoke` CI job while keeping Verus manual or nightly at first.

### Phase 2: high-value Kani harnesses

1. add `#[cfg(kani)]` harnesses in `crates/weaverd/src/safety_harness/`,
2. add `#[cfg(kani)]` harnesses in
   `crates/weaverd/src/dispatch/act/apply_patch/`,
3. add `#[cfg(kani)]` harnesses in `crates/weaver-plugins/src/` for capability
   resolution, and
4. add property-based tests around path normalization, refusal-code stability,
   and bounded routing tables.

### Phase 3: contract clarification

1. state the filesystem and rollback assumptions explicitly in the docs,
2. define semantic-lock failure precisely, and
3. define the trust boundary between verified orchestration and trusted
   external tools.

### Phase 4: small Verus kernel

1. create `verus/weaver_proofs.rs`,
2. prove the transaction-gating model,
3. prove rollback restoration over a modelled workspace map, and
4. prove capability-resolution soundness over an abstract resolver.

### Phase 5: later expansion

1. add Kani harnesses for graph-slice budgets after `7.2.5` lands,
2. add Kani harnesses for `max_duplicates` and assignment injectivity after
   `7.4.8` and `7.4.9` land, and
3. add Kani harnesses for Sempai semantic constraints once the planned
   parser/backend crates exist.

## Final recommendation

The most coherent plan for Weaver is:

- Kani first on the Double-Lock transaction kernel, `act apply-patch`, and
  capability-aware routing,
- Verus second on a tiny proof-only model of transactional and routing
  invariants, and
- later Kani harnesses for graph and matching guardrails once those roadmap
  items land.

That sequence pushes the highest bug-finding value to the front, keeps the
developer workflow familiar, and avoids adding more proof machinery than the
current contracts can support.

## References

[^1]: Weaver design document, safety harness and transaction model:
  <https://github.com/leynos/weaver/blob/main/docs/weaver-design.md>
[^2]: Weaver user's guide, two-phase verification:
  <https://github.com/leynos/weaver/blob/main/docs/users-guide.md>
[^3]: Weaver roadmap, safety harness and atomic transaction tasks:
  <https://github.com/leynos/weaver/blob/main/docs/roadmap.md>
[^4]: Weaver design document, testing conventions and behavioural tests:
  <https://github.com/leynos/weaver/blob/main/docs/weaver-design.md>
[^5]: Weaver design document, `act apply-patch` and plugin orchestration:
  <https://github.com/leynos/weaver/blob/main/docs/weaver-design.md>
[^6]: Kani overview, first steps, and limitations:
  <https://model-checking.github.io/kani/>
[^7]: Weaver root `Cargo.toml`:
  <https://github.com/leynos/weaver/blob/main/Cargo.toml>
[^8]: Weaver repository layout:
  <https://github.com/leynos/weaver/blob/main/docs/repository-layout.md>
[^9]: Weaver `Makefile`:
  <https://github.com/leynos/weaver/blob/main/Makefile>
[^10]: Weaver Continuous Integration workflow:
  <https://github.com/leynos/weaver/blob/main/.github/workflows/ci.yml>
[^11]: Sempai query-language design and testing strategy:
  <https://github.com/leynos/weaver/blob/main/docs/sempai-query-language-design.md>
[^12]: Jacquard graph and matching design guidance:
  <https://github.com/leynos/weaver/blob/main/docs/jacquard-card-first-symbol-graph-design.md>
[^13]: Verus guide:
  <https://verus-lang.github.io/verus/guide/>
[^14]: Verus installation instructions:
  <https://github.com/verus-lang/verus/blob/main/INSTALL.md>
[^15]: Weaver roadmap, `act apply-patch` requirements:
  <https://github.com/leynos/weaver/blob/main/docs/roadmap.md>
[^16]: Weaver roadmap and design document, plugin routing and refusal
  diagnostics: <https://github.com/leynos/weaver/blob/main/docs/roadmap.md>
[^17]: Weaver roadmap, graph-slice budget tasks:
  <https://github.com/leynos/weaver/blob/main/docs/roadmap.md>
[^18]: Weaver roadmap, matching guardrail tasks:
  <https://github.com/leynos/weaver/blob/main/docs/roadmap.md>
