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

import yaml

REPO_ROOT: typ.Final = Path(__file__).resolve().parents[2]
WORKFLOW_DIR: typ.Final = REPO_ROOT / ".github" / "workflows"
ACTIONLINT_CONFIG: typ.Final = REPO_ROOT / ".github" / "actionlint.yaml"

#: Both spellings GitHub accepts for a workflow file's extension.
WORKFLOW_FILE_PATTERNS: typ.Final = ("*.yml", "*.yaml")

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
    paths = sorted(
        path
        for pattern in WORKFLOW_FILE_PATTERNS
        for path in WORKFLOW_DIR.glob(pattern)
    )
    documents = tuple(
        (path.name, yaml.safe_load(path.read_text(encoding="utf-8")))
        for path in paths
    )
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


def runner_declarations(definition: dict[str, object]) -> list[object]:
    """Return every runner declaration a job makes, from either mechanism.

    A ``runs-on`` may itself be a list of labels, which is why this flattens
    rather than returning a single value.

    Examples
    --------
    >>> runner_declarations({"runs-on": ["self-hosted", "linux"]})
    ['self-hosted', 'linux']
    >>> runner_declarations({"uses": "./b.yml", "with": {"runner": "macos-15"}})
    ['macos-15']
    >>> runner_declarations({})
    []
    """
    declarations: list[object] = []
    declared = runner_value(definition)
    if declared is not None:
        declarations.extend(declared if isinstance(declared, list) else [declared])
    supplied = caller_runner_input(definition)
    if supplied is not None:
        declarations.append(supplied)
    return declarations


def declaration_labels(declaration: object) -> set[str]:
    """Return every label one runner declaration can select.

    Both arms of a conditional count: a label reachable only when a pull
    request comes from a fork is as much in use as one reachable otherwise.
    An expression with no quoted literal selects whatever it was handed and
    contributes no label of its own.

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
    if "${{" in text:
        return set(EXPRESSION_LITERAL.findall(text))
    return {text.strip()}


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
    config = yaml.safe_load(ACTIONLINT_CONFIG.read_text(encoding="utf-8")) or {}
    return frozenset((config.get("self-hosted-runner") or {}).get("labels") or [])
