# Roadmap

## 1. Foundation & tooling (complete)

### 1.1. Establish foundation and documentation baseline

- [x] 1.1.1. Set up the project workspace, Continuous Integration and
      Continuous Deployment (CI/CD) pipeline, and core
      dependencies.
- [x] 1.1.2. Normalize parser and Semgrep documentation style and navigation,
      including
      `docs/contents.md` and `docs/repository-layout.md`, as delivered in
      `docs/execplans/sempai-design.md`.

## 2. Core MVP & safety harness foundation

*Goal: Establish the core client/daemon architecture, basic LSP integration,
and the foundational security and verification mechanisms. The MVP must be safe
for write operations from day one.*

### 2.1. Deliver CLI and daemon foundation

*Outcome: Ship a pair of crates (`weaver-cli`, `weaverd`) that honour the
design contract in `docs/weaver-design.md` and expose the lifecycle expected by
`docs/documentation-style-guide.md`.*

- [x] 2.1.1. Define the shared configuration schema for `weaver-cli` and
      `weaverd`
      in `weaver-config`, using `ortho-config` to merge config files,
      environment overrides, and CLI flags for daemon sockets, logging, and the
      capability matrix defaults.
      - Acceptance criteria: Schema documented in crate docs, integration tests
        demonstrate precedence order (file < env < CLI), and default sockets
        align with the design doc.
- [x] 2.1.2. Implement the `weaver-cli` executable as the thin JSON Lines
      (JSONL)
      client that
      initializes configuration via `ortho-config`, exposes the
      `--capabilities` probe, and streams requests to a running daemon over
      standard IO.
      - Acceptance criteria: CLI command surface mirrors the design table,
        capability probe outputs the negotiated matrix, and JSONL framing is
        validated with golden tests.
- [x] 2.1.3. Implement the `weaverd` daemon bootstrap that consumes the shared
      configuration, starts the Semantic Fusion backends lazily, and supervises
      them with structured logging and error reporting.
      - Acceptance criteria: Bootstrap performs health reporting hooks,
        backends start only on demand, and failures propagate as structured
        events.
- [x] 2.1.4. Implement robust daemonisation and process management for
      `weaverd`,
      including backgrounding with `daemonize-me`, PID/lock file handling,
      health checks, and graceful shutdown on signals.
      - Acceptance criteria: Background start creates PID and lock files,
        duplicate starts fail fast, and signal handling shuts down within the
        timeout budget.
- [x] 2.1.5. Provide lifecycle commands in `weaver-cli` (for example,
      `daemon start`,
      `daemon stop`, `daemon status`) that manage the daemon process, verify
      socket availability, and surface actionable errors when start-up fails.
      - Acceptance criteria: Lifecycle commands call into shared helper logic,
        refuse to start when sockets are bound, and emit recovery guidance for
        the operator.

- [x] 2.1.6. Implement the socket listener in `weaverd` to accept client
      connections
      on the configured Unix domain socket (or TCP socket on non-Unix
      platforms).
      - Acceptance criteria: Daemon binds to the socket path from configuration,
        accepts concurrent connections, and gracefully handles connection errors
        without crashing the daemon.

- [x] 2.1.7. Implement the JSONL request dispatch loop in `weaverd` that reads
      `CommandRequest` messages from connected clients, routes them to the
      appropriate domain handler, and streams `CommandResponse` messages back.
      - Acceptance criteria: Request parsing rejects malformed JSONL with
        structured errors, domain routing covers `observe` and `act` commands,
        and responses include the terminal `exit` message with appropriate
        status codes.

- [x] 2.1.8. Wire end-to-end domain command execution from CLI through daemon to
      backend, starting with `observe get-definition` as the first complete
      path.
      - Acceptance criteria: `weaver observe get-definition` with a running
        daemon returns LSP definition results, errors propagate with structured
        messages, and the CLI exits with the daemon-provided status code.

- [x] 2.1.9. Deliver the `weaver-lsp-host` crate with language-server
    initialization, capability detection, and core Language Server Protocol
    (LSP) operations for Rust, Python, and TypeScript.
  - Acceptance criteria: `weaver-lsp-host` initializes and advertises
    capabilities for all three languages; definition, references, and
    diagnostics requests return structured success responses on valid inputs;
    unsupported or pre-initialization requests return deterministic errors; and
    integration tests cover one success case and one failure case per feature.

- [x] 2.1.10. Implement process-based language server adapters for
      `weaver-lsp-host`.
    The `LspHost` currently requires external callers to register
    `LanguageServer` implementations via `register_language()`. This step adds
    concrete adapters that spawn real language server processes (e.g.,
    `rust-analyzer`, `pyrefly`, `tsgo`).
  - Acceptance criteria: `SemanticBackendProvider::start_backend()` registers
    adapters for configured languages, adapters spawn server processes and
    communicate via stdio, server shutdown is handled gracefully on daemon
    stop, and missing server binaries produce clear diagnostic errors.

