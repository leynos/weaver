# Architectural decision record (ADR) 014: Sandbox-compatible CI runners

## Status

Accepted. The full coverage diagnostic passed on 2026-09-24; use `ubuntu-22.04`
for the Rust test and coverage jobs.

## Date

2026-09-24.

## Context and problem statement

The blocking Rust CI and coverage workflows run tests that create Birdcage user
namespaces and write initial and nested UID/GID mappings. The selected runner
must permit these operations while preserving the full test and coverage gates.

The unchanged Birdcage trial failed on two candidate runner routes. Diagnostic
PR #299 on Ubicloud reached a permission-denied error writing
`/proc/self/uid_map`. Diagnostic PR #300 on `ubuntu-latest` failed the
unchanged Birdcage trial with EPERM. Diagnostic PR #301 used `ubuntu-22.04`,
image version `20260920.303.1`; its unchanged direct test, namespace creation,
and initial and nested UID/GID mapping writes succeeded.

Diagnostic PR #302 completed the pinned coverage action on `ubuntu-22.04`,
against runtime commit `e39abdc710f95ef2ed7554ddff1dfef242fbb137`. Run
[35933641404](https://github.com/leynos/weaver/actions/runs/35933641404) used
image `20260920.303.1` and kernel `6.8.0-1064-azure`. The runner label selects
a GitHub-hosted image family, not an immutable image revision. The diagnostic
context and coverage artefacts are `10782198111` and `10782232818`.

## Decision drivers

- Keep the complete blocking test suite and coverage ratchet enabled.
- Use a runner whose observed host policy permits Birdcage namespace mapping.
- Cover fork and same-repository pull requests with the same blocking gate.
- Keep baseline publication and the CodeScene token within their existing
  trust boundaries.
- Preserve supported release, macOS, and FreeBSD lanes.
- Make later host-image changes observable and reversible.

## Options considered

### Option A: Use `ubuntu-22.04` for both Rust test and coverage jobs

Route `ci.yml`'s `build-test` and `coverage-main.yml`'s `coverage-upload` to
the tested GitHub-hosted label. Keep their existing steps, permissions,
timeouts, triggers, and coverage settings.

### Option B: Keep Ubicloud CI with a fork fallback

Retain the previous runner split. The Ubicloud diagnostic failed at the
required UID-map write, so this leaves the ordinary blocking CI path unable to
run the sandbox test.

### Option C: Use `ubuntu-latest` for both jobs

Use the moving Ubuntu alias consistently. The diagnostic on the current
`ubuntu-latest` image failed the unchanged Birdcage trial, so this does not
meet the required host capability.

## Decision statement

For the blocking Rust test and coverage jobs, where Birdcage requires user
namespace UID/GID mapping, diagnostics observed a denied `/proc/self/uid_map`
write on Ubicloud and EPERM from the Birdcage trial on `ubuntu-latest`, Weaver
selects literal `ubuntu-22.04` runner labels for both `build-test` and
`coverage-upload`, and retains the complete test suite, coverage ratchet, token
boundary, timeouts, and other platform placements, to provide the host
capability demonstrated by the direct and full-suite trials while preserving
existing checks and trust boundaries, accepting that the GitHub-hosted label
does not pin an immutable image and requires revalidation as its image or
kernel changes.

## Decision outcome

Adopt option A. Keep `build-test` on pull requests and manual dispatch,
including pull requests from forks. Keep its full blocking gate and ratcheted
coverage action. Keep `coverage-upload` on pushes to the default branch and
manual dispatch, with its existing 20-minute timeout and `contents: read`
permission.

The coverage action uses `publish-baseline: auto`. Diagnostic PR #302 checked
the existing ratchet baseline without publishing one: `Save baselines` was
explicitly skipped. The pinned action saves a baseline only on a push to
`main`. The diagnostic contained no CodeScene step or token. In
`coverage-main.yml`, the separate CodeScene upload remains guarded to the
`main` ref and receives its token only in that upload step. The runner change
does not alter those conditions or permissions.

The full coverage run passed 1,562 tests and skipped 4. The sandbox child probe
passed. Coverage was 78.5% against the 78.16% baseline, within the configured
+/-1.00 percentage-point tolerance. Diagnostic PR #302 was closed unmerged
after collecting this evidence.

Release Linux jobs retain their Ubicloud placement and fork fallback. The macOS
release job remains on `macos-15`; the disabled FreeBSD release leg remains on
`ubuntu-latest`.

## Consequences

- Fork and same-repository pull requests exercise the same Birdcage-capable
  blocking CI job.
- Coverage tests run on the same runner label in CI and in the main-branch
  coverage publisher.
- The CI job keeps the complete test suite, lint and documentation gates, and
  ratcheted coverage check.
- Historical queue and work measurements from 2026-09-16 no longer describe
  the current test and coverage runner routes.

## Known risks and limitations

- The direct trial and full coverage run establish compatibility only for the
  observed image and kernel; the hosted label can move to a different image.
- GitHub may update the image or kernel behind the `ubuntu-22.04` label, which
  can change user-namespace policy or runtime characteristics.

## Reversal conditions

Reconsider this placement if a future full coverage run regresses because of
runner incompatibility, or if a later image change prevents the unchanged
Birdcage mapping trial. Select a replacement only after a reviewed diagnostic
shows that it supports the required mapping operations and the full test and
coverage gates. Do not address incompatibility by skipping tests, weakening
host protections, or disabling the ratchet.

## Maintenance

- Recheck the direct sandbox trial when the hosted image or kernel changes in
  a way that affects user-namespace mapping.
