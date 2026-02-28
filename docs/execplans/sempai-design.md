# Normalize documentation style and navigation for parser and Semgrep docs

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

The `docs/` set currently has several parser and query-language documents with
inconsistent structure, style, and cross-linking. After this change, the
requested documents will follow `docs/documentation-style-guide.md`, internal
links will resolve consistently within the Weaver repository, and there will be
an explicit documentation index (`docs/contents.md`) plus a repository layout
reference (`docs/repository-layout.md`) that distinguishes implemented and
planned components.

Success is observable by reading the updated documents and by running
`make markdownlint`, `make fmt`, and `make nixie` without failures.

## Constraints

- Do not modify source-code crates for this task; scope is documentation only.
- Keep terminology and spelling in en-GB-oxendict except where external APIs or
  tool names require upstream spelling.
- Keep link targets repository-local when possible, and avoid introducing
  absolute host URLs for local docs.
- Preserve technical intent of each document while normalizing style and
  structure.
- Create the contents and repository-layout documents in `docs/` using the same
  approach as `../zamburak/docs/contents.md` and
  `../zamburak/docs/repository-layout.md`.

## Tolerances (exception triggers)

- Scope tolerance: if this requires changes outside `docs/` or more than 15
  documentation files, stop and escalate.
- Interface tolerance: if requested consistency requires renaming existing
  `docs/` files referenced by external tooling, stop and escalate.
- Dependency tolerance: if a new build or lint dependency is required, stop and
  escalate.
- Iteration tolerance: if markdown gates fail after three fix iterations, stop
  and report failure mode.
- Ambiguity tolerance: if style-guide rules conflict with existing project
  standards and no clear precedence exists, stop and escalate with options.

## Risks

- Risk: Existing cross-links may use historical filenames that differ from
  canonical names, creating broken references. Severity: medium. Likelihood:
  high. Mitigation: run explicit link-target checks across changed docs and
  repair references to existing files.

- Risk: The repository-layout document may drift from actual tree layout.
  Severity: medium. Likelihood: medium. Mitigation: derive implemented sections
  from current `crates/` and root tree, then mark planned components explicitly
  as planned.

- Risk: Formatting and lint rules may fail after large prose edits.
  Severity: low. Likelihood: medium. Mitigation: run `make markdownlint` and
  `make fmt` before commit and apply targeted corrections.

## Implementation approach

1. Inventory current docs and link references for the requested files and
   `docs/semgrep-language-reference/`.
2. Normalize style for headings, list formatting, code-block info strings,
   footnote ordering, and paragraph wrapping to align with
   `docs/documentation-style-guide.md`.
3. Repair broken or inconsistent repository-local links.
4. Add `docs/contents.md` modelled on `../zamburak/docs/contents.md` for Weaver.
5. Add `docs/repository-layout.md` modelled on
   `../zamburak/docs/repository-layout.md`, with separate sections for
   implemented and planned components.
6. Run markdown quality gates and capture logs.
7. Commit with a descriptive message.

## Validation plan

Run the following commands with `set -o pipefail` and `tee` logs:

```bash
set -o pipefail
make markdownlint | tee /tmp/markdownlint-weaver-sempai-design.out
make fmt | tee /tmp/fmt-weaver-sempai-design.out
make nixie | tee /tmp/nixie-weaver-sempai-design.out
```

Then inspect each log tail for failures.

## Progress

- [x] (2026-02-28 00:00 UTC) Confirmed branch context (`sempai-design`) and
      loaded guidance (`AGENTS.md`, style guide, and template docs).
- [x] (2026-02-28 00:00 UTC) Created this ExecPlan before applying
      documentation updates.
- [x] (2026-02-28 00:00 UTC) Inventoried requested docs and repaired
      repository-local link consistency, including removal of non-portable
      absolute local paths.
- [x] (2026-02-28 00:00 UTC) Normalized requested docs to
      `docs/documentation-style-guide.md`.
- [x] (2026-02-28 00:00 UTC) Created `docs/contents.md`.
- [x] (2026-02-28 00:00 UTC) Created `docs/repository-layout.md` with
      implemented and planned component separation.
- [x] (2026-02-28 00:00 UTC) Ran markdown gates and captured evidence logs:
      `/tmp/markdownlint-weaver-sempai-design.out`,
      `/tmp/fmt-weaver-sempai-design.out`,
      `/tmp/nixie-weaver-sempai-design.out`.
- [x] (2026-02-28 00:00 UTC) Committed documentation updates with gate
      evidence.

## Surprises & Discoveries

- Initial `make markdownlint` failed before formatting due violations in
  `docs/adr-001-plugin-capability-model-and-act-extricate.md`. Running
  `make fmt` applied repository markdownlint autofixes, after which
  `make markdownlint` passed.

## Decision Log

- Decision: Use `../zamburak/docs/contents.md` and
  `../zamburak/docs/repository-layout.md` as structural templates, but adapt
  terminology, paths, and scope to Weaver-specific crates and docs. Rationale:
  The request explicitly asks for the same approach, not direct copy.

- Decision: Keep this task as a single atomic documentation change set unless
  gate failures force isolated remediation commits. Rationale: The changes are
  tightly coupled around style normalization and navigation consistency.

## Outcomes & Retrospective

Documentation coverage for parser and Semgrep references is now standardized
with the style guide, and repository navigation now has explicit index and
layout documents. Markdown gates (`make markdownlint`, `make fmt`, and
`make nixie`) passed for the final staged state.
