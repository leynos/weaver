"""The reviewed runner-placement decisions, and why each one is what it is.

Everything here was decided by a person and is enforced by
``ci_runner_placement_test``; everything in ``runner_placement_reader`` is
derived from the workflow tree. That is the line these modules are split on.

The tables are keyed by ``(workflow file, job id)`` coordinate rather than by
job name, because two workflows may both define a job called ``release`` and
a contract keyed on the bare name would check one of them twice and the other
never. weaver has exactly that: ``release.yml``'s ``release`` and
``release-dry-run.yml``'s ``release``.
"""

from __future__ import annotations

import typing as typ

UBICLOUD_LABEL_PREFIX: typ.Final = "ubicloud-"

#: The larger Ubicloud shape, for lanes that compile the workspace.
#: Ubicloud's four vCPU are 1.5 to 2 times slower than a GitHub public
#: runner's four for Rust compilation, measured on wireframe, so two would
#: roughly triple a lane that already takes a quarter of an hour.
UBICLOUD_LARGE: typ.Final = "ubicloud-standard-4"

#: The smaller Ubicloud shape, for lanes nobody waits on.
UBICLOUD_SMALL: typ.Final = "ubicloud-standard-2"

GITHUB_HOSTED_LABEL: typ.Final = "ubuntu-latest"
MACOS_LABEL: typ.Final = "macos-15"

#: The one guard a fork fallback may key on. Compared by equality, because a
#: sibling field such as ``head.repo.private`` yields an expression of exactly
#: the same shape that selects the wrong runner on every fork pull request.
FORK_GUARD: typ.Final = "github.event.pull_request.head.repo.fork"

#: Lanes that meet forks and therefore carry the fallback, with the label the
#: non-fork arm must select. Every one of these serves pull requests: the two
#: release jobs do so because ``release-dry-run.yml`` calls ``release.yml`` on
#: every pull request, which is easy to miss from the workflow's name.
EXPECTED_FORK_FALLBACK: typ.Final = {
    ("ci.yml", "build-test"): UBICLOUD_LARGE,
    ("release.yml", "metadata"): UBICLOUD_SMALL,
}

#: Lanes that cannot meet a fork and so declare a bare label. A constant
#: guard would read as a decision nobody made.
#:
#: ``coverage-upload`` serves push and dispatch, neither of which carries a
#: pull request. ``release.yml``'s ``release`` is gated on ``should_publish``,
#: true only on a tag push, so it is skipped on every dry run.
#: ``build-macos`` needs a macOS runner and Ubicloud offers none, so there is
#: no fallback to write.
#:
#: ``build-freebsd`` stays GitHub-hosted for a different reason: it is
#: switched off by the ``ENABLE_FREEBSD_RELEASE_BUILDS`` repository variable
#: and has never run, so there is nothing to measure, nothing to speed up, and
#: no way to observe that a move worked. It is pinned here so that staying put
#: reads as a decision, and it moves when somebody re-enables it and can watch
#: it run.
EXPECTED_LITERAL_LABEL: typ.Final = {
    ("coverage-main.yml", "coverage-upload"): UBICLOUD_SMALL,
    ("release.yml", "release"): UBICLOUD_SMALL,
    ("release.yml", "build-macos"): MACOS_LABEL,
    ("release.yml", "build-freebsd"): GITHUB_HOSTED_LABEL,
    ("release.yml", "build-linux"): GITHUB_HOSTED_LABEL,
}

#: The one job whose runner is genuinely not this repository's decision at the
#: point it is written: ``build-and-package.yml`` serves three platforms and
#: takes its label from whichever caller invoked it. The callers above are
#: where that decision is pinned.
CALLEE_PLACED_BY_INPUT: typ.Final = {
    ("build-and-package.yml", "build"): "${{ inputs.runner }}",
}

#: The coordinates whose placement is written as a ``runner`` input rather
#: than a ``runs-on`` line, because ``build-and-package.yml`` takes its runner
#: from its caller. This repository chooses their labels, so they are placed;
#: but they cannot carry a ceiling, because GitHub refuses ``timeout-minutes``
#: on a job with ``uses:``. The callee's ceiling bounds them, and the totality
#: test keeps that callee in the ceiling table so the bound cannot go missing.
PLACED_BY_CALLER_INPUT: typ.Final = frozenset({
    ("release.yml", "build-linux"),
    ("release.yml", "build-macos"),
    ("release.yml", "build-freebsd"),
})

#: Jobs this repository does not place at all: thin callers of reusable
#: workflows that pass no ``runner``, so the callee chooses.
DELEGATED_JOBS: typ.Final = {
    ("mutation-testing.yml", "mutation"),
    ("dependabot-automerge.yml", "automerge"),
    ("release-dry-run.yml", "release"),
}

#: Ceilings, pinned by value rather than bounded, because a ceiling can drift
#: to a number nobody chose while every inequality still holds. Measurements
#: are from the last green run of each workflow on 2026-09-16.
#:
#: ``build-test`` 938 s, ``coverage-upload`` 174 s, ``metadata`` 14 s, and
#: ``build-and-package``'s single job 343 s on its slowest leg. ``release`` is
#: a judgement and is recorded as one in the workflow: it is skipped on every
#: dry run, so no green run of it exists to size from.
#:
#: The three build legs in ``PLACED_BY_CALLER_INPUT`` are absent on purpose:
#: a ``uses:`` job may not declare a ceiling, and the single
#: ``build-and-package.yml`` entry here is the one that bounds all three.
EXPECTED_CEILING_MINUTES: typ.Final = {
    ("ci.yml", "build-test"): 30,
    ("coverage-main.yml", "coverage-upload"): 20,
    ("release.yml", "metadata"): 10,
    ("release.yml", "release"): 15,
    ("build-and-package.yml", "build"): 30,
}

#: GitHub-hosted labels this repository may use without registering them with
#: actionlint, which knows them already. ``macos-15`` is here because the
#: macOS build legs are legitimately hosted, not because macOS is exempt from
#: review: ``EXPECTED_LITERAL_LABEL`` pins that leg by coordinate.
GITHUB_HOSTED_LABELS: typ.Final = frozenset({GITHUB_HOSTED_LABEL, MACOS_LABEL})

#: Prohibited runner families. A prohibition reads by substring on purpose: a
#: renamed or neutered label still leaves its family's text behind.
#: ``macos`` is deliberately absent, because this repository builds macOS
#: packages and must keep a macOS lane; that lane is controlled by coordinate
#: instead.
FOREIGN_RUNNER_FRAGMENTS: typ.Final = ("namespace", "windows", "self-hosted")
