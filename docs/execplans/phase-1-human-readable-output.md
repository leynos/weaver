# Phase 1: Human-Readable Output Rendering

## Goal

Implement human-readable rendering for command outputs that contain code
locations or diagnostics. The renderer must produce `miette`-style context
blocks while keeping the JSONL protocol payloads intact for machine use.

## Acceptance Criteria

- Definition, reference, diagnostics, and safety harness failure outputs render
  file headers, line-numbered context, and caret spans in human-readable mode.
- JSONL output from `weaverd` remains unchanged; JSON output mode continues to
  emit the raw payloads without additional rendering.
- Missing source content falls back to path-and-range output with a clear
  explanation of why context could not be rendered.

## Design Decisions

1. **CLI-side rendering**: Keep `weaverd` JSONL payloads unchanged and perform
   human-readable rendering in `weaver-cli`, preserving the canonical transport
   while allowing richer presentation.
2. **Output format selection**: Add an `--output` flag with `auto`, `human`, and
   `json` values. `auto` defaults to `human` when stdout is a TTY and `json`
   when stdout is redirected, ensuring machine pipelines remain stable.
3. **Miette-style renderer configuration**: Emit ASCII-only context blocks
   modelled on `miette` output to match the design doc examples.
4. **Lightweight response models**: Define CLI-side response structs mirroring
   `docs/users-guide.md` JSON shapes for definitions, references, diagnostics,
   and safety harness failures to avoid cross-crate coupling.
5. **Source grouping and fallback**: Group results by file with one header per
   file and a context block per location. When source content is unavailable,
   emit a fallback message that preserves the path and range with the reason.

## Module Structure

```text
crates/weaver-cli/src/
  output/
    mod.rs          # OutputFormat and render entry points (~80 lines)
    models.rs       # JSON response structs (~140 lines)
    render.rs       # miette-based rendering helpers (~220 lines)
    source.rs       # URI -> path, span mapping (~180 lines)
```

## Implementation Steps

### Step 1: Update dependencies

- Upgrade `rstest-bdd` and `rstest-bdd-macros` to v0.4.0 across the workspace,
  updating uses if required by the new API.

### Step 2: Introduce output format selection

- Add an `OutputFormat` enum (`Auto`, `Human`, `Json`) to `weaver-cli`.
- Extend CLI parsing in `crates/weaver-cli/src/lib.rs` to accept `--output`.
- Use TTY detection to resolve `Auto` to `Human` or `Json`.

### Step 3: Define response models for rendering

- Add `output/models.rs` with structs for:
  - Definitions (`Vec<DefinitionLocation>`)
  - References (`ReferencesResponse` with locations)
  - Diagnostics (`DiagnosticsResponse` with line/column/message)
  - Safety harness failures (path + optional line/column + message)
- Implement `serde` parsing helpers that fail fast with clear errors.

### Step 4: Build the context renderer

- Convert URI + line/column (1-indexed) to byte offsets, handling multi-line
  spans and range end defaults for point locations.
- Render ASCII context blocks with caret spans and group multiple spans per
  file under a shared header.
- Provide fallback rendering for unreadable or missing files, including the
  reason (e.g., "file missing" or "invalid URI").

### Step 5: Wire rendering into CLI output flow

- Update `read_daemon_messages` to accept the resolved `OutputFormat` and the
  invoked domain/operation.
- For `human` output, parse JSON payloads for definition, references,
  diagnostics, and safety harness failure responses; render using the new
  module.
- For `json` output (or parse failures), stream the raw payload unchanged.

### Step 6: Add unit tests

- Unit tests for span calculation, multi-line ranges, and file grouping.
- Unit tests for fallback messaging when source content is missing or the URI
  is invalid.
- Unit tests validating JSON parsing for each response model.

### Step 7: Add behavioural tests (rstest-bdd v0.4.0)

- Add a new `.feature` file under `crates/weaver-cli/tests/features/` covering:
  - Happy path: definition output renders context blocks.
  - Happy path: diagnostics output renders context blocks.
  - Unhappy path: missing file falls back to path-and-range explanation.
  - Format selection: `--output json` passes through raw JSON.
- Implement or extend step definitions in `crates/weaver-cli/src/tests/` to
  drive a fake daemon with JSONL responses and assert rendered output.

### Step 8: Update end-to-end (e2e) tests

- Extend `crates/weaver-e2e` to validate human-readable rendering for
  definitions and diagnostics with real language server responses.
- Add unhappy-path coverage that exercises missing source content fallbacks.

### Step 9: Update documentation

- Record the output-format decision and renderer behaviour in
  `docs/weaver-design.md` (section 2.1.4).
- Update `docs/users-guide.md` output format section to describe `--output`,
  the TTY-driven default, and the new context block format.

### Step 10: Mark roadmap entry done

- Update `docs/roadmap.md` to mark the human-readable output item complete.

### Step 11: Run quality gates

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt.log
make lint 2>&1 | tee /tmp/lint.log
make test 2>&1 | tee /tmp/test.log
```

Run `make markdownlint`, `make fmt`, and `make nixie` after documentation
updates, per `AGENTS.md`.

## Files Modified

| File                                                         | Action                                      |
| ------------------------------------------------------------ | ------------------------------------------- |
| `Cargo.toml`                                                 | Bump `rstest-bdd` deps                      |
| `crates/weaver-cli/Cargo.toml`                               | Add output rendering deps                   |
| `crates/weaver-cli/src/lib.rs`                               | Add `--output` and renderer integration     |
| `crates/weaver-cli/src/output/mod.rs`                        | Create                                      |
| `crates/weaver-cli/src/output/models.rs`                     | Create                                      |
| `crates/weaver-cli/src/output/render.rs`                     | Create                                      |
| `crates/weaver-cli/src/output/source.rs`                     | Create                                      |
| `crates/weaver-cli/tests/features/weaver_cli_output.feature` | Create                                      |
| `crates/weaver-cli/src/tests/behaviour.rs`                   | Extend steps for new scenarios              |
| `crates/weaver-cli/src/tests/support/`                       | Add fixtures for rendered output            |
| `crates/weaver-e2e/tests/`                                   | Extend for human-readable output validation |
| `docs/weaver-design.md`                                      | Record design decisions                     |
| `docs/users-guide.md`                                        | Document output format changes              |
| `docs/roadmap.md`                                            | Mark entry done                             |
| `docs/execplans/phase-1-human-readable-output.md`            | Create                                      |

## Dependencies

- `rstest-bdd` v0.4.0 + `rstest-bdd-macros` v0.4.0
