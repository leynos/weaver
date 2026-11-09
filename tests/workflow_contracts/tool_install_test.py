"""Contracts keeping CI's tool installs on prebuilt, pinned releases.

`build-test` used to compile two tools on every run. `cargo install
merman-cli` built Merman from crates.io, a median 2.6 minutes, and the
Whitaker step built `whitaker-installer` from a pinned git revision, cloned
the Whitaker source and compiled the lint suite with Cranelift, a median 1.9
minutes. Both now come from `leynos/shared-actions`: `install-nixie` fetches
Merman's checksum-verified release archive, and `install-whitaker` fetches
the prebuilt, digest-verified lint suite and Dylint tools.

The prohibition runs over every step in every workflow, so a source build
cannot come back under a different step name. The requirements then pin the
replacement down, because a prohibition alone is satisfied by deleting the
install and letting `make lint` or `make nixie` fail on a missing tool, or by
an action that quietly builds from source anyway:

- each install is a shared action pinned to a full commit SHA, so the code
  that runs is the code that was reviewed;
- the Whitaker-specific contract verifies its pinned prebuilt action and
  Cranelift input;
- `install-whitaker` is asked for an installer release new enough to pass
  `--no-source-fallback`, and the inputs that would force or permit a source
  build stay at the values that refuse one;
- each install runs before the step that needs its tool.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ

import pytest
from workflow_loader import repository_workflows as workflows

#: A `cargo install`, with an optional `+toolchain` override. A newline only
#: joins the command when shell escaping continues it; this prohibition fails
#: loudly when it recognises a source build.
CARGO_INSTALL: typ.Final = re.compile(
    r"\bcargo[^\S\r\n]+(?:\\\n[^\S\r\n]*)*(?:\+\S+[^\S\r\n]+(?:\\\n[^\S\r\n]*)*)?install\b"
)

#: A clone of the Whitaker source, which only a source build needs. Escaped
#: newlines may join the command's words; ordinary newlines cannot.
WHITAKER_CLONE: typ.Final = re.compile(
    r"\bgit(?:[^\S\r\n]+(?:\\\n[^\S\r\n]*)*|\\\n[^\S\r\n]+)clone\b"
    r"[^\n]*(?:\\\n[^\n]*)*leynos/whitaker"
)

#: A full commit SHA, the only pin that names one immutable revision.
COMMIT_SHA: typ.Final = re.compile(r"^[0-9a-f]{40}$")

#: The first `whitaker-installer` release with `--no-source-fallback`, which the
#: action always passes. An older installer cannot refuse a source build, so
#: the step would silently compile the lint suite when a prebuilt asset is
#: missing.
MINIMUM_INSTALLER: typ.Final = (0, 2, 9)

LANE: typ.Final = ("ci.yml", "build-test")
NIXIE_ACTION: typ.Final = "leynos/shared-actions/.github/actions/install-nixie"
WHITAKER_ACTION: typ.Final = "leynos/shared-actions/.github/actions/install-whitaker"


def _steps(workflow: str, job: str) -> list[dict[str, object]]:
    """Return one job's steps."""
    definition = workflows()[workflow]["jobs"][job]
    assert isinstance(definition, dict), f"{workflow}:{job} is not a job mapping"
    steps = definition.get("steps", [])
    assert isinstance(steps, list), f"{workflow}:{job} steps must be a list"
    return [step for step in steps if isinstance(step, dict)]


def _every_step() -> list[tuple[str, str, dict[str, object]]]:
    """Return ``(workflow, job, step)`` for every step in every workflow."""
    return [
        (workflow, job, step)
        for workflow, document in workflows().items()
        for job, definition in _mapping(document.get("jobs")).items()
        for step in _mapping(definition).get("steps", []) or []
        if isinstance(step, dict)
    ]


def _mapping(value: object) -> dict[str, typ.Any]:
    """Return `value` when it is a mapping, else an empty one."""
    return value if isinstance(value, dict) else {}


def _action_step(action: str) -> tuple[int, dict[str, object]]:
    """Return the index and step in the lane that uses `action`, exactly once."""
    matches = [
        (index, step)
        for index, step in enumerate(_steps(*LANE))
        if str(step.get("uses", "")).partition("@")[0] == action
    ]
    assert len(matches) == 1, (
        f"{LANE[0]}:{LANE[1]} must use {action} exactly once, found {len(matches)}"
    )
    return matches[0]


def _run_index(command: str) -> int:
    """Return the index of the lane step whose whole command is `command`."""
    matches = [
        index
        for index, step in enumerate(_steps(*LANE))
        if str(step.get("run", "")).strip() == command
    ]
    assert len(matches) == 1, (
        f"{LANE[0]}:{LANE[1]} must run `{command}` as exactly one step's whole "
        f"command, found {len(matches)}"
    )
    return matches[0]