- [x] 2.1.11. Add human-readable output rendering for commands that return code
    locations or diagnostics, using `miette` or a compatible renderer to
    show context blocks.
  - Acceptance criteria: Definition, reference, diagnostics, and safety
    harness failure outputs include file headers, line-numbered source
    context, and caret spans in human-readable mode; JSONL output remains
    unchanged; missing source content falls back to path-and-range with a
    clear explanation.

- [x] 2.1.12. Deliver the initial `weaver-sandbox` crate with enforced process
    isolation for external tool execution.
  - Acceptance criteria: Linux sandboxing enforces namespaces and seccomp-bpf
    policies via `birdcage`; platform support matrix is documented for Linux
    and non-Linux behaviour; forbidden syscalls and filesystem escapes are
    rejected in tests; and sandbox validation tests run under `make test`.

- [x] 2.1.13. Implement the full "Double-Lock" safety harness logic in
      `weaverd`.
    This is a critical, non-negotiable feature for the MVP. All `act` commands
    must pass through this verification layer before committing to the
    filesystem.
  - Acceptance criteria: Edit transactions pass through syntactic and semantic
    lock validation before commit, failures leave the filesystem untouched,
    and behaviour-driven development (BDD) scenarios cover success, syntactic
    failure, semantic failure, and backend unavailable error paths.

- [x] 2.1.14. Implement atomic edits to ensure that multi-file changes either
      succeed
    or fail as a single transaction.
  - Acceptance criteria: Two-phase commit with prepare (temp files) and commit
    (atomic renames) phases, rollback restores original content on partial
    failure, and new file creation properly tracks file existence for
    rollback.

### 2.2. Deliver baseline command-line interface (CLI) discoverability

*Outcome: Ship baseline guidance in the MVP so first-use command discovery
does* *not require source inspection or external runbooks.*

