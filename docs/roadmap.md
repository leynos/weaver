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
- [x] 2.2.2. List all domains and operations in top-level help output.
      See
      [Gap 1a](ui-gap-analysis.md#gap-1a--domains-not-enumerated)
      and
      [Gap 1b](ui-gap-analysis.md#gap-1b--operations-not-enumerated).
  - [x] Add an `after_help` catalogue covering `observe`, `act`, and `verify`
        operations.
  - [x] Acceptance criteria: `weaver --help` lists all three domains and every
        CLI-supported operation for each domain, and completes without daemon
        startup or socket access.
- [x] 2.2.3. Add top-level version output and long-form CLI description.
      See
      [Gap 1d](ui-gap-analysis.md#gap-1d--no---version-flag)
      and
      [Gap 1e](ui-gap-analysis.md#gap-1e--no-long-description-or-after-help-text).
  - [x] Enable clap-provided `--version` and `-V` support.
  - [x] Add a `long_about` quick-start block aligned with the
        [user's guide](users-guide.md).
  - [x] Acceptance criteria: `weaver --version` and `weaver -V` both exit 0
        and emit the same version string, and `weaver --help` includes at
        least one runnable quick-start command example, and `make check-fmt`,
        `make markdownlint`, `make fmt`, `make lint`, and `make test` pass.
- [x] 2.2.4. Provide contextual guidance when a domain is supplied without an
      operation. See
      [Level 2](ui-gap-analysis.md#level-2--domain-without-operation-weaver-observe)
      and
      [Level 10](ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10e).
  - [x] Print available operations for the provided domain and a follow-up help
        command.
  - [x] Acceptance criteria: `weaver <domain>` without an operation exits
        non-zero, lists all operations registered for that domain, and includes
        one concrete `weaver <domain> <operation> --help` hint.

### 2.3. Enrich validation and actionable error responses

*Outcome: Ensure MVP error paths fail fast with deterministic, actionable*
*operator guidance before daemon startup and during command routing.*

- [x] 2.3.1. Validate domains client-side before daemon startup.
      See
      [Level 3](ui-gap-analysis.md#level-3--unknown-domain-weaver-bogus-something)
      and
      [Level 10](ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10b).
  - [x] Reject unknown domains with a valid-domain list.
  - [x] Add edit-distance suggestions for close typos.
  - [x] Acceptance criteria: invalid domains fail before daemon spawn, return
        all three valid domains in the error body, and include a single
        "did you mean" suggestion only when exactly one valid domain is within
        edit distance 2.
- [x] 2.3.2. Include valid operation alternatives for unknown operations.
      See
      [Level 4](ui-gap-analysis.md#level-4--unknown-operation-weaver-observe-nonexistent)
      and
      [Level 10](ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10c).
  - [x] Extend daemon and CLI error payloads to include known operations for
        the domain.
  - [x] Acceptance criteria: unknown-operation errors in both JSON and
        human-readable output include the full known-operation set for the
        domain, with a count equal to the router's `known_operations` length.
- [x] 2.3.3. Standardize actionable guidance in startup and routing errors.
      See
      [Level 10](ui-gap-analysis.md#level-10--error-messages-and-exit-codes)
      (10a-10e).
  - [x] Apply a single error template: problem statement, valid alternatives,
        and explicit next command.
  - [x] Add startup failure guidance for `WEAVERD_BIN` and installation checks.
  - [x] Acceptance criteria: each Level 10 path (10a through 10e) renders the
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
*and relational understanding of code, and pair this with operation-level and*
*localized help for dependable day-to-day operation.*

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
      See
      [weaver design §2.1.5](weaver-design.md#215-localized-help-and-reference-surfaces)
      and
      [weaver design §2.3.1](weaver-design.md#231-configuration-contract).
  - [ ] Register `--config-path`, `--daemon-socket`, `--log-filter`,
        `--log-format`, `--capability-overrides`, and `--locale` as visible
        global flags.
  - [ ] Acceptance criteria: all six flags appear in both `weaver --help` and
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
- [x] 3.2.5. Document ortho-config v0.8.0 behaviour in 3.2 guidance. See
      [ortho-config v0.8.0 migration guide](ortho-config-v0-8-0-migration-guide.md).
  - [x] Document the new dependency-graph model used by configuration loading
        and precedence resolution.
  - [x] Document fail-fast discovery behaviour when configuration files exist
        but are invalid.
  - [x] Document YAML 1.2 parsing semantics via `SaphyrYaml`, including known
        compatibility warnings.
  - [x] Update internal runbooks and user-facing documentation to reflect
        `ortho-config` v0.8.0 operational behaviour.
  - [x] Validate documentation quality gates and docs tests after updates.
  - [x] Acceptance criteria: migration guide, runbooks, and user docs are
        updated with explicit sections for dependency graph, fail-fast
        discovery, and YAML 1.2 semantics; and `make markdownlint`,
        `make fmt`, `make nixie`, and documentation tests pass.

### 3.3. Deliver localized CLI and reference outputs

*Outcome: Let operators choose a locale once and receive consistent Fluent-*
*backed help, error text, and generated reference artefacts across Weaver.*

- [ ] 3.3.1. Introduce locale selection in the shared configuration contract.
      See
      [weaver design §2.1.5](weaver-design.md#215-localized-help-and-reference-surfaces)
      and
      [weaver design §2.3.1](weaver-design.md#231-configuration-contract).
  - [ ] Add a `locale` field to `weaver-config`, surfaced as `--locale`,
        `WEAVER_LOCALE`, and a config-file key.
  - [ ] Add a pre-config bootstrap pass that honours `--locale` and
        `WEAVER_LOCALE` before consulting `LC_ALL`, `LC_MESSAGES`, and `LANG`,
        then rebuild the localizer if the resolved config locale differs.
  - [ ] Acceptance criteria: `weaver --help` and other pre-config clap display
        paths honour `--locale` and `WEAVER_LOCALE` immediately; malformed
        `--locale` and `WEAVER_LOCALE` values fail fast; malformed ambient
        `LC_*` or `LANG` values warn and fall back; file-backed locale
        settings apply after full config loading; and `en-US` remains the
        guaranteed fallback.
- [ ] 3.3.2. Localize clap help and parse errors through `ortho_config`.
      Requires 3.3.1. See
      [weaver design §2.1.5](weaver-design.md#215-localized-help-and-reference-surfaces).
  - [ ] Drive clap help with `Cli::command().localize(&localizer)` and route
        parse failures through `localize_clap_error_with_command`.
  - [ ] Move bare-invocation help, lifecycle guidance, and other manual
        operator text to Fluent message IDs with argument-aware rendering.
  - [ ] Acceptance criteria: bare invocation, `weaver --help`, and common clap
        validation failures render translated copy for supported locales
        without changing existing exit-code semantics.
- [ ] 3.3.3. Centralize the localized command and operation catalogue.
      Requires 2.2.4 and 3.3.2. See
      [weaver design §2.1.5](weaver-design.md#215-localized-help-and-reference-surfaces).
  - [ ] Replace duplicated domain and operation tables with one structured
        catalogue shared by the router, contextual-help renderer, and test
        fixtures.
  - [ ] Store message IDs, examples, and operation descriptions in that
        catalogue instead of hard-coded padded English strings.
  - [ ] Acceptance criteria: top-level help, domain-without-operation
        guidance, and `weaver help <topic>` all read from the same catalogue,
        and adding a new operation requires one metadata change rather than
        parallel edits in help, tests, and routing.
- [ ] 3.3.4. Generate localized reference artefacts from ortho-config
      metadata. Requires 3.2.4 and 3.3.2. See
      [weaver design §2.1.5](weaver-design.md#215-localized-help-and-reference-surfaces).
  - [ ] Add stable documentation IDs to config-backed fields and expose
        `OrthoConfigDocs` metadata for the generated help schema.
  - [ ] Add `[package.metadata.ortho_config]` wiring and `cargo orthohelp`
        generation for at least `en-US` and one secondary locale.
  - [ ] Acceptance criteria: localized intermediate representation (IR) files
        are generated per locale, `en-US` manpage packaging remains intact,
        and artefact validation proves the generated help text matches the
        runtime Fluent catalogue.

## 4. Query language infrastructure (Sempai)

*Goal: Deliver the Semgrep-compatible query language stack as a standalone
phase* *with explicit parser, backend, and Weaver-integration milestones.*

### 4.1. Deliver Sempai core infrastructure

*Outcome: Implement the Sempai front-end and normalization architecture from*
*`docs/sempai-query-language-design.md`, including YAML parsing, one-liner*
*domain-specific language (DSL) parsing, semantic validation, and stable*
*diagnostic contracts.*

- [x] 4.1.1. Scaffold `sempai_core` and `sempai` with stable public types and
      facade entrypoints.
  - Acceptance criteria: public API documentation builds for `sempai`, and
    stable types cover language, span, match, capture, and diagnostics models.
- [x] 4.1.2. Define structured diagnostics with stable `E_SEMPAI_*` error
      codes and report schema.
  - Acceptance criteria: diagnostics include code, message, primary span, and
    notes, and JSON snapshots remain stable across parser and validator paths.
- [x] 4.1.3. Implement YAML rule parsing via `saphyr` and `serde-saphyr` with
      schema-aligned rule models.
  - Acceptance criteria: rule metadata and query principals parse from
    Semgrep-compatible YAML forms, and parse failures emit structured
    diagnostics.
- [x] 4.1.4. Implement mode-aware validation for `search`, `extract`, `taint`,
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
- [x] 5.2.2. Update `weaver-plugin-rope` manifest and runtime handshake to
      declare and serve `rename-symbol` through the capability interface.
      Requires 5.2.1.
  - Acceptance criteria: plugin advertises `rename-symbol` in capability probes,
    request payloads conform to schema, and response payloads conform to schema.
    Legacy provider routing is not required for Python rename flows.
- [x] 5.2.3. Update `weaver-plugin-rust-analyzer` manifest and runtime
      handshake to declare and serve `rename-symbol` through the capability
      interface. Requires 5.2.1.
  - Acceptance criteria: plugin advertises `rename-symbol` in capability probes,
    request payloads conform to schema, and response payloads conform to schema.
    Rust rename flows are capability-routed.
- [x] 5.2.4. Implement daemon capability resolution for `rename-symbol` so
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
- [ ] 5.7.3. Regenerate and validate localized manpages from the schema-backed
      help model. Requires 2.2.2, 3.2.1, 3.2.3, and 3.3.4. See
      [Level 11](ui-gap-analysis.md#level-11--manpage).
      See
      [weaver design §2.1.5](weaver-design.md#215-localized-help-and-reference-surfaces).
  - [ ] Verify that domain listings, operation listings, global config flags,
        locale-aware help text, and help-topic content render in troff output.
  - [ ] Acceptance criteria: generated manpages include all updated help
        surfaces with no manual post-processing, ship `en-US` by default, and
        can be emitted for additional supported locales including all six
        global config flags.

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

## 7. Cards-first symbol context (Jacquard)

*Goal: Deliver small, structured “symbol cards” and bounded symbol graph slices
as first-class `observe` operations, then extend them to deterministic,
budgeted history diffs over recent commits. This phase operationalizes the
design in
[`docs/jacquard-card-first-symbol-graph-design.md`](jacquard-card-first-symbol-graph-design.md)
 within Weaver’s existing Semantic Fusion architecture.*

### 7.1. Deliver `observe get-card` (Tree-sitter first)

*Outcome: Provide a deterministic, cacheable symbol card payload that defaults
to Tree-sitter extraction and optionally enriches via LSP when available. See
`docs/jacquard-card-first-symbol-graph-design.md` §9.1-§9.3 and §10.1-§10.3.*

- [x] 7.1.1. Define stable JSONL request and response schemas for
      `observe get-card`, including versioning, provenance fields, and
      progressive detail levels. Requires 2.1.7 and 3.1.1.
  - [x] Include attachment bundling and interstitial payloads in the schema
        (doc comments, decorators, import blocks, and bundle rules).
  - [x] Add schema fixtures and snapshot coverage for success and refusal
        payloads.
  - [x] Acceptance criteria: schema fixtures lock field names and payload
        shapes, including attachments and interstitials; responses include
        provenance for non-trivial fields; and the default output is stable
        (byte-identical) for unchanged inputs.
- [x] 7.1.2. Implement Tree-sitter symbol card extraction for the initial
      supported languages (Rust, Python, and TypeScript). Requires 3.1.1.
  - [x] Add an entity/interstitial region pass and attach interstitials to the
        relevant cards (file/module or interstitial cards).
  - [x] Bundle doc comments and decorator/annotation blocks onto symbol cards
        using deterministic backwards-scanning rules.
  - [x] Enforce nested entity filtering so locals/closures do not enter the
        entity table by default.
  - [x] Acceptance criteria: unit tests cover at least three symbol kinds per
        language; extracted ranges are deterministic; comment/decorator
        bundling is stable under whitespace edits; nested locals never appear
        as entities; and whitespace-only edits do not change `SymbolId`
        fingerprints.
- [x] 7.1.3. Implement optional LSP enrichment for `observe get-card` when
      `--detail semantic` (or higher) is requested. Requires 2.1.9 and 3.1.3.
  - [x] Enrich cards with hover/type and deprecation metadata where supported.
  - [x] Acceptance criteria: enrichment is gated by capability negotiation; LSP
        unavailability degrades to the Tree-sitter-only card with explicit
        provenance; and integration tests cover both enriched and degraded
        behaviour.
- [x] 7.1.4. Add cache integration for card extraction keyed by URI, language,
      and document revision. Requires 7.1.2.
  - [x] Reuse Tree-sitter parser registries and cache extracted entity tables
        with an LRU (Least Recently Used) policy keyed by repo, ref, file path,
        and blob hash.
  - [x] Avoid unnecessary string cloning in card and region extraction; prefer
        borrowing or interning for hot paths.
  - [x] Acceptance criteria: repeated `get-card` requests for unchanged
        revisions hit cache in integration tests; revision changes invalidate
        deterministically; and cache misses preserve correctness.

### 7.2. Deliver `observe graph-slice` (budgeted traversal)

*Outcome: Return a bounded subgraph rooted at an entry symbol, with typed edges
(`call`, `import`, and `config`) and explicit budget constraints. See
`docs/jacquard-card-first-symbol-graph-design.md` §12.1-§12.3.*

- [x] 7.2.1. Define stable JSONL request and response schemas for
      `observe graph-slice`, including budgets, spillover metadata, and
      provenance for edges. Requires 2.1.7.
  - [x] Acceptance criteria: schema fixtures lock `budget` semantics and
    default values; responses are deterministic for a fixed repo revision; and
    spillover metadata is present when traversal is truncated; and edges carry
    resolution scope (`full_symbol_table`, `partial_symbol_table`, or `lsp`).
- [ ] 7.2.2. Implement a two-pass Tree-sitter extraction pipeline that builds a
      symbol table before resolving edges.
  - [ ] Acceptance criteria: edge extraction uses a full or partial symbol
    table and marks resolution scope on each edge; unresolved references are
    preserved as external nodes with explicit confidence.
- [ ] 7.2.3. Implement call-edge slice expansion using the existing LSP call
      hierarchy provider via `weaver-graph`. Requires 3.1.4 and 2.1.9.
  - [ ] Acceptance criteria: `call` edges include explicit provenance; depth
    limits are enforced; and end-to-end tests validate a depth-2 traversal on a
    fixture repository.
- [ ] 7.2.4. Implement baseline `import` and `config` edge extraction using
      Tree-sitter interstitial passes and per-language queries. Requires 3.1.1.
  - [ ] Acceptance criteria: extracted edges include confidence values and
    provenance; edge extraction is bounded by the slice budget; and at least
    one test per language asserts both `import` and `config` edge behaviour.
- [ ] 7.2.5. Implement budgeted traversal using a priority-queue expansion
      strategy with explicit `max_cards`, `max_edges`, and
      `max_estimated_tokens` enforcement.
  - [ ] Acceptance criteria: traversal never exceeds configured caps; rejection
    reasons are emitted when `--debug` is enabled; and behaviour-driven
    development (BDD) tests cover fan-out explosion and budget truncation
    cases.

### 7.3. Deliver `observe graph-history` in `snapshots_on_demand` mode

*Outcome: Diff a slice over the last N commits without requiring a working tree
checkout, producing deterministic output suitable for caching and regression
tests. See `docs/jacquard-card-first-symbol-graph-design.md` §13.1-§13.2 and
§22.*

- [ ] 7.3.1. Implement git-backed blob loading for historical revisions without
      checkout, scoped to only the files required by the slice budget.
  - [ ] Add explicit operational limits for blob size, parse time per file,
        total files per commit, and partial-parse thresholds, with fallback
        reasons recorded in the output (`timeout`, `blob_too_large`,
        `partial_parse`, `unsupported_grammar`).
  - [ ] Acceptance criteria: history queries never invoke `git checkout`;
    missing blobs return structured diagnostics; and the file loader is covered
    by unit tests for typical path and revision scenarios.
- [ ] 7.3.2. Implement slice reconstruction per commit with explicit data
      quality metadata and partial symbol table resolution. Requires 7.2.5.
  - [ ] Acceptance criteria: `--commits 5` returns a stable set of commits and
    per-commit slice payloads with `quality.resolution_scope` and
    `quality.fallbacks`; delta payloads include added/removed/changed nodes and
    edges; and BDD tests validate output against a curated git fixture
    repository.
- [ ] 7.3.3. Implement delta computation normalization and change taxonomy
      classification for nodes and edges.
  - [ ] Treat import blocks and decorators as commutative sets for deltas, and
        persist normalized representations alongside raw text.
  - [ ] Acceptance criteria: import/decorator reordering is classified as
        `text` change; taxonomy output includes confidence; and fixtures cover
        comment-only and signature-only edits.
- [ ] 7.3.4. Implement semantic risk warnings on history deltas for
      dependency/dependent changes in the slice neighbourhood.
  - [ ] Expose `--warning-depth` to widen the dependency neighbourhood scanned
        for warnings.
  - [ ] Acceptance criteria: warnings include edge paths and confidence;
    `text`-only deltas emit lower-risk warnings; and curated fixtures validate
    both warning types.
- [ ] 7.3.5. Implement history-mode gating and safe defaults, with LSP
      enrichment disabled by default for history queries.
  - [ ] Acceptance criteria: default mode uses Tree-sitter-only extraction for
    historical commits; enabling enrichment is explicit and documented; and
    degraded behaviour is made visible via provenance fields.

### 7.4. Deliver probabilistic matching and “reason codes”

*Outcome: Map symbols across commits probabilistically when identifiers drift,
exposing confidence and alternates rather than hiding ambiguity. See
`docs/jacquard-card-first-symbol-graph-design.md` §14.1-§14.8.*

- [ ] 7.4.1. Implement phase 1 stable-identity matching (type, name, container,
      file hint), with explicit confidence output.
  - [ ] Acceptance criteria: match outputs include the winning phase and
    confidence; non-matching candidates are rejected rather than forced; and
    fixtures include rename/move cases that must not match in phase 1.
- [ ] 7.4.2. Implement phase 2 body-hash matching for rename detection.
      Requires 7.4.1.
  - [ ] Acceptance criteria: match outputs include the winning phase and
    confidence; low-confidence matches are rejected rather than forced; and
    fixtures cover rename scenarios with unchanged bodies.
- [ ] 7.4.3. Implement phase 3 structural-hash matching on AST-normalized
      shapes. Requires 7.4.2.
  - [ ] Acceptance criteria: match outputs include the winning phase and
    confidence; low-confidence matches are rejected rather than forced; and
    fixtures cover move scenarios with formatting-only edits.
- [ ] 7.4.4. Implement phase 4 fuzzy similarity matching (token overlap and
      shingles). Requires 7.4.3.
  - [ ] Acceptance criteria: match outputs include the winning phase and
    confidence; low-confidence matches are rejected rather than forced; and
    fixtures cover rename and move scenarios with minor body edits.
- [ ] 7.4.5. Implement phase 5 graph refinement and global assignment
      refinement. Requires 7.4.4.
  - [ ] Acceptance criteria: match outputs include the winning phase and
    confidence; low-confidence matches are rejected rather than forced; and
    fixtures cover rename/move scenarios resolved by neighbourhood evidence.
- [ ] 7.4.6. Implement feature extraction for cross-commit matching using
      signature, AST-shape, docstring fingerprints, attachments, and
      neighbourhood sketches.
  - [ ] Acceptance criteria: feature extraction is deterministic for identical
    inputs; unit tests cover feature stability under whitespace-only edits and
    alpha-renaming of locals; and failures emit structured diagnostics.
- [ ] 7.4.7. Implement candidate generation and scoring with calibrated
      probabilities, emitting top-K alternates and “reason codes”. Requires
      7.4.6.
  - [ ] Acceptance criteria: response payloads always include `best_match` plus
    alternates up to the requested cap; reason codes are stable enumerations;
    and debug output surfaces the top contributing features.
- [ ] 7.4.8. Implement duplicate-name guardrails (`max_duplicates`) that force
      ambiguous mappings or fallback matching when homonyms explode.
  - [ ] Acceptance criteria: `--max-duplicates` returns explicit “ambiguous
    mapping” responses; observability counters capture guardrail triggers; and
    fixtures with same-name functions avoid false renames.
- [ ] 7.4.9. Implement assignment across the slice using a solver that avoids
      mapping multiple sources to one target unless explicitly enabled.
  - [ ] Acceptance criteria: property tests prevent illegal many-to-one
    mappings by default; a feature flag enables split/merge experimentation;
    and deterministic test fixtures cover rename and move scenarios.

### 7.5. Optional ledger cache and richer edge types

*Outcome: Add a persisted ledger keyed by commit hash for faster history
queries and broader edge coverage once `snapshots_on_demand` is proven
reliable. This step is intentionally staged behind the on-demand
implementation. See `docs/jacquard-card-first-symbol-graph-design.md` §13.2 and
§18.1-§18.2.*

- [ ] 7.5.1. Define a versioned on-disk ledger format for cards, edges, and
      deltas keyed by commit hash. Requires 7.3.2.
  - [ ] Acceptance criteria: format is forward-compatible via explicit version
    fields; corruption is detected with checksums; and schema changes are gated
    behind migrations.
- [ ] 7.5.2. Implement incremental ledger population and invalidation rules.
  - [ ] Acceptance criteria: ledger writes are atomic; invalidation occurs when
    inputs change; and performance benchmarks show a measurable improvement for
    repeated history queries.

## 8. Formal verification and proof tooling

*Goal: Add bounded formal verification checks for Weaver-owned transactional,*
*patching, routing, and guardrail invariants without replacing the existing*
*test stack. See `docs/formal-verification-methods-in-weaver.md`.*

### 8.1. Establish formal verification tooling

*Outcome: Add pinned verifier installation, explicit make targets, and staged*
*Continuous Integration (CI) entry points for Kani and Verus.*

- [ ] 8.1.1. Add pinned verifier version files and install scripts for Kani and
      Verus. See `docs/formal-verification-methods-in-weaver.md`
      "Repository layout and tooling".
  - [ ] Add `tools/kani/VERSION`.
  - [ ] Add `tools/verus/VERSION` and `tools/verus/SHA256SUMS`.
  - [ ] Add `scripts/install-kani.sh`, `scripts/install-verus.sh`, and
        `scripts/run-verus.sh`.
  - [ ] Acceptance criteria: local installs are reproducible from pinned
        versions, scripts fail fast on version or checksum mismatch, and the
        normal Rust toolchain workflow remains unchanged unless a formal target
        is invoked.
- [ ] 8.1.2. Add explicit `make kani`, `make kani-full`, `make verus`,
      `make formal-pr`, and `make formal-nightly` targets. Requires 8.1.1.
  - [ ] Keep the Kani smoke harness list explicit rather than scan-based.
  - [ ] Keep Verus execution outside Cargo through `scripts/run-verus.sh`.
  - [ ] Acceptance criteria: `make kani` runs only smoke harnesses,
        `make kani-full` runs all checked-in Kani harnesses, `make verus`
        executes the proof entrypoint, and the new targets are documented in
        the `Makefile`.
- [ ] 8.1.3. Add staged CI jobs for formal verification. Requires 8.1.2.
  - [ ] Add `kani-smoke` to pull-request validation after the first smoke
        harnesses land.
  - [ ] Add `verus-proofs` as manual or nightly validation first, then promote
        only if the proof set remains stable.
  - [ ] Acceptance criteria: the existing `build-test` job remains intact,
        formal jobs install their own tools, and slow proof suites are isolated
        from the default pull-request path.

### 8.2. Clarify proof contracts before gating

*Outcome: Define the exact assurances that Kani and Verus are expected to*
*prove, including filesystem assumptions and trust boundaries.*

- [ ] 8.2.1. Publish the transaction atomicity contract for the Double-Lock
      path. See `docs/formal-verification-methods-in-weaver.md`
      "Atomicity contract".
  - [ ] State the filesystem assumptions that define "all changes applied or
        original state restored".
  - [ ] State catastrophic failure conditions that are outside the verified
        model.
  - [ ] Acceptance criteria: the design document and user's guide describe the
        same atomicity promise using one shared contract.
- [ ] 8.2.2. Define the semantic-lock contract precisely. Requires 8.2.1. See
      `docs/formal-verification-methods-in-weaver.md`
      "Semantic-lock contract".
  - [ ] Specify severity handling, provider normalization, baseline scope, and
        backend-unavailable semantics.
  - [ ] Acceptance criteria: implementation docs, CLI behaviour, and future
        proof harnesses can refer to one explicit semantic-lock definition
        without relying on inferred behaviour.
- [ ] 8.2.3. Document the formal-verification trust boundary. Requires 8.2.2.
      See `docs/formal-verification-methods-in-weaver.md` "Trust boundary".
  - [ ] Separate verified orchestration invariants from trusted external-tool
        assumptions.
  - [ ] Acceptance criteria: docs name the verified kernel, list unverified
        dependencies explicitly, and avoid claiming semantic correctness for
        third-party tools.

### 8.3. Add Kani checks for the transaction and patch kernels

*Outcome: Add bounded model-checking coverage for the highest-risk write path*
*that Weaver owns directly.*

- [ ] 8.3.1. Add Kani smoke harnesses for Double-Lock transaction ordering in
      `crates/weaverd/src/safety_harness/`. Requires 8.1.2 and 8.2.1.
  - [ ] Prove commit is reachable only when both locks pass.
  - [ ] Prove lock-failure and backend-unavailable states are non-committing.
  - [ ] Acceptance criteria: `make kani` executes transaction smoke harnesses,
        and counterexamples are reproducible through the documented target.
- [ ] 8.3.2. Add Kani smoke harnesses for rollback bookkeeping and bounded file
      traces in `crates/weaverd/src/safety_harness/`. Requires 8.3.1.
  - [ ] Cover bounded create, modify, and delete combinations.
  - [ ] Cover commit-phase failure that restores the pre-state under the
        documented assumptions.
  - [ ] Acceptance criteria: harnesses assert file-set preservation and
        rollback restoration over bounded traces.
- [ ] 8.3.3. Add Kani smoke harnesses for `act apply-patch` matching and path
      guardrails in `crates/weaverd/src/dispatch/act/apply_patch/`.
      Requires 8.3.1 and 6.1.4.
  - [ ] Cover cursor monotonicity for ordered `SEARCH`/`REPLACE` blocks.
  - [ ] Cover whole-command abort on unmatched blocks.
  - [ ] Cover path normalization rejecting absolute and parent-escape paths.
  - [ ] Acceptance criteria: `make kani` includes apply-patch smoke harnesses,
        and the checked properties map directly to the documented patch
        contract.
- [ ] 8.3.4. Promote larger transaction and patch harnesses to `make kani-full`
      once the smoke harnesses are stable. Requires 8.3.2 and 8.3.3.
  - [ ] Expand touched-file counts and mixed-operation sequences.
  - [ ] Keep smoke and full harnesses separate.
  - [ ] Acceptance criteria: `make kani-full` exercises larger bounded traces
        than the pull-request smoke set, and scheduled runs record stable pass
        or fail outcomes.

### 8.4. Add Kani checks for capability routing and refusal semantics

*Outcome: Verify bounded capability-selection invariants in the plugin control*
*plane before expanding proof coverage elsewhere.*

- [ ] 8.4.1. Add Kani smoke harnesses for capability-resolution soundness in
      `crates/weaver-plugins/src/`. Requires 8.1.2, 5.3.2, and 8.2.3.
  - [ ] Prove the selected provider satisfies the requested language and
        capability.
  - [ ] Prove refusal is deterministic when no compatible provider exists.
  - [ ] Acceptance criteria: `make kani` runs capability-routing smoke
        harnesses, and refusal semantics are asserted over bounded routing
        tables.
- [ ] 8.4.2. Add property-based tests for refusal-code stability, path-policy
      helpers, and bounded routing tables. Requires 8.4.1.
  - [ ] Acceptance criteria: generated tests complement the Kani harnesses by
        exploring larger input spaces without widening the verified kernel
        claims.

### 8.5. Add a proof-only Verus kernel

*Outcome: Prove the smallest stable invariants in proof-only modules outside*
*the main Cargo build.*

- [ ] 8.5.1. Add a proof-only Verus workspace under `verus/` with
      `weaver_proofs.rs` as the entrypoint. Requires 8.1.2 and 8.2.3.
  - [ ] Add `transaction_kernel.rs`, `capability_routing.rs`, and
        `apply_patch_paths.rs`.
  - [ ] Acceptance criteria: `make verus` executes the proof entrypoint, and
        the proof modules use proof-specific types rather than widening the
        production API.
- [ ] 8.5.2. Prove transaction-gating and rollback-restoration lemmas over a
      modelled workspace state. Requires 8.5.1 and 8.2.1.
  - [ ] Acceptance criteria: proofs establish that commit requires both locks
        and that documented rollback restoration holds under the chosen model
        assumptions.
- [ ] 8.5.3. Prove capability-resolution soundness over an abstract resolver.
      Requires 8.5.1 and 8.2.3.
  - [ ] Acceptance criteria: proofs establish that successful resolution
        satisfies language, capability, and policy predicates, and that refusal
        occurs instead of silent fallback when no provider qualifies.

### 8.6. Extend formal verification coverage after later roadmap features land

*Outcome: Expand proof coverage only when the underlying contracts and kernels*
*are implemented and stable.*

- [ ] 8.6.1. Add Kani harnesses for graph-slice budget enforcement after 7.2.5
      lands. Requires 7.2.5 and 8.3.4.
  - [ ] Acceptance criteria: bounded graph harnesses prove counters do not
        exceed accepted-card, edge, and token-budget caps on small graphs.
- [ ] 8.6.2. Add Kani harnesses for duplicate-name guardrails and assignment
      injectivity after 7.4.8 and 7.4.9 land. Requires 7.4.8, 7.4.9, and
      8.3.4.
  - [ ] Acceptance criteria: bounded matching harnesses prove injective
        assignments by default and prove many-to-one assignments remain gated
        behind explicit split or merge modes.
- [ ] 8.6.3. Add Kani harnesses for Sempai semantic constraints only after the
      planned parser and backend crates exist. Requires 4.2 and 4.3.
  - [ ] Acceptance criteria: formal checks cover deterministic matcher and
        normalization kernels without trying to verify external parser or
        runtime dependencies wholesale.
