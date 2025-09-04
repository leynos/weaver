# Roadmap

## Phase 0: Foundation & Tooling (Complete)

-   [x] Set up the project workspace, CI/CD pipeline, and core dependencies.

## Phase 1: Core MVP & Safety Harness Foundation

*Goal: Establish the core client/daemon architecture, basic LSP integration, and the foundational security and verification mechanisms. The MVP must be safe for write operations from day one.*

-   [ ] Implement the `weaver-cli` and `weaverd` crates with robust daemonisation and process management.

-   [ ] Build the `weaver-lsp-host` crate with support for initialisation, capability detection, and core LSP features (definition, references, diagnostics) for Rust, Python, and TypeScript.

-   [ ] Implement the initial version of the `weaver-sandbox` crate, using `birdcage` for its focused scope and production usage, prioritising robust Linux support via namespaces and seccomp-bpf.

-   [ ] Implement the full "Double-Lock" safety harness logic in `weaverd`. This is a critical, non-negotiable feature for the MVP. All `act` commands must pass through this verification layer before committing to the filesystem.

-   [ ] Implement atomic edits to ensure that multi-file changes either succeed or fail as a single transaction.

## Phase 2: Syntactic & Relational Intelligence

*Goal: Add the Tree-sitter and call graph layers to provide deeper structural and relational understanding of code.*

-   [ ] Create the `weaver-syntax` crate and implement the structural search engine for `observe grep` and `act apply-rewrite`, drawing inspiration from ast-grep's pattern language.

-   [ ] Integrate the "Syntactic Lock" from `weaver-syntax` into the "Double-Lock" harness.

-   [ ] Create the `weaver-graph` crate and implement the LSP Provider for call graph generation, using the `textDocument/callHierarchy` request as the initial data source.

## Phase 3: Plugin Ecosystem & Specialist Tools

*Goal: Build the plugin architecture to enable orchestration of best-in-class, language-specific tools.*

-   [ ] Design and implement the `weaver-plugins` crate, including the secure IPC protocol between the `weaverd` broker and sandboxed plugin processes.

-   [ ] Develop the first set of actuator plugins:

    -   [ ] A plugin for `rope` to provide advanced Python refactoring.

    -   [ ] A plugin for `srgn` to provide high-performance, precision syntactic editing.

-   [ ] Develop the first specialist sensor plugin:

    -   [ ] A plugin for `jedi` to provide supplementary static analysis for Python.

-   [ ] Refine the graceful degradation logic to suggest specific plugin-based solutions when core LSP features are missing.

-   [ ] Implement the Static Analysis Provider for `weaver-graph` (e.g., wrapping PyCG) as the first major graph plugin.

## Phase 4: Advanced Agent Support & RAG

*Goal: Introduce features specifically designed to support advanced agent planning and human-in-the-loop workflows.*

-   [ ] Implement the `onboard-project` command based on the "Meta-RAG" design, orchestrating other Weaver components to generate the `PROJECT.dna` summary file.

-   [ ] Implement a hybrid interactive mode (`--interactive`) that, in case of a "Double-Lock" verification failure, presents the proposed diff and the resulting errors to a human user for manual review, approval, or rejection.

-   [ ] Begin research and development for the Dynamic Analysis Ingestion provider for `weaver-graph`, allowing it to consume and merge profiling data from tools like `gprof` and `callgrind`.

