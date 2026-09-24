"""Contract tests for runner placement, ceilings, and the actionlint registry.

This module holds the assertions. The reviewed decisions and the reasons for
them live in ``runner_placement_policy``, and the machinery that reads the
workflow tree lives in ``runner_placement_reader``. The split is by role:
what a person decided, what is derived from the tree, and what enforces the
one against the other.

Five things here are easy to get wrong in ways a green run does not show.

First, the folded scalar. Written as

.. code-block:: yaml

    runs-on: >-
      ${{ github.event.pull_request.head.repo.fork
          && 'ubuntu-latest' || 'ubicloud-standard-4' }}

the more-indented continuation keeps its line break, so the parsed value
carries a newline in the middle of the expression. GitHub evaluates it
regardless and the job runs, so nothing fails and nothing is reported. Every
guard here is on the *parsed* value, not on the file's text.

Second, the narrowness of the expression check. A contract that matches the
shape of the expression rather than its content passes when
``head.repo.fork`` is replaced by a sibling field that reads just as
plausibly and selects the wrong runner. The guard and both arms are compared
by equality against exact strings.

Third, where a placement decision is written. Four of weaver's placements are
not ``runs-on`` lines at all: ``release.yml``'s build matrix passes a
``runner`` input to ``build-and-package.yml``. A contract reading only
``runs-on`` would see those jobs as declaring no runner and would treat them
as somebody else's decision, when this repository chooses their labels. It
would also miss them in the registry, because the labels they select appear
nowhere else.

Fourth, which lanes meet forks. ``release.yml`` looks like a tag workflow,
but ``release-dry-run.yml`` calls it on every pull request, so its
``metadata`` and build jobs do meet forks and do need the fallback. Its
``release`` job does not, because it is gated on ``should_publish``. The
distinction is recorded in two tables rather than inferred, so a reader sees
that the bare label was chosen rather than forgotten.

Fifth, the totality of the tables. Every other assertion here iterates a
table, and an iterated table cannot report what was never put in it.
``test_the_reviewed_tables_are_total`` compares the tables against the tree
and against each other, so a lane cannot escape review by being left out of
one of them.

On ceilings: a job that calls a reusable workflow may not declare
``timeout-minutes`` at all, because GitHub does not accept the key there. So
the ceiling-free set is not a judgement call here, it is exactly the set of
``uses:`` jobs, and ``test_a_delegating_job_declares_no_ceiling`` asserts the
absence with that reason. The callee's own ceiling is what bounds those runs,
which is why ``build-and-package.yml``'s ``build`` carries one.

Mutation proof; each applied alone and reverted, recorded in the pull
request with the failing test names.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest
from runner_placement_policy import (
    CALLEE_PLACED_BY_INPUT,
    DELEGATED_JOBS,
    PLACED_BY_CALLER_INPUT,
    EXPECTED_CEILING_MINUTES,
    EXPECTED_FORK_FALLBACK,
    EXPECTED_LITERAL_LABEL,
    FOREIGN_RUNNER_FRAGMENTS,
    FORK_GUARD,
    GITHUB_HOSTED_LABEL,
    GITHUB_HOSTED_LABELS,
    UBICLOUD_LABEL_PREFIX,
)
from runner_placement_reader import (
    RUNNER_EXPRESSION,
    WORKFLOW_DIR,
    caller_runner_input,
    case_id,
    job_labels,
    jobs,
    labels_in_use,
    registered_labels,
    runner_declarations,
)

pytestmark = pytest.mark.skipif(
    not WORKFLOW_DIR.is_dir(),
    reason="workflow files not present in this working copy",
)

#: Every coordinate this repository places, by whichever mechanism.
PLACED_JOBS = (
    set(EXPECTED_FORK_FALLBACK) | set(EXPECTED_LITERAL_LABEL) | set(CALLEE_PLACED_BY_INPUT)
)


def _sole_declaration(coordinate: tuple[str, str]) -> object:
    """Return the one runner declaration a placed job makes.

    A job declaring two, a ``runs-on`` and a ``runner`` input, is ambiguous
    about which one placed it, so it is refused rather than read.
    """
    declarations = runner_declarations(jobs()[coordinate])
    assert len(declarations) == 1, (
        f"{coordinate[0]}:{coordinate[1]} should make exactly one runner "
        f"declaration; found {declarations!r}"
    )
    return declarations[0]


def test_every_job_is_pinned_by_coordinate() -> None:
    """Scenario: a lane is added and nobody decides where it runs.

    Invariant: the tree's jobs are exactly the placed ones plus the
    delegating ones. Iterating only the pinned coordinates would never
    examine a job nobody listed, so a new lane could carry any label at all
    and satisfy every other assertion here.
    """
    observed = set(jobs())
    expected = PLACED_JOBS | DELEGATED_JOBS
    assert observed == expected, (
        "runner placement is pinned per job; unpinned jobs "
        f"{sorted(observed - expected)} and stale pins "
        f"{sorted(expected - observed)} must be reconciled"
    )


def test_disposable_whitaker_probe_excludes_fork_pull_requests() -> None:
    """The temporary diagnostic cannot request its runner from a fork."""
    diagnostic = jobs()[
        ("pr303-whitaker-ubuntu2204-diagnostic.yml", "whitaker-probe")
    ]
    assert diagnostic.get("if") == (
        "github.event.pull_request.head.repo.full_name == github.repository && "
        "github.head_ref == 'diagnose-pr303-whitaker-ubuntu2204-20260924'"
    )


def test_the_reviewed_tables_are_total() -> None:
    """Scenario: a lane is added to one reviewed table but not the others.

    Invariant: the tables agree about which coordinates they cover. Three
    gaps close here, and each is one an iterated table cannot report.

    A coordinate in two placement tables would be checked twice under
    contradictory expectations, and whichever test ran second would be the
    one that mattered.

    A placed job absent from the ceiling table passes both ceiling tests: the
    per-coordinate one iterates that table, and the tree walk reads only
    ``ubicloud-`` lanes, so a new GitHub-hosted lane would inherit the
    six-hour default unremarked.

    A delegating job listed as placed, or the reverse, would be asserted
    against the wrong mechanism entirely.
    """
    tables = (
        ("fork fallback", set(EXPECTED_FORK_FALLBACK)),
        ("literal label", set(EXPECTED_LITERAL_LABEL)),
        ("callee placed by input", set(CALLEE_PLACED_BY_INPUT)),
        ("delegated", DELEGATED_JOBS),
    )
    for index, (name, coordinates) in enumerate(tables):
        for other_name, other in tables[index + 1 :]:
            overlap = coordinates & other
            assert not overlap, (
                f"{sorted(overlap)} appears in both the {name} and "
                f"{other_name} tables; a coordinate is placed one way or it "
                "is not placed"
            )
    assert PLACED_BY_CALLER_INPUT <= PLACED_JOBS, (
        "a coordinate placed by caller input must also be a placed job; "
        f"{sorted(PLACED_BY_CALLER_INPUT - PLACED_JOBS)} is not"
    )
    needing_ceilings = PLACED_JOBS - PLACED_BY_CALLER_INPUT
    assert set(EXPECTED_CEILING_MINUTES) == needing_ceilings, (
        "every placed job that declares its own runs-on needs a reviewed "
        f"ceiling; missing {sorted(needing_ceilings - set(EXPECTED_CEILING_MINUTES))} "
        f"and stale {sorted(set(EXPECTED_CEILING_MINUTES) - needing_ceilings)}"
    )


@pytest.mark.parametrize(
    ("coordinate", "label"), sorted(EXPECTED_LITERAL_LABEL.items()), ids=case_id
)
def test_the_job_carries_its_reviewed_label(
    coordinate: tuple[str, str], label: str
) -> None:
    """Scenario: a lane drifts onto a larger shape, or back to a hosted one.

    Invariant: the job declares exactly the reviewed label and nothing
    beside it. Equality, not containment: a check that the declaration
    merely contains ``ubicloud-`` accepts ``ubicloud-standard-16`` as readily
    as the reviewed shape, and size is the thing being reviewed.
    """
    declared = _sole_declaration(coordinate)
    assert declared == label, (
        f"{coordinate[0]}:{coordinate[1]} must run on exactly {label!r}, "
        f"got {declared!r}"
    )


@pytest.mark.parametrize("coordinate", sorted(EXPECTED_FORK_FALLBACK), ids=case_id)
def test_the_runner_expression_parses_to_one_line(coordinate: tuple[str, str]) -> None:
    """Scenario: the folded scalar's continuation is indented one level deeper.

    Invariant: the parsed value is a single line. A more-indented
    continuation keeps its line break, putting a newline inside the
    expression. GitHub evaluates the broken value and the job runs, so a
    green run is not evidence; only the parsed value shows it.
    """
    value = _sole_declaration(coordinate)
    assert isinstance(value, str), (
        f"{coordinate[0]}:{coordinate[1]} should declare its runner as one "
        f"scalar, got {value!r}"
    )
    assert "\n" not in value, (
        f"{coordinate[0]}:{coordinate[1]} has a line break inside its runner "
        f"expression ({value!r}); the folded scalar's continuation line must "
        "sit at the same indent as the line above it"
    )


@pytest.mark.parametrize(
    ("coordinate", "label"), sorted(EXPECTED_FORK_FALLBACK.items()), ids=case_id
)
def test_the_runner_expression_is_exactly_the_reviewed_one(
    coordinate: tuple[str, str], label: str
) -> None:
    """Scenario: the fork guard is swapped for a plausible sibling field.

    Invariant: the guard and both arms equal the reviewed strings. Matching
    the expression's shape rather than its content passes when
    ``head.repo.fork`` becomes, say, ``head.repo.private``: an expression of
    exactly the same form that sends every fork pull request to a runner it
    cannot obtain, and every other pull request to the wrong place.
    """
    value = str(_sole_declaration(coordinate))
    match = RUNNER_EXPRESSION.match(value)
    assert match is not None, (
        f"{coordinate[0]}:{coordinate[1]} should declare the reviewed fork "
        f"fallback expression; got {value!r}"
    )
    assert match["guard"] == FORK_GUARD, (
        f"the fallback is keyed on {match['guard']!r}; only {FORK_GUARD!r} "
        "identifies a fork"
    )
    assert match["fork_arm"] == GITHUB_HOSTED_LABEL, (
        f"forks are sent to {match['fork_arm']!r}; a fork cannot obtain an "
        f"Ubicloud runner, so the fork arm must be {GITHUB_HOSTED_LABEL!r}"
    )
    assert match["default_arm"] == label, (
        f"non-fork events run on {match['default_arm']!r}, not the reviewed "
        f"{label!r}"
    )


@pytest.mark.parametrize(
    ("coordinate", "expression"), sorted(CALLEE_PLACED_BY_INPUT.items()), ids=case_id
)
def test_the_shared_build_job_takes_its_runner_from_its_caller(
    coordinate: tuple[str, str], expression: str
) -> None:
    """Scenario: the shared build job acquires a label of its own.

    Invariant: it defers to the caller. One job definition serves Linux,
    macOS and a FreeBSD cross build, so a label written here would place all
    three at once, and two of them wrongly. Pinning the expression rather
    than merely checking that one is present stops the input being renamed
    or defaulted out from under the callers.
    """
    declared = jobs()[coordinate].get("runs-on")
    assert declared == expression, (
        f"{coordinate[0]}:{coordinate[1]} serves several platforms and must "
        f"take its runner from its caller as {expression!r}, got {declared!r}"
    )


@pytest.mark.parametrize(
    ("coordinate", "minutes"), sorted(EXPECTED_CEILING_MINUTES.items()), ids=case_id
)
def test_the_job_declares_its_reviewed_ceiling(
    coordinate: tuple[str, str], minutes: int
) -> None:
    """Scenario: a ceiling drifts to a number nobody reviewed.

    Invariant: each job carries the exact ceiling recorded for it. A
    per-minute runner bills until something stops it, so the six-hour
    default is one failure mode; a ceiling near the measured work is the
    other, because it cancels the run at the moment the overrun becomes
    interesting and discards the log that would explain it.
    """
    declared = jobs()[coordinate].get("timeout-minutes")
    assert declared == minutes, (
        f"{coordinate[0]}:{coordinate[1]} must set timeout-minutes: "
        f"{minutes}, got {declared!r}"
    )


def test_every_ubicloud_job_has_a_ceiling() -> None:
    """Scenario: a new Ubicloud lane inherits GitHub's six-hour default.

    Invariant: every job that can select any Ubicloud label declares a
    ceiling. The pinned table is keyed by coordinate; this reads the tree
    instead, so a lane cannot escape by being absent from a list, and it
    tests the label prefix so a right-sized shape stays covered.

    A caller that passes an Ubicloud label to a reusable workflow is exempt,
    because GitHub refuses ``timeout-minutes`` on a ``uses:`` job; the
    callee's ceiling bounds it, and the totality test keeps that callee in
    the ceiling table.
    """
    unbounded = [
        coordinate
        for coordinate, definition in jobs().items()
        if any(
            label.startswith(UBICLOUD_LABEL_PREFIX)
            for label in job_labels(definition)
        )
        and "uses" not in definition
        and definition.get("timeout-minutes") is None
    ]
    assert not unbounded, (
        f"jobs on an Ubicloud runner without a ceiling: {sorted(unbounded)}"
    )


@pytest.mark.parametrize(
    "coordinate", sorted(DELEGATED_JOBS | PLACED_BY_CALLER_INPUT), ids=case_id
)
def test_a_delegating_job_declares_no_ceiling(coordinate: tuple[str, str]) -> None:
    """Scenario: someone adds the "missing" ceiling to a reusable caller.

    Invariant: a job with ``uses:`` declares none, because GitHub does not
    accept ``timeout-minutes`` there and the workflow would fail to parse.
    The absence is asserted rather than merely left, so it reads as a
    constraint rather than as the gap the neighbouring test looks for. What
    bounds these runs is the callee's own ceiling, which the totality test
    keeps in the ceiling table.
    """
    definition = jobs()[coordinate]
    assert "uses" in definition, (
        f"{coordinate[0]}:{coordinate[1]} is recorded as delegating but "
        "declares no uses key"
    )
    assert definition.get("timeout-minutes") is None, (
        f"{coordinate[0]}:{coordinate[1]} calls a reusable workflow, so it "
        "must not declare timeout-minutes: GitHub rejects the key there and "
        "the callee's own ceiling bounds the run"
    )


@pytest.mark.parametrize("coordinate", sorted(DELEGATED_JOBS), ids=case_id)
def test_a_delegated_job_chooses_no_runner(coordinate: tuple[str, str]) -> None:
    """Scenario: a scheduled or administrative lane acquires a runner.

    Invariant: these coordinates call a reusable workflow and pass no
    ``runner``, so the callee places them. Either a ``runs-on`` or a
    ``runner`` input appearing here is this repository taking a placement
    decision it has not reviewed.
    """
    definition = jobs()[coordinate]
    assert "uses" in definition, (
        f"{coordinate[0]}:{coordinate[1]} is recorded as a reusable-workflow "
        "caller but declares no uses key"
    )
    assert not runner_declarations(definition), (
        f"{coordinate[0]}:{coordinate[1]} delegates placement to its callee; "
        f"found {runner_declarations(definition)!r}"
    )
    assert caller_runner_input(definition) is None, (
        f"{coordinate[0]}:{coordinate[1]} must pass no runner input"
    )


def test_the_actionlint_registry_matches_the_labels_in_use() -> None:
    """Scenario: the registry and the workflows drift apart.

    Invariant: the registered labels are exactly the non-GitHub-hosted
    labels in use, compared in both directions. An unregistered label makes
    actionlint report a lane nobody broke; a registered label no lane uses
    is worse, because it silently permits a runner family nobody reviewed
    for whatever lane adopts it next. "In use" includes the labels the
    release workflow passes as ``runner`` inputs, which appear in no
    ``runs-on`` line anywhere.
    """
    needing_registration = labels_in_use() - set(GITHUB_HOSTED_LABELS)
    registered = registered_labels()
    assert registered == needing_registration, (
        "the actionlint runner registry must hold exactly the labels in use; "
        f"unregistered {sorted(needing_registration - registered)} and stale "
        f"{sorted(registered - needing_registration)}"
    )


def test_no_lane_uses_a_foreign_runner_family() -> None:
    """Scenario: a lane acquires a Namespace, Windows or ad-hoc self-hosted runner.

    Invariant: this repository has no such lane. The reading is by
    substring, which is the safe direction for a prohibition: a renamed or
    neutered label still leaves its family's text behind. macOS is
    deliberately not prohibited, because the release matrix needs it; that
    lane is controlled by coordinate instead.
    """
    offenders = sorted(
        label
        for label in labels_in_use()
        if any(fragment in label.lower() for fragment in FOREIGN_RUNNER_FRAGMENTS)
    )
    assert not offenders, (
        "this repository has no Namespace, Windows or ad-hoc self-hosted "
        f"lane; found {offenders}"
    )