#: The version grammar the pinned actions accept: one to three numeric
#: components, none with a leading zero. `install-whitaker` pads a short
#: version to three components before comparing it with its floor, and
#: rejects `0.2.09` precisely because of the leading zero.
_ACCEPTED_VERSION: typ.Final = re.compile(
    r"(0|[1-9][0-9]*)(\.(0|[1-9][0-9]*)){0,2}"
)


def _version(text: str) -> tuple[int, ...]:
    """Parse an action version, normalised to `(MAJOR, MINOR, PATCH)`.

    A caller may write one to three numeric components without a leading zero,
    which is what `install-whitaker` validates before padding the omitted
    components with zero. Comparing a short version as written would order
    `1.0` below `0.2.9` instead of above it, so the result always has three
    components.

    Examples
    --------
    >>> _version("0.2.9")
    (0, 2, 9)
    >>> _version("1.0")
    (1, 0, 0)
    """
    assert _ACCEPTED_VERSION.fullmatch(text), (
        f"{text!r} is not one to three numeric components without a leading zero"
    )
    parts = [int(part) for part in text.split(".")]
    return (*parts, *([0] * (3 - len(parts))))


def _normalise_shell_continuations(script: str) -> str:
    """Collapse Bash backslash-newline pairs while retaining command boundaries."""
    return re.sub(r"\\\r?\n", "", script)


def _matches_source_build(script: str, detector: re.Pattern[str]) -> bool:
    """Match one source-build detector after normalising Bash continuations."""
    return bool(detector.search(_normalise_shell_continuations(script)))


def _source_build_offenders(
    steps: typ.Iterable[tuple[str, str, dict[str, object]]],
) -> list[str]:
    """Return workflow steps that build tools from source instead of releases."""
    return [
        f"{workflow}:{job} step {step.get('name', '')!r}"
        for workflow, job, step in steps
        if any(
            _matches_source_build(str(step.get("run", "")), detector)
            for detector in (CARGO_INSTALL, WHITAKER_CLONE)
        )
    ]


def test_no_step_compiles_a_tool_from_source() -> None:
    """No workflow step runs `cargo install` or clones the Whitaker source."""
    offenders = _source_build_offenders(_every_step())
    assert not offenders, (
        "these steps build a tool from source instead of installing a "
        f"prebuilt release: {', '.join(offenders)}"
    )


@pytest.mark.parametrize(
    ("script", "matches_install"),
    [
        pytest.param(
            "cargo install cargo-dylint",
            True,
            id="single-line-install",
        ),
        pytest.param(
            "cargo \\\n  install cargo-dylint",
            True,
            id="continued-install",
        ),
        pytest.param(
            "cargo\ninstall cargo-dylint",
            False,
            id="uncontinued-multiline-script",
        ),
    ],
)
def test_cargo_install_detection_respects_shell_continuations(
    script: str, matches_install: bool
) -> None:
    """Only escaped newlines join a `cargo install` source build command."""
    assert _matches_source_build(script, CARGO_INSTALL) is matches_install, (
        f"expected cargo install detection {matches_install} for {script!r}"
    )


@pytest.mark.parametrize(
    ("script", "matches_clone"),
    [
        pytest.param(
            "git clone https://github.com/leynos/whitaker",
            True,
            id="single-line-clone",
        ),
        pytest.param(
            "git \\\n  clone https://github.com/leynos/whitaker",
            True,
            id="continued-git-to-clone-with-space",
        ),
        pytest.param(
            "git\\\n  clone https://github.com/leynos/whitaker",
            True,
            id="continued-git-to-clone-without-space",
        ),
        pytest.param(
            "git\nclone https://github.com/leynos/whitaker",
            False,
            id="uncontinued-git-to-clone",
        ),
        pytest.param(
            "git clone \\\n  https://github.com/leynos/whitaker",
            True,
            id="continued-clone-to-url-with-space",
        ),
        pytest.param(
            "git clone\\\n  https://github.com/leynos/whitaker",
            True,
            id="continued-clone-to-url-without-space",
        ),
        pytest.param(
            "git clone\nhttps://github.com/leynos/whitaker",
            False,
            id="uncontinued-clone-to-url",
        ),
        pytest.param(
            "git clone https://github.com/example/other\n"
            "printf '%s\\n' https://github.com/leynos/whitaker",
            False,
            id="unrelated-multiline-script",
        ),
    ],
)
def test_whitaker_clone_detection_respects_shell_continuations(
    script: str, matches_clone: bool
) -> None:
    """Only clone commands joined by a shell continuation identify source builds."""
    assert _matches_source_build(script, WHITAKER_CLONE) is matches_clone, (
        f"expected Whitaker clone detection {matches_clone} for {script!r}"
    )


