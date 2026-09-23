"""Reading machinery for the runner-placement contract.

This module knows how to get facts out of the workflow tree. It holds no
opinion about where anything should run: every reviewed decision lives in
``runner_placement_policy``, and the assertions that hold one against the
other live in ``ci_runner_placement_test``.

Four readings here are less obvious than they look, and each exists because
the obvious reading has a failure mode that no green run would show.

``workflow_paths`` reads both spellings of the extension. GitHub runs a
workflow written either way, so reading only ``*.yml`` would leave a lane in
``*.yaml`` outside every assertion in the contract.

``runner_value`` returns the *parsed* value rather than the file's text. A
folded scalar whose continuation is indented one level deeper keeps its line
break, so the parsed value carries a newline in the middle of an expression.
GitHub evaluates it regardless and the job runs, which is why the contract
reads the parse and not the source.

``caller_runner_input`` reads the ``runner`` value a job passes to a reusable
workflow. weaver places four jobs that way rather than with ``runs-on``,
because ``build-and-package.yml`` takes its runner as a ``workflow_call``
input. A contract that read only ``runs-on`` would see those four jobs as
declaring no runner at all and would call them somebody else's problem, when
in fact this repository chooses their labels.

``declaration_labels`` counts both arms of a conditional, and returns nothing
for an expression with no quoted literal. The second half matters for
``build-and-package.yml``'s own ``runs-on: ${{ inputs.runner }}``: that job
selects whatever its caller passed, so it contributes no label of its own to
the registry question, and reading its text as a label would register the
string ``${{ inputs.runner }}``.
"""

from __future__ import annotations

import functools
import re
import typing as typ
from pathlib import Path

from workflow_loader import load_workflow, read_workflows

REPO_ROOT: typ.Final = Path(__file__).resolve().parents[2]
WORKFLOW_DIR: typ.Final = REPO_ROOT / ".github" / "workflows"
ACTIONLINT_CONFIG: typ.Final = REPO_ROOT / ".github" / "actionlint.yaml"

#: The input name a reusable-workflow caller uses to place its callee.
RUNNER_INPUT: typ.Final = "runner"

#: One fork-fallback expression, anchored end to end. ``[^'\n]`` in the arms
#: and ``\S`` in the guard keep a value carrying an embedded line break from
#: matching here as well, so the line-break failure is reported by its own
#: test rather than arriving as a confusing "expression not recognized".
RUNNER_EXPRESSION: typ.Final = re.compile(
    r"^\$\{\{ (?P<guard>\S+)"
    r" && '(?P<fork_arm>[^'\n]*)'"
    r" \|\| '(?P<default_arm>[^'\n]*)' \}\}$"
)

#: Every quoted literal in an expression, used to read the labels a lane can
#: actually select.
EXPRESSION_LITERAL: typ.Final = re.compile(r"'([^'\n]*)'")

#: The one expression that may name no label of its own: a reusable workflow
#: selecting whatever runner its caller passed. Any other literal-free
#: expression, such as ``${{ matrix.os }}``, selects labels this reader cannot
#: see, so it is refused rather than read as selecting nothing.
CALLER_INPUT_EXPRESSION: typ.Final = re.compile(
    r"^\$\{\{\s*inputs\.[A-Za-z_][\w-]*\s*\}\}$"
)


class RunnerShapeError(ValueError):
    """A runner declaration this contract cannot read."""


def case_id(value: object) -> str:
    """Render one parametrized case identifier.

    A coordinate is a ``(workflow, job id)`` tuple, which pytest would
    otherwise render as an opaque index.

    Examples
    --------
    >>> case_id(("ci.yml", "build-test"))
    'ci.yml-build-test'
    >>> case_id(30)
    '30'
    """
    if isinstance(value, tuple):
        return "-".join(str(item) for item in value)
    return str(value)


