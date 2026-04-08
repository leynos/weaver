# Weaver

Weaver gives you semantic code operations through a CLI and daemon. Use it when
plain text search is too lossy, when the user wants structured JSON about code
relationships, or when a rename should go through language-aware tooling
instead of raw text edits.

## Compatibility

- Requires `weaver` in `PATH`.
- Best support today is for Rust (`.rs`), Python (`.py`), and TypeScript
  (`.ts`/`.tsx`).
- Domain commands auto-start the daemon when needed, so manual daemon control
  is mainly for health checks and debugging.
- In this repository, the implemented semantic read operations are currently
  `observe get-definition` and `observe get-card`. Do not route reference
  lookup, call-graph, or diagnostics requests to Weaver here; use fallback
  tooling instead.

## Operating stance

- Prefer `weaver --output json` unless the user explicitly wants human-readable
  terminal output.
- Prefer `observe` commands before `act` commands.
- After any `act` command, inspect the routing output, review the diff, and run
  the project's validators.
- If Weaver refuses a request because a language or capability is unavailable,
  explain the refusal briefly and fall back to the repository's normal tools.

## Before you run a command

1. Convert source paths to file URIs for `observe` commands, for example
   `file:///abs/path/to/file.rs`.
2. Use 1-indexed `LINE:COL` values for `--position`.
3. Choose the narrowest implemented command that matches the task:
   - definition lookup → `observe get-definition`
   - rich symbol summary → `observe get-card`
   - semantic rename → `act refactor --refactoring rename`
4. If the user asks for references, call graphs, or diagnostics, do not call
   the unimplemented Weaver operations in this repo. Use the repository's
   normal exploration or validation tools instead.
5. If the user asks what Weaver supports in the current setup, run
   `weaver --capabilities`.

## Command playbook

### Inspect the daemon

```sh
weaver daemon status
weaver daemon start
```

You usually do not need `daemon start` because domain commands auto-start it,
but `daemon status` is a useful probe when diagnosing failures.

### Get a definition

```sh
weaver --output json observe get-definition \
  --uri file:///abs/path/to/file.rs \
  --position 42:17
```

Expect an array of locations, each with `uri`, `line`, and `column`.

### Fetch a symbol card

```sh
weaver --output json observe get-card \
  --uri file:///abs/path/to/file.rs \
  --position 42:17 \
  --detail structure
```

Detail levels:

- `minimal` — identity only
- `signature` — callable surface
- `structure` — default, good general view
- `semantic` — adds hover and type data when LSP is available
- `full` — richest available payload, though it may still degrade gracefully

Start with `structure`. Escalate to `semantic` or `full` only when the extra
detail will change the answer.

Note that the request position uses 1-indexed `LINE:COL`, but successful card
responses report `range.start` and `range.end` as 0-based half-open offsets.

### Rename a symbol

```sh
weaver --output json act refactor \
  --refactoring rename \
  --file src/main.rs \
  new_name=better_name \
  offset=123
```

Notes:

- `--file` is workspace-relative.
- `offset` is a 0-based UTF-8 byte offset, not `LINE:COL`.
- The daemon emits a `CapabilityResolution` record before the final status;
  inspect it when routing matters.
- Treat the rename as complete only after diff review and validator passes.

## Choosing commands and fallbacks

- Use `get-definition` for "where is this defined?"
- Use `get-card` for a compact semantic brief on a symbol.
- Use `act refactor rename` only for deliberate symbol changes with a clear
  target location.
- Use fallback tooling, not Weaver, for:
  - "show me all usages"
  - "who calls this?" / "what does this call?"
  - file or project diagnostics

If Weaver reports an unsupported language, missing capability, or another
structured refusal:

1. Quote the refusal reason briefly.
2. Switch to the repository's normal exploration or editing tools.
3. State why the fallback is necessary.

## Output handling

When you use `--output json`, extract the actionable parts for the user instead
of pasting raw payloads unless they asked for the full JSON. Focus on:

- matched locations
- symbol names, kinds, and containers
- routing decisions for refactors
- refusal reasons and next steps
- clear fallback guidance when the requested operation is not implemented in
  this repository

## Examples

### Definition lookup

User: "Use Weaver to find the definition of the symbol at `src/lib.rs:42:17`."

Agent flow:

1. Build a file URI for `src/lib.rs`.
2. Run `weaver --output json observe get-definition ...`.
3. Return the resolved location or locations succinctly.

### Richer semantic context

User: "Give me a semantic summary of the function at `src/handler.py:18:5`."

Agent flow:

1. Start with `observe get-card --detail structure`.
2. Escalate to `--detail semantic` only if hover or type data matters.
3. Summarize the signature, docs, locals, branches, and provenance.

### Safe rename

User: "Rename the symbol at byte offset 123 in `src/main.py` to `build_index`
with Weaver."

Agent flow:

1. Run `act refactor --refactoring rename --file src/main.py
   new_name=build_index offset=123`.
2. Inspect the capability resolution.
3. Review the diff.
4. Run project validators.
5. Report the outcome.
