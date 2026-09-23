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
- `install-whitaker` asks for an installer at or above 0.2.7, the first to
  take the prebuilt Dylint path, and neither pins the lint suite nor turns off
  `ci-mode`, since either forces or permits a source build;
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

#: The first installer release that takes the prebuilt Dylint path.
MINIMUM_INSTALLER: typ.Final = (0, 2, 7)

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


def _version(text: str) -> tuple[int, ...]:
    """Parse a dotted numeric version, refusing anything else."""
    assert re.fullmatch(r"\d+\.\d+\.\d+", text), f"{text!r} is not MAJOR.MINOR.PATCH"
    return tuple(int(part) for part in text.split("."))


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
    assert _version(f"{python}.0" if python.count(".") == 1 else python) >= (3, 14), (
        f"install-nixie's python-version {python} is below 3.14, which "
        "nixie-cli 1.1.0 requires; the install fails to resolve"
    )


def test_whitaker_takes_the_prebuilt_path() -> None:
    """The installer is new enough, and nothing forces or permits a source build."""
    _, step = _action_step(WHITAKER_ACTION)
    inputs = step.get("with", {})
    assert isinstance(inputs, dict), "install-whitaker must declare its inputs"
    version = str(inputs.get("installer-version", ""))
    assert _version(version) >= MINIMUM_INSTALLER, (
        f"installer-version {version} predates the prebuilt Dylint path "
        f"({'.'.join(map(str, MINIMUM_INSTALLER))})"
    )
    assert not inputs.get("suite-version"), (
        "suite-version pins the lint suite, which forces a source build"
    )
    assert str(inputs.get("ci-mode", "true")).lower() == "true", (
        "ci-mode must stay on, so a fallback to a source build fails the step"
    )
    assert str(inputs.get("allow-suite-pin", "false")).lower() == "false", (
        "allow-suite-pin permits the source build ci-mode exists to refuse"
    )


def test_development_backend_and_linker_precede_lint() -> None:
    """The Linux lint lane provisions the backend and linker selected by Make."""
    matches = [
        (index, str(step.get("run", "")))
        for index, step in enumerate(_steps(*LANE))
        if step.get("name") == "Provision development backend and linker"
    ]
    assert len(matches) == 1, "the lint lane needs one backend setup step"
    index, script = matches[0]
    assert "rustup component add rustc-codegen-cranelift" in script
    assert "apt-get install --yes mold" in script
    assert index < _action_step(WHITAKER_ACTION)[0] < _run_index("make lint")


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