@functools.cache
def _parsed_workflows() -> tuple[tuple[str, dict[str, object]], ...]:
    """Read and parse the workflow tree once, in a stable order.

    This is the module's only entry point to the filesystem for workflow
    documents. Every query below derives from its result, so the tree is read
    and parsed once per session rather than once per parametrized case, and a
    parse failure is reported from one place instead of from whichever
    assertion happened to ask first.

    Returns
    -------
    tuple[tuple[str, dict[str, object]], ...]
        Each workflow's file name paired with its parsed document, sorted by
        path.
    """
    documents = tuple(sorted(read_workflows(WORKFLOW_DIR).items()))
    assert documents, "the repository should define at least one workflow"
    return documents


def workflow_paths() -> list[Path]:
    """Return every workflow document's path, in a stable order.

    Both spellings of the extension are read, because GitHub runs a workflow
    written either way.

    Returns
    -------
    list[Path]
        Every workflow document under ``.github/workflows``, sorted by path.

    Examples
    --------
    >>> "ci.yml" in [path.name for path in workflow_paths()]
    True
    """
    return [WORKFLOW_DIR / name for name, _ in _parsed_workflows()]


def workflows() -> dict[str, dict[str, object]]:
    """Parse every workflow document, keyed by file name.

    Returns
    -------
    dict[str, dict[str, object]]
        A fresh mapping of file name to parsed document. The mapping is
        rebuilt on each call so that a caller mutating it cannot disturb
        another assertion; the parse behind it is cached.

    Examples
    --------
    >>> "ci.yml" in workflows()
    True
    """
    return dict(_parsed_workflows())


def jobs() -> dict[tuple[str, str], dict[str, object]]:
    """Return every job in the repository, keyed by ``(workflow, job id)``.

    Examples
    --------
    >>> ("ci.yml", "build-test") in jobs()
    True
    """
    keyed: dict[tuple[str, str], dict[str, object]] = {}
    for name, document in workflows().items():
        for job_id, definition in ((document or {}).get("jobs") or {}).items():
            keyed[(name, job_id)] = definition
    return keyed


def runner_value(definition: dict[str, object]) -> object | None:
    """Return a job's parsed ``runs-on`` value, if it declares one.

    Examples
    --------
    >>> runner_value({"runs-on": "ubuntu-latest"})
    'ubuntu-latest'
    >>> runner_value({"uses": "./reusable.yml"}) is None
    True
    """
    return definition.get("runs-on")


def caller_runner_input(definition: dict[str, object]) -> object | None:
    """Return the ``runner`` a job passes to the reusable workflow it calls.

    This is how ``release.yml`` places its build matrix, so it is a placement
    decision this repository makes and owes an assertion for, even though the
    job declares no ``runs-on`` of its own.

    Examples
    --------
    >>> caller_runner_input({"uses": "./b.yml", "with": {"runner": "macos-15"}})
    'macos-15'
    >>> caller_runner_input({"uses": "./b.yml", "with": {"platform": "linux"}}) is None
    True
    >>> caller_runner_input({"runs-on": "ubuntu-latest"}) is None
    True
    """
    supplied = definition.get("with")
    if not isinstance(supplied, dict):
        return None
    return supplied.get(RUNNER_INPUT)


def runs_on_declarations(declared: object) -> list[str]:
    """Return the label declarations one ``runs-on`` value makes.

    GitHub accepts three forms: a scalar label or expression, a sequence of
    labels a runner must all carry, and a mapping with ``group`` and
    ``labels``. Each is read; anything else is refused rather than read as
    declaring no runner, because a reading of "no runner" exempts the lane
    from every placement, ceiling and registry assertion at once.

    A mapping is read only when ``labels`` is its sole key, so a ``group``
    is refused. This repository places no lane by runner group, and a group
    selects runners by an organization setting this contract cannot read, so
    admitting one is a reviewed decision with its own assertion, not
    something to fall through to.

    Examples
    --------
    >>> runs_on_declarations("ubuntu-latest")
    ['ubuntu-latest']
    >>> runs_on_declarations(["self-hosted", "linux"])
    ['self-hosted', 'linux']
    >>> runs_on_declarations({"labels": "ubicloud-standard-4"})
    ['ubicloud-standard-4']

    Raises
    ------
    RunnerShapeError
        When the value is none of the three forms, is empty, carries a
        non-string label, or names a runner group.
    """
    match declared:
        case str():
            return [declared]
        case list() if declared and all(isinstance(x, str) for x in declared):
            return list(declared)
        case {"labels": str() | list() as labels} if len(declared) == 1:
            return runs_on_declarations(labels)
    message = f"unreadable runs-on {declared!r}"
    raise RunnerShapeError(message)


