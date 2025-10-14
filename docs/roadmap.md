# Roadmap

## Phase 0: Foundation & Tooling (Complete)

- [x] Set up the project workspace, CI/CD pipeline, and core dependencies.

## Phase 1: Core MVP & Safety Harness Foundation

*Goal: Establish the core client/daemon architecture, basic LSP integration,
and the foundational security and verification mechanisms. The MVP must be safe
for write operations from day one.*

### Step: Deliver CLI and daemon foundation

*Outcome: Ship a pair of crates (`weaver-cli`, `weaverd`) that honour the
design contract in `docs/weaver-design.md` and expose the lifecycle expected by
`docs/documentation-style-guide.md`.*

- [x] Define the shared configuration schema for `weaver-cli` and `weaverd`
      in `weaver-config`, using `ortho-config` to merge config files,
      environment overrides, and CLI flags for daemon sockets, logging, and the
      capability matrix defaults.
      - Acceptance criteria: Schema documented in crate docs, integration tests
        demonstrate precedence order (file < env < CLI), and default sockets
        align with the design doc.
- [ ] Implement the `weaver-cli` executable as the thin JSONL client that
      initialises configuration via `ortho-config`, exposes the
      `--capabilities` probe, and streams requests to a running daemon over
      standard IO.
      - Acceptance criteria: CLI command surface mirrors the design table,
        capability probe outputs the negotiated matrix, and JSONL framing is
        validated with golden tests.
- [ ] Implement the `weaverd` daemon bootstrap that consumes the shared
      configuration, starts the Semantic Fusion backends lazily, and supervises
      them with structured logging and error reporting.
      - Acceptance criteria: Bootstrap performs health reporting hooks,
        backends start only on demand, and failures propagate as structured
        events.
- [ ] Implement robust daemonisation and process management for `weaverd`,
      including backgrounding with `daemonize-me`, PID/lock file handling,
      health checks, and graceful shutdown on signals.
      - Acceptance criteria: Background start creates PID and lock files,
        duplicate starts fail fast, and signal handling shuts down within the
        timeout budget.
- [ ] Provide lifecycle commands in `weaver-cli` (for example, `daemon start`,
      `daemon stop`, `daemon status`) that manage the daemon process, verify
      socket availability, and surface actionable errors when start-up fails.
      - Acceptance criteria: Lifecycle commands call into shared helper logic,
        refuse to start when sockets are bound, and emit recovery guidance for
        the operator.

- [ ] Build the `weaver-lsp-host` crate with support for initialisation,
    capability detection, and core LSP features (definition, references,
    diagnostics) for Rust, Python, and TypeScript.

- [ ] Implement the initial version of the `weaver-sandbox` crate, using
    `birdcage` for its focused scope and production usage, prioritising robust
    Linux support via namespaces and seccomp-bpf.

- [ ] Implement the full "Double-Lock" safety harness logic in `weaverd`.
    This is a critical, non-negotiable feature for the MVP. All `act` commands
    must pass through this verification layer before committing to the
    filesystem.

- [ ] Implement atomic edits to ensure that multi-file changes either succeed
    or fail as a single transaction.

## Phase 2: Syntactic & Relational Intelligence

*Goal: Add the Tree-sitter and call graph layers to provide deeper structural
and relational understanding of code.*

- [ ] Create the `weaver-syntax` crate and implement the structural search
    engine for `observe grep` and `act apply-rewrite`, drawing inspiration from
    ast-grep's pattern language.

- [ ] Integrate the "Syntactic Lock" from `weaver-syntax` into the
    "Double-Lock" harness.

- [ ] Create the `weaver-graph` crate and implement the LSP Provider for call
    graph generation, using the `textDocument/callHierarchy` request as the
    initial data source.

## Phase 3: Plugin Ecosystem & Specialist Tools

*Goal: Build the plugin architecture to enable orchestration of best-in-class,
language-specific tools.*

- [ ] Design and implement the `weaver-plugins` crate, including the secure
    IPC protocol between the `weaverd` broker and sandboxed plugin processes.

- [ ] Develop the first set of actuator plugins:

  - [ ] A plugin for `rope` to provide advanced Python refactoring.

  - [ ] A plugin for `srgn` to provide high-performance, precision
        syntactic editing.

- [ ] Develop the first specialist sensor plugin:

  - [ ] A plugin for `jedi` to provide supplementary static analysis for
        Python.

- [ ] Refine the graceful degradation logic to suggest specific plugin-based
    solutions when core LSP features are missing.

- [ ] Implement the Static Analysis Provider for `weaver-graph` (e.g.,
    wrapping PyCG) as the first major graph plugin.

## Phase 4: Advanced Agent Support & RAG

*Goal: Introduce features specifically designed to support advanced agent
planning and human-in-the-loop workflows.*

- [ ] Implement the `onboard-project` command based on the "Meta-RAG" design,
    orchestrating other Weaver components to generate the `PROJECT.dna` summary
    file.

- [ ] Implement a hybrid interactive mode (`--interactive`) that, in case of
    a "Double-Lock" verification failure, presents the proposed diff and the
    resulting errors to a human user for manual review, approval, or rejection.

- [ ] Begin research and development for the Dynamic Analysis Ingestion
    provider for `weaver-graph`, allowing it to consume and merge profiling
    data from tools like `gprof` and `callgrind`.