- [x] 2.2.1. Show short help when `weaver` is invoked without arguments.
      See
      [Level 0](ui-gap-analysis.md#level-0--bare-invocation-weaver)
      and
      [Level 10](ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10d).
  - [x] Replace bare missing-domain output with short help and a clear next
        step.
  - [x] Acceptance criteria: `weaver` with no arguments exits non-zero, prints
        a `Usage:` line, lists the three valid domains (`observe`, `act`,
        `verify`), and includes exactly one pointer to `weaver --help`.
- [ ] 2.2.2. List all domains and operations in top-level help output.
      See
      [Gap 1a](ui-gap-analysis.md#gap-1a--domains-not-enumerated)
      and
      [Gap 1b](ui-gap-analysis.md#gap-1b--operations-not-enumerated).
  - [ ] Add an `after_help` catalogue covering `observe`, `act`, and `verify`
        operations.
  - [ ] Acceptance criteria: `weaver --help` lists all three domains and every
        CLI-supported operation for each domain, and completes without daemon
        startup or socket access.
- [ ] 2.2.3. Add top-level version output and long-form CLI description.
      See
      [Gap 1d](ui-gap-analysis.md#gap-1d--no---version-flag)
      and
      [Gap 1e](ui-gap-analysis.md#gap-1e--no-long-description-or-after-help-text).
  - [ ] Enable clap-provided `--version` and `-V` support.
  - [ ] Add a `long_about` quick-start block aligned with the
        [user's guide](users-guide.md).
  - [ ] Acceptance criteria: `weaver --version` and `weaver -V` both exit 0
        and emit the same version string, and `weaver --help` includes at
        least one runnable quick-start command example.
- [ ] 2.2.4. Provide contextual guidance when a domain is supplied without an
      operation. See
      [Level 2](ui-gap-analysis.md#level-2--domain-without-operation-weaver-observe)
      and
      [Level 10](ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10e).
  - [ ] Print available operations for the provided domain and a follow-up help
        command.
  - [ ] Acceptance criteria: `weaver <domain>` without an operation exits
        non-zero, lists all operations registered for that domain, and includes
        one concrete `weaver <domain> <operation> --help` hint.

### 2.3. Enrich validation and actionable error responses

*Outcome: Ensure MVP error paths fail fast with deterministic, actionable*
*operator guidance before daemon startup and during command routing.*

- [ ] 2.3.1. Validate domains client-side before daemon startup.
      See
      [Level 3](ui-gap-analysis.md#level-3--unknown-domain-weaver-bogus-something)
      and
      [Level 10](ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10b).
  - [ ] Reject unknown domains with a valid-domain list.
  - [ ] Add edit-distance suggestions for close typos.
  - [ ] Acceptance criteria: invalid domains fail before daemon spawn, return
        all three valid domains in the error body, and include a single
        "did you mean" suggestion when edit distance is 2 or less.
- [ ] 2.3.2. Include valid operation alternatives for unknown operations.
      See
      [Level 4](ui-gap-analysis.md#level-4--unknown-operation-weaver-observe-nonexistent)
      and
      [Level 10](ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10c).
  - [ ] Extend daemon and CLI error payloads to include known operations for
        the domain.
  - [ ] Acceptance criteria: unknown-operation errors in both JSON and
        human-readable output include the full known-operation set for the
        domain, with a count equal to the router's `known_operations` length.
- [ ] 2.3.3. Standardize actionable guidance in startup and routing errors.
      See
      [Level 10](ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10a-10e).
  - [ ] Apply a single error template: problem statement, valid alternatives,
        and explicit next command.
  - [ ] Add startup failure guidance for `WEAVERD_BIN` and installation checks.
  - [ ] Acceptance criteria: each Level 10 path (10a through 10e) renders the
        same three-part template (error, alternatives, next command), and
        preserves stable non-zero exit-code semantics.
- [ ] 2.3.4. Return complete argument requirements for `act refactor`.
      See
      [Gap 5b](ui-gap-analysis.md#gap-5b--act-refactor-without-arguments).
  - [ ] List all required flags, valid provider names, and known refactoring
        operations.
  - [ ] Acceptance criteria: `weaver act refactor` without arguments reports
        all three required flags (`--provider`, `--refactoring`, `--file`) in
        one response, plus at least one valid provider and refactoring value.

## 3. Syntactic & relational intelligence

*Goal: Add the Tree-sitter and call graph layers to provide deeper structural*
*and relational understanding of code, and pair this with operation-level help*
*for dependable day-to-day operation.*

### 3.1. Deliver syntax and graph foundations

- [x] 3.1.1. Create the `weaver-syntax` crate and implement the structural
      search
    engine for `observe grep` and `act apply-rewrite`, drawing inspiration from
    ast-grep's pattern language.
  - Acceptance criteria: `observe grep` and `act apply-rewrite` both execute
    through `weaver-syntax`; structural queries return deterministic spans and
    rewrites for Rust, Python, and TypeScript fixtures; invalid query syntax
    returns structured parse diagnostics; and snapshot tests cover success and
    failure paths.

- [x] 3.1.2. Integrate the "Syntactic Lock" from `weaver-syntax` into the
    "Double-Lock" harness.
  - Acceptance criteria: all `act` write paths invoke syntactic verification
    before commit; lock failures prevent on-disk writes; diagnostics include
    file path and source location; and behaviour tests cover pass/fail paths.

- [x] 3.1.3. Extend the `LanguageServer` trait with document sync methods
    (`did_open`, `did_change`, `did_close`) to enable semantic validation
    of modified content at real file paths without writing to disk.
  - Acceptance criteria: trait implementations expose `did_open`,
    `did_change`, and `did_close`; semantic validation paths use in-memory
    document sync instead of disk writes; and integration tests verify
    diagnostics for open-change-close sequences.

- [x] 3.1.4. Create the `weaver-graph` crate and implement the LSP Provider for
      call
    graph generation, using the `textDocument/callHierarchy` request as the
    initial data source.
  - Acceptance criteria: call hierarchy provider returns incoming and outgoing
    edges via `textDocument/callHierarchy`; responses include stable node IDs,
    spans, and relationship direction; provider errors are surfaced as
    structured diagnostics; and end-to-end tests validate graph output.

### 3.2. Expose configuration and operation-level help surfaces

*Outcome: Make configuration and operation help directly discoverable from the*
*CLI without requiring external documentation lookup.*

- [ ] 3.2.1. Surface configuration flags in clap help output.
      See
      [Gap 1c](ui-gap-analysis.md#gap-1c--configuration-flags-invisible)
      and
      [Level 6](ui-gap-analysis.md#level-6--configuration-flags-invisible-in-help).
  - [ ] Register `--config-path`, `--daemon-socket`, `--log-filter`,
        `--log-format`, and `--capability-overrides` as visible global flags.
  - [ ] Acceptance criteria: all five flags appear in both `weaver --help` and
        `weaver daemon start --help`, and existing precedence tests
        (file < env < CLI) continue to pass.
- [ ] 3.2.2. Extend `daemon start` help with config and environment guidance.
      See
      [Level 8](ui-gap-analysis.md#level-8--daemon-subcommand-help).
  - [ ] Document `WEAVERD_BIN` and `WEAVER_FOREGROUND` in `long_about` or
        `after_help`.
  - [ ] Acceptance criteria: `weaver daemon start --help` documents both
        environment variables and includes at least one startup example using
        an override.
- [ ] 3.2.3. Re-enable and extend the `help` subcommand.
      See
      [Gap 1f](ui-gap-analysis.md#gap-1f--help-subcommand-disabled)
      and
      [Level 12](ui-gap-analysis.md#level-12--weaver-help-subcommand).
  - [ ] Remove `disable_help_subcommand = true`.
  - [ ] Support topic help for domains and operations (`weaver help <topic>`).
  - [ ] Acceptance criteria: `weaver help`, `weaver help observe`, and
        `weaver help act refactor` all exit 0 and return topic-specific help
        with no fallback to generic top-level output.
- [ ] 3.2.4. Deliver operation-level help for required arguments.
      Requires 3.2.3. See
      [Gap 5a](ui-gap-analysis.md#gap-5a--observe-get-definition-without-arguments).
  - [ ] Implement nested clap subcommands, or an equivalent schema-backed help
        pipeline, so `weaver <domain> <operation> --help` is operation-specific.
  - [ ] Acceptance criteria: every exposed operation supports
        `weaver <domain> <operation> --help` and each help screen includes
        required flags, argument types, and at least one concrete invocation.
- [ ] 3.2.5. Document ortho-config v0.6.0 behaviour in 3.2 guidance. See
      [ortho-config v0.6.0 migration guide](ortho-config-v0-6-0-migration-guide.md).
  - [ ] Document the new dependency-graph model used by configuration loading
        and precedence resolution.
  - [ ] Document fail-fast discovery behaviour when configuration files exist
        but are invalid.
  - [ ] Document YAML 2.2 parsing semantics via `SaphyrYaml`, including known
        compatibility warnings.
  - [ ] Update internal runbooks and user-facing documentation to reflect
        `ortho-config` v0.6.0 operational behaviour.
  - [ ] Validate documentation quality gates and docs tests after updates.
  - [ ] Acceptance criteria: migration guide, runbooks, and user docs are
        updated with explicit sections for dependency graph, fail-fast
        discovery, and YAML 2.2 semantics; and `make markdownlint`,
        `make fmt`, `make nixie`, and documentation tests pass.

## 4. Query language infrastructure (Sempai)

*Goal: Deliver the Semgrep-compatible query language stack as a standalone
phase* *with explicit parser, backend, and Weaver-integration milestones.*

### 4.1. Deliver Sempai core infrastructure

*Outcome: Implement the Sempai front-end and normalization architecture from*
*`docs/sempai-query-language-design.md`, including YAML parsing, one-liner*
*domain-specific language (DSL) parsing, semantic validation, and stable*
*diagnostic contracts.*

- [ ] 4.1.1. Scaffold `sempai_core` and `sempai` with stable public types and
      facade entrypoints.
  - Acceptance criteria: public API documentation builds for `sempai`, and
    stable types cover language, span, match, capture, and diagnostics models.
- [ ] 4.1.2. Define structured diagnostics with stable `E_SEMPAI_*` error
      codes and report schema.
  - Acceptance criteria: diagnostics include code, message, primary span, and
    notes, and JSON snapshots remain stable across parser and validator paths.
- [ ] 4.1.3. Implement YAML rule parsing via `saphyr` and `serde-saphyr` with
      schema-aligned rule models.
  - Acceptance criteria: rule metadata and query principals parse from
    Semgrep-compatible YAML forms, and parse failures emit structured
    diagnostics.
- [ ] 4.1.4. Implement mode-aware validation for `search`, `extract`, `taint`,
      and `join`, with execution gating to supported modes.
  - Acceptance criteria: unsupported execution modes return deterministic
    `UnsupportedMode` diagnostics, and search mode validation enforces required
    key combinations.
- [ ] 4.1.5. Implement legacy and v2 normalization into one canonical
      `Formula` model with semantic constraint checks. Requires 4.1.3.
  - Acceptance criteria: paired legacy and v2 fixtures normalize to equivalent
    formulas, and semantic invalid states emit deterministic rule diagnostics.
- [ ] 4.1.6. Implement `logos` tokenization and Chumsky Pratt parsing for the
      one-liner DSL with Semgrep precedence mapping.
  - Acceptance criteria: precedence tests match documented binding order, and
    parser output round-trips for supported DSL forms.
- [ ] 4.1.7. Implement DSL error recovery with delimiter anchors and partial
      abstract syntax tree (AST) emission for best-effort diagnostics. Requires
      4.1.6.
  - Acceptance criteria: malformed DSL inputs produce partial parse output and
    labelled diagnostics without parser panics.

### 4.2. Deliver Sempai Tree-sitter backend

*Outcome: Implement the Tree-sitter-backed Sempai execution engine with*
*Semgrep-token rewriting, pattern intermediate representation (IR), formula*
*evaluation, and bounded matching semantics across supported languages.*

- [ ] 4.2.1. Implement language profiles and wrapper registry for Rust, Python,
      TypeScript, and Go, with optional HashiCorp Configuration Language (HCL)
      support.
  - Acceptance criteria: Rust, Python, TypeScript, and Go profiles each define
    wrapper templates, list-shape mappings, and rewrite boundaries; optional
    HCL profile loads only when the feature flag is enabled; profile selection
    failures return deterministic diagnostics; and fixtures validate all profile
    registrations.
- [ ] 4.2.2. Implement Semgrep-token rewrite logic with language-safe boundaries
      for metavariables, ellipsis, and deep ellipsis.
  - Acceptance criteria: rewrite logic avoids substitutions in unsafe lexical
    regions and produces deterministic placeholder mappings.
- [ ] 4.2.3. Compile rewritten snippets into `PatNode`-based pattern IR with
      span traceability.
  - Acceptance criteria: compiled IR snapshots are stable, and wrapper/root
    extraction metadata is preserved for diagnostics.
- [ ] 4.2.4. Implement node-kind matching and metavariable unification over
      Tree-sitter syntax trees.
  - Acceptance criteria: repeated metavariables unify across compatible nodes,
    and mismatches fail deterministically.
- [ ] 4.2.5. Implement list-context ellipsis and ellipsis-variable matching
      using bounded dynamic programming.
  - Acceptance criteria: list-context fixtures pass across supported languages,
    and runtime avoids exponential backtracking.
- [ ] 4.2.6. Implement deep-ellipsis matching with bounded traversal controls.
  - Acceptance criteria: deep matching respects configured node limits and
    returns bounded, deterministic results.
- [ ] 4.2.7. Compile normalized formulas into plan nodes with explicit anchor
      and constraint separation. Requires 4.1.5.
  - Acceptance criteria: conjunction plans enforce positive-term requirements,
    and compiled plan shapes remain snapshot-stable.
- [ ] 4.2.8. Implement conjunction, disjunction, and negative-constraint
      execution semantics.
  - Acceptance criteria: `not`, `inside`, and `anywhere` semantics align with
    documented behaviour and pass regression fixtures.
- [ ] 4.2.9. Implement metavariable `where`-clause constraint evaluation with
      supported and unsupported outcomes.
  - Acceptance criteria: supported constraints execute deterministically, and
    unsupported constraints return stable diagnostic codes.
- [ ] 4.2.10. Implement focus selection plus `as` and `fix` projection
      behaviour in emitted matches.
  - Acceptance criteria: focus and capture projection follow documented
    precedence, and `fix` is surfaced as metadata without direct application.
- [ ] 4.2.11. Implement Tree-sitter query escape hatch with capture-name
      mapping into Semgrep-style capture keys.
  - Acceptance criteria: raw Tree-sitter queries emit normalized captures and
    focus behaviour consistent with Sempai match output contracts.
- [ ] 4.2.12. Add execution safety controls for match caps, capture text caps,
      deep-search bounds, and bounded alternation.
  - Acceptance criteria: safety limits are configurable, deterministic, and
    enforced across execution paths.

### 4.3. Deliver Sempai Weaver integration and readiness

*Outcome: Integrate Sempai into Weaver observe flows with stable command and*
*JSON Lines (JSONL) contracts, cache integration, diagnostics conformance, and*
*release gates for default enablement.*

- [ ] 4.3.1. Add Sempai execution routing in `weaverd` for `observe.query`.
      Requires 4.2.12.
  - Acceptance criteria: daemon execution paths compile and execute Sempai
    plans for supported languages and return structured match streams.
- [ ] 4.3.2. Add `weaver observe query` command surface with `--lang`, `--uri`,
      and `--rule-file|--rule|--q` inputs. Requires 4.3.1.
  - Acceptance criteria: CLI validates input combinations and supports YAML and
    one-liner query workflows with stable error messaging.
- [ ] 4.3.3. Define stable JSONL request and response schemas for Sempai query
      operations, with snapshot coverage. Requires 4.3.2.
  - Acceptance criteria: schema fixtures lock field names and payload shapes,
    and streaming output remains deterministic.
- [ ] 4.3.4. Integrate parse-cache adapter keyed by URI, language, and
      revision, aligned with daemon document lifecycle.
  - Acceptance criteria: cache keys use URI, language, and revision values;
    repeated queries against unchanged revisions hit cache in integration tests;
    revision changes invalidate cached parses deterministically; and cache
    misses and invalidations preserve semantic correctness.
- [ ] 4.3.5. Implement actuation handoff contract using focus-first selection
      with span fallback and optional capture targeting. Requires 4.3.3.
  - Acceptance criteria: downstream `act` commands can consume Sempai output
    deterministically for target selection.
- [ ] 4.3.6. Add diagnostics conformance suites for YAML, DSL, semantic,
      compilation, and execution error categories.
  - Acceptance criteria: each diagnostic category is covered by deterministic
    snapshots and stable `E_SEMPAI_*` error codes.
- [ ] 4.3.7. Add layered quality suites (unit, snapshot, corpus, property, and
      fuzz) for parser and execution behaviour.
  - Acceptance criteria: suites run under repository gates and include
    representative language corpora and malformed-input coverage.
- [ ] 4.3.8. Publish compatibility boundaries for supported operators, modes,
      constraints, and escape-hatch behaviour in user-facing docs.
  - Acceptance criteria: documentation clearly distinguishes supported,
    unsupported, and parse-only behaviours with stable terminology.
- [ ] 4.3.9. Define release gates for enabling Sempai by default, including
      crash-free requirements, diagnostics parity, and documentation parity.
  - Acceptance criteria: release checklist is codified in CI policy and blocks
    default enablement when thresholds are not met.

## 5. Plugin ecosystem & specialist tools

*Goal: Build capability-driven plugin architecture in a dependency-first
order:* *stabilize existing `rename-symbol` implementations, then extend to
new* *capabilities and specialist providers.*

### 5.1. Establish plugin platform foundation

- [x] 5.1.1. Design and implement the `weaver-plugins` crate, including the
      secure
    IPC protocol between the `weaverd` broker and sandboxed plugin processes.
    *(Phase 5.1.1 — see `docs/execplans/3-1-1-weaver-plugins-crate.md`)*
  - Acceptance criteria: plugin broker and sandboxed plugin process establish
    authenticated IPC sessions; request and response envelopes validate against
    crate-level schemas; protocol errors return deterministic failure codes; and
    behaviour tests cover handshake success, schema rejection, and timeout
    cases.

### 5.2. Migrate existing actuator plugins to `rename-symbol` capability

*Outcome: Bring the existing Python and Rust actuator plugins into the new*
*plugin architecture as first-class implementations of the `rename-symbol`*
*capability, with deterministic routing and compatibility guarantees.*

- [x] 5.2.1. Define the `rename-symbol` capability contract for actuator
      plugins, including request schema, response schema, and refusal
      diagnostics. Requires 5.1.1.
  - Acceptance criteria: capability contract is versioned, broker validation
    enforces schema shape, and refusal diagnostics use stable reason codes.
- [ ] 5.2.2. Update `weaver-plugin-rope` manifest and runtime handshake to
      declare and serve `rename-symbol` through the capability interface.
      Requires 5.2.1.
  - Acceptance criteria: plugin advertises `rename-symbol` in capability probes,
    request payloads conform to schema, and response payloads conform to schema.
    Legacy provider routing is not required for Python rename flows.
- [ ] 5.2.3. Update `weaver-plugin-rust-analyzer` manifest and runtime
      handshake to declare and serve `rename-symbol` through the capability
      interface. Requires 5.2.1.
  - Acceptance criteria: plugin advertises `rename-symbol` in capability probes,
    request payloads conform to schema, and response payloads conform to schema.
    Rust rename flows are capability-routed.
- [ ] 5.2.4. Implement daemon capability resolution for `rename-symbol` so
      plugin selection is language-aware and policy-driven. Requires 5.2.2 and
      5.2.3.
  - Acceptance criteria: routing selects the correct plugin per language,
    fallback and refusal paths are deterministic, and routing decisions include
    machine-readable rationale.
- [ ] 5.2.5. Add unit, behavioural, and end-to-end coverage for Python and Rust
      `rename-symbol` under the new capability architecture. Requires 5.2.4.
  - Acceptance criteria: tests cover success paths, refusal paths, and rollback
    guarantees, and both plugins pass shared contract fixtures.
- [ ] 5.2.6. Publish migration notes for `rename-symbol` capability routing and
      deprecate legacy provider-specific command paths. Requires 5.2.5.
  - Acceptance criteria: docs and CLI guidance identify capability-based
    behaviour as the default path, and deprecation messaging is stable.

### 5.3. Deliver capability-first `act extricate`

*Outcome: Implement the cross-language `extricate-symbol` capability model,*
*command contract, and plugin-selection foundation defined in*
*`docs/adr-001-plugin-capability-model-and-act-extricate.md`, including
initial* *Python delivery and shared failure semantics.*

- [ ] 5.3.1. Add capability ID scaffolding and resolver policy for actuator
    capabilities (`rename-symbol`, `extricate-symbol`, `extract-method`,
    `replace-body`, `extract-predicate`). Requires 5.2.4.
  - Acceptance criteria: capability IDs are strongly typed in daemon routing,
    and resolution output includes language, selected provider, and policy
    rationale.
- [ ] 5.3.2. Extend plugin manifest schema and broker loading to support
      capability
    declarations and capability-aware selection.
  - Acceptance criteria: manifest validation enforces capability fields, and
    provider selection respects language plus capability compatibility.
- [ ] 5.3.3. Add the `weaver act extricate --uri --position --to` command
      contract and
    wire capability discovery output for `extricate-symbol`.
  - Acceptance criteria: CLI request shape is stable across providers, and
    capability probe output reports extrication support by language.
- [ ] 5.3.4. Extend the Rope plugin with `extricate-symbol` support for Python.
  - Acceptance criteria: plugin returns unified diffs through existing patch
    application flow and preserves symbol semantics for supported Python shapes.
- [ ] 5.3.5. Extend plugin and daemon failure schemas with deterministic refusal
    diagnostics and hard rollback guarantees.
  - Acceptance criteria: refusal paths emit structured `PluginDiagnostic`
    payloads, include stable error codes, and leave the filesystem unchanged.
- [ ] 5.3.6. Add unit, behavioural, and end-to-end coverage for capability
    resolution and Python extrication baseline paths.
  - Acceptance criteria: tests assert capability negotiation, refusal behaviour,
    incomplete payload failures, and deterministic patch output.

### 5.4. Deliver Rust `extricate-symbol` actuator

*Outcome: Implement Rust `extricate-symbol` as a standalone actuator programme*
*in line with `docs/rust-extricate-actuator-plugin-technical-design.md`, with*
*safe orchestration, deterministic repair loops, and release-grade validation.*

- [ ] 5.4.1. Define Rust extrication orchestration contracts and transaction
      boundaries in `weaverd`, including capability ownership and stage
      interfaces. Requires 5.3.3.
  - Acceptance criteria: stage boundaries are explicit, rollback semantics are
    codified per stage, and orchestration contracts are covered by unit tests.
- [ ] 5.4.2. Implement Rust symbol planning pipeline using rust-analyzer
      definition, references, and call-site discovery for move planning.
      Requires 5.4.1.
  - Acceptance criteria: planner identifies extraction scope deterministically,
    and unsupported symbol shapes emit structured diagnostics.
- [ ] 5.4.3. Implement staged Rust transformation execution via
      `weaver-plugin-rust-analyzer`, including extraction edits, path updates,
      and patch bundling. Requires 5.4.2.
  - Acceptance criteria: staged execution emits unified diffs, preserves
    deterministic operation order, and reports stage-level failures.
- [ ] 5.4.4. Implement import and module-graph repair loops, including ambiguous
      import handling and code-action follow-up passes. Requires 5.4.3.
  - Acceptance criteria: common import breakages are auto-repaired, ambiguous
    repairs return deterministic refusal diagnostics, and no partial writes are
    committed.
- [ ] 5.4.5. Integrate semantic verification and rollback enforcement for Rust
      extrication transactions before commit. Requires 5.4.4 and 5.3.5.
  - Acceptance criteria: semantic lock failures abort the transaction, rollback
    is complete across all touched files, and diagnostics identify failed
    verification stage.
- [ ] 5.4.6. Add Rust-specific unit, behavioural, and end-to-end coverage for
      extrication scenarios, including nested module moves, trait impl updates,
      and macro-adjacent boundaries. Requires 5.4.5.
  - Acceptance criteria: tests assert meaning-preservation probes, module graph
    updates, rollback guarantees, and deterministic failure semantics.
- [ ] 5.4.7. Publish Rust `extricate-symbol` compatibility boundaries and
      operator guidance in docs and capability probe output. Requires 5.4.6.
  - Acceptance criteria: docs and capability surfaces use stable terminology for
    supported, partial, and unsupported Rust shapes.

### 5.5. Deliver additional actuator plugins

*Outcome: Extend actuator coverage beyond `rename-symbol` and*
*`extricate-symbol` with precision syntactic editing support.*

- [ ] 5.5.1. Deliver the `srgn` actuator plugin to provide high-performance,
      precision syntactic editing via capability-routed patch generation.
  - Acceptance criteria: plugin declares capability metadata in its manifest,
    emits deterministic unified diffs for supported edit operations, rejects
    unsupported inputs with structured diagnostics, and passes unit plus
    integration coverage through the plugin broker.

### 5.6. Deliver first specialist sensor plugin

- [ ] 5.6.1. Deliver the `jedi` specialist sensor plugin to provide
      supplementary Python static-analysis signals through the plugin broker.
  - Acceptance criteria: plugin loads through `weaver-plugins`, returns
    deterministic Python analysis payloads for supported files, rejects
    unsupported languages with structured diagnostics, and integration tests
    verify success and refusal paths.

### 5.7. Deliver plugin and capability discoverability coverage

*Outcome: Provide discoverability for plugin inventory and runtime capability*
*negotiation directly from CLI help and introspection commands.*

- [ ] 5.7.1. Add plugin introspection commands.
      See
      [Gap 1g](ui-gap-analysis.md#gap-1g--plugin-listing-absent)
      and
      [Level 7](ui-gap-analysis.md#level-7--plugin-discoverability).
  - [ ] Implement `weaver list-plugins` with `--kind` and `--language`
        filters.
  - [ ] Show plugin name, kind, language support, version, and timeout data.
  - [ ] Acceptance criteria: users can discover valid `act refactor`
        providers from CLI output alone, and table output includes the five
        fields `NAME`, `KIND`, `LANGUAGES`, `VERSION`, and `TIMEOUT`.
- [ ] 5.7.2. Wire plugin introspection into refactor guidance paths.
      Requires 5.7.1. See
      [Gap 5b](ui-gap-analysis.md#gap-5b--act-refactor-without-arguments)
      and
      [Level 7](ui-gap-analysis.md#level-7--plugin-discoverability).
  - [ ] Reference `weaver list-plugins` in refactor-related help and errors.
  - [ ] Acceptance criteria: every provider-related error points users to a
        discoverability command by including the exact string
        `weaver list-plugins`.
- [ ] 5.7.3. Regenerate and validate the manpage from the improved clap model.
      Requires 2.2.2, 3.2.1, and 3.2.3. See
      [Level 11](ui-gap-analysis.md#level-11--manpage).
  - [ ] Verify that domain listings, operation listings, global config flags,
        and help-topic text render in troff output.
  - [ ] Acceptance criteria: generated manpage includes all updated help
        surfaces with no manual post-processing, including three domain
        listings and all five global config flags.

Capability probe discoverability tasks:

- [ ] 5.7.4. Clarify current `--capabilities` output semantics.
      See
      [Level 9](ui-gap-analysis.md#level-9----capabilities-output).
  - [ ] Annotate output and help text that current data represents overrides
        unless runtime capability data is merged.
  - [ ] Acceptance criteria: users can distinguish override configuration from
        runtime-negotiated capability support via an explicit output marker and
        matching help-text note.
- [ ] 5.7.5. Merge runtime capability negotiation into the capabilities probe.
      Requires daemon capability query support. See
      [Level 9](ui-gap-analysis.md#level-9----capabilities-output).
  - [ ] Query daemon-supported capabilities and combine them with configured
        overrides into one matrix.
  - [ ] Acceptance criteria: `weaver --capabilities` returns a complete matrix
        for each configured language and operation, and includes source labels
        for runtime capability versus override values.

### 5.8. Refine graceful degradation guidance

- [ ] 5.8.1. Refine the graceful degradation logic to suggest specific
      plugin-based
    solutions when core LSP features are missing.
  - Acceptance criteria: missing core LSP features produce actionable fallback
    suggestions naming compatible plugins; suggestions include command hints and
    capability rationale; unavailable plugin paths return deterministic
    diagnostics; and regression tests cover at least three degradation
    scenarios.

### 5.9. Deliver static analysis provider integration

- [ ] 5.9.1. Implement the Static Analysis Provider for `weaver-graph` (for
      example, wrapping PyCG) as the first major graph plugin.
  - Acceptance criteria: provider ingests static-analysis call graphs into
    `weaver-graph` with stable node and edge schemas; unsupported languages
    return structured diagnostics; and integration tests validate successful
    ingestion and refusal paths.

## 6. Agent workflows & advanced support

*Goal: Deliver advanced agent-facing workflows after core query and plugin*
*infrastructure is in place, with explicit dependencies on earlier phases.*

### 6.1. Deliver `act apply-patch` command

*Outcome: Provide a safety-locked patch application path that mirrors the*
*`apply_patch` semantics for agents and integrates with the Double-Lock
harness.*

- [x] 6.1.1. Add JSONL request/response types and a `weaver act apply-patch`
      command
    that reads the patch stream from standard input (STDIN) and forwards it to
    the daemon.
  - Acceptance criteria: CLI streams raw patch input, returns non-zero exit
    codes on failure, and surfaces structured errors.
- [x] 6.1.2. Implement the patch parser and matcher in `weaverd` to support
      modify,
    create, and delete operations, including fuzzy matching, line-ending
    normalization, and path traversal checks.
  - Acceptance criteria: patch application is atomic per command, missing
    hunks are rejected, and parent directories are created for new files.
- [x] 6.1.3. Integrate apply-patch with the safety harness using syntactic and
    semantic locks, ensuring no on-disk writes on lock failure.
  - Acceptance criteria: Tree-sitter validates modified/new files, LSP
    diagnostics are compared against the pre-edit baseline, and failures
    leave the filesystem untouched.
- [x] 6.1.4. Add unit, BDD, and end-to-end tests covering create/modify/delete
      and
    failure paths (missing hunk, invalid header, traversal attempt).
  - Acceptance criteria: tests pass under `make test` and error messaging is
    asserted for each failure mode.

### 6.2. Deliver advanced agent workflow foundations

*Outcome: Add onboarding and interactive orchestration paths that build on*
*completed command discoverability and plugin-capability infrastructure.*
*Prerequisites: complete 2.2 and 2.3 for CLI help baselines, and complete 5.2*
*and 5.7 for capability routing plus discoverability surfaces.*

- [ ] 6.2.1. Deliver the `onboard-project` command that orchestrates existing
      Weaver components to generate a deterministic `PROJECT.dna` summary
      artefact.
  - Acceptance criteria: command ingests repository metadata and analysis
    outputs into one `PROJECT.dna` file; output schema is versioned and stable;
    reruns on unchanged inputs produce byte-identical output; and failure paths
    emit structured diagnostics with actionable remediation hints.

- [ ] 6.2.2. Deliver a hybrid interactive mode (`--interactive`) that presents
      lock-failure diffs and diagnostics for explicit human approval or
      rejection before write operations continue.
  - Acceptance criteria: interactive mode displays proposed diff plus syntactic
    and semantic lock diagnostics; approval resumes execution and rejection
    aborts without filesystem changes; timeout or non-interactive environments
    fail closed; and behaviour tests cover approve, reject, and timeout flows.

- [ ] 6.2.3. Deliver the Dynamic Analysis Ingestion provider for
      `weaver-graph` to consume and merge profiling data from tools such as
      `gprof` and `callgrind`.
  - Acceptance criteria: provider ingests at least `gprof` and `callgrind`
    traces into a normalized graph schema; merge logic preserves source
    identity and call-edge attribution; malformed trace inputs return structured
    ingestion diagnostics; and integration tests validate multi-source merges.