def runner_declarations(definition: dict[str, object]) -> list[object]:
    """Return every runner declaration a job makes, from either mechanism.

    A ``runs-on`` may itself be a list of labels, or a mapping holding one,
    which is why this flattens rather than returning a single value.

    Examples
    --------
    >>> runner_declarations({"runs-on": ["self-hosted", "linux"]})
    ['self-hosted', 'linux']
    >>> runner_declarations({"uses": "./b.yml", "with": {"runner": "macos-15"}})
    ['macos-15']
    >>> runner_declarations({})
    []

    Raises
    ------
    RunnerShapeError
        When either declaration is not a shape GitHub accepts.
    """
    declarations: list[object] = []
    declared = runner_value(definition)
    if declared is not None:
        declarations.extend(runs_on_declarations(declared))
    supplied = caller_runner_input(definition)
    if supplied is not None:
        if not isinstance(supplied, str):
            message = f"unreadable runner input {supplied!r}"
            raise RunnerShapeError(message)
        declarations.append(supplied)
    return declarations


def declaration_labels(declaration: object) -> set[str]:
    """Return every label one runner declaration can select.

    Both arms of a conditional count: a label reachable only when a pull
    request comes from a fork is as much in use as one reachable otherwise.
    An expression with no quoted literal contributes no label of its own
    only when it is ``${{ inputs.<name> }}``, a reusable workflow selecting
    whatever its caller passed; the caller's declaration names the labels.
    Any other literal-free expression is refused.

    Examples
    --------
    >>> declaration_labels("ubuntu-latest")
    {'ubuntu-latest'}
    >>> sorted(declaration_labels("${{ x && 'ubuntu-latest' || 'other' }}"))
    ['other', 'ubuntu-latest']
    >>> declaration_labels("${{ inputs.runner }}")
    set()
    """
    text = str(declaration)
    if "${{" not in text:
        return {text.strip()}
    literals = set(EXPRESSION_LITERAL.findall(text))
    if not literals and not CALLER_INPUT_EXPRESSION.match(text.strip()):
        message = f"expression {text!r} selects labels this reader cannot see"
        raise RunnerShapeError(message)
    return literals


def job_labels(definition: dict[str, object]) -> set[str]:
    """Return every label one job can select, across all its declarations.

    Examples
    --------
    >>> sorted(job_labels({"runs-on": ["self-hosted", "linux"]}))
    ['linux', 'self-hosted']
    """
    return {
        label
        for declaration in runner_declarations(definition)
        for label in declaration_labels(declaration)
    }


def labels_in_use() -> set[str]:
    """Return every label any lane in the repository can select.

    Examples
    --------
    >>> "ubuntu-latest" in labels_in_use()
    True
    """
    return {label for definition in jobs().values() for label in job_labels(definition)}


def registered_labels() -> set[str]:
    """Return the labels ``.github/actionlint.yaml`` registers.

    Examples
    --------
    >>> "ubicloud-standard-4" in registered_labels()
    True
    """
    return set(_registered_labels())


@functools.cache
def _registered_labels() -> frozenset[str]:
    """Read the actionlint registry once.

    Returns
    -------
    frozenset[str]
        Every label registered under ``self-hosted-runner``, empty when the
        configuration registers none.
    """
    assert ACTIONLINT_CONFIG.exists(), (
        "this repository uses a runner label actionlint does not know, so "
        f"{ACTIONLINT_CONFIG.relative_to(REPO_ROOT)} must exist"
    )
    config = load_workflow(ACTIONLINT_CONFIG.read_text(encoding="utf-8"))
    return frozenset((config.get("self-hosted-runner") or {}).get("labels") or [])