@pytest.mark.parametrize(
    ("step_name", "script", "expected_offender"),
    [
        pytest.param(
            "Install Dylint source",
            "cargo \\\n  install cargo-dylint",
            "mutated.yml:source-build step 'Install Dylint source'",
            id="continued-cargo-install",
        ),
        pytest.param(
            "Clone Whitaker source",
            "git clone \\\n  https://github.com/leynos/whitaker",
            "mutated.yml:source-build step 'Clone Whitaker source'",
            id="continued-whitaker-clone",
        ),
        pytest.param(
            "Clone Whitaker with continued command",
            "git \\\n  clone https://github.com/leynos/whitaker",
            "mutated.yml:source-build step 'Clone Whitaker with continued command'",
            id="continued-git-to-clone",
        ),
        pytest.param(
            "Clone Whitaker with split URL",
            "git clone https://github.com/leynos/\\\nwhitaker",
            "mutated.yml:source-build step 'Clone Whitaker with split URL'",
            id="continued-whitaker-url",
        ),
    ],
)
def test_contract_rejects_continued_source_build(
    step_name: str, script: str, expected_offender: str
) -> None:
    """A shell continuation cannot conceal a source build from the gate."""
    offenders = _source_build_offenders(
        [
            (
                "mutated.yml",
                "source-build",
                {
                    "name": step_name,
                    "run": script,
                },
            )
        ]
    )
    assert offenders == [expected_offender], (
        f"a continued source build must be rejected: {step_name!r}"
    )


@pytest.mark.parametrize("action", [NIXIE_ACTION, WHITAKER_ACTION])
def test_each_install_is_a_shared_action_pinned_to_a_commit(action: str) -> None:
    """The install is pinned to a full SHA, so the reviewed code is what runs."""
    _, step = _action_step(action)
    ref = str(step["uses"]).partition("@")[2]
    assert COMMIT_SHA.fullmatch(ref), (
        f"{action} must be pinned to a commit SHA, not {ref!r}"
    )


def test_merman_comes_from_the_verified_release() -> None:
    """`install-nixie` installs the verified Merman release on a usable Python.

    nixie-cli 1.1.0 requires Python 3.14, so a lower `python-version` fails
    the install; the action's default is 3.14.
    """
    _, step = _action_step(NIXIE_ACTION)
    inputs = step.get("with", {})
    assert isinstance(inputs, dict), "install-nixie must declare its inputs"
    assert inputs.get("merman-version") == "0.7.0", (
        "install-nixie must install Merman 0.7.0, the release whose archive "
        f"checksum the action pins; got {inputs.get('merman-version')!r}"
    )
    assert inputs.get("nixie-version") == "1.1.0", (
        "install-nixie must install Nixie 1.1.0, which requires the Python "
        f"version checked below; got {inputs.get('nixie-version')!r}"
    )
    python = str(inputs.get("python-version", "3.14"))
    assert _version(python) >= (3, 14), (
        f"install-nixie's python-version {python} is below 3.14, which "
        "nixie-cli 1.1.0 requires; the install fails to resolve"
    )


@pytest.mark.parametrize(
    ("action", "command"),
    [(NIXIE_ACTION, "make nixie"), (WHITAKER_ACTION, "make lint")],
)
def test_each_install_runs_before_the_step_that_needs_it(
    action: str, command: str
) -> None:
    """The tool is on the runner before the gate that calls it."""
    install, _ = _action_step(action)
    assert install < _run_index(command), (
        f"{action} must run before `{command}` in {LANE[0]}:{LANE[1]}"
    )


def test_whitaker_cannot_fall_back_to_a_source_build() -> None:
    """The installer is new enough to refuse a source build, and does.

    `install-whitaker` always passes `--no-source-fallback`, which only exists
    from installer 0.2.9. The caller must therefore ask for at least that
    release. `suite-version` is refused by the action when set (a pin would
    force a source build), and `ci-mode` turns off the check that catches a
    missing prebuilt asset, so a caller that unsets it would let the step
    proceed without one.
    """
    _, step = _action_step(WHITAKER_ACTION)
    inputs = step.get("with", {})
    assert isinstance(inputs, dict), "install-whitaker must declare its inputs"

    # An absent input takes the action's default, 0.2.11, which is new enough.
    version = str(inputs.get("installer-version", "0.2.11"))
    assert _version(version) >= MINIMUM_INSTALLER, (
        f"installer-version {version} predates --no-source-fallback "
        f"({'.'.join(map(str, MINIMUM_INSTALLER))}), so the step could "
        "compile the lint suite when a prebuilt asset is missing"
    )
    assert not inputs.get("suite-version"), (
        "suite-version pins the rolling lint suite, which forces a source build"
    )
    # An unquoted `true` arrives as a boolean and the runner lowercases it, so
    # `True` is what the action's `true|false` case sees. A quoted `"TRUE"`
    # reaches the action verbatim and fails that case, so it must not pass here.
    ci_mode = inputs.get("ci-mode", "true")
    assert ci_mode is True or ci_mode == "true", (
        "ci-mode must stay on, so an unpublished prebuilt asset fails the step "
        "instead of being built from source"
    )
