#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///
"""Generate ``typos.toml`` from the shared en-GB-oxendict dictionary.

The shared dictionary is refreshed into an untracked repository-local cache
only when the authoritative copy is newer. A valid cache remains usable when
the network is unavailable, and ``typos.local.toml`` supplies the narrow
repository-specific policy that must not weaken the estate-wide base.
"""

from pathlib import Path

import typos_rollout as rollout

DEFAULT_BASE_URL = (
    "https://raw.githubusercontent.com/leynos/agent-helper-scripts/"
    "refs/heads/main/data/typos-oxendict-base.toml"
)
REPOSITORY_ROOT = Path(__file__).resolve().parent.parent


def dictionary_from_cache(repository: Path = REPOSITORY_ROOT) -> rollout.Dictionary:
    """Load the cached shared base merged with local repository policy.

    Parameters
    ----------
    repository
        Repository containing the untracked base and optional local overlay.

    Returns
    -------
    rollout.Dictionary
        Merged estate and repository policy.

    Raises
    ------
    OSError, TypeError, ValueError, tomllib.TOMLDecodeError
        If either dictionary cannot be read or validated.

    Examples
    --------
    >>> dictionary_from_cache(REPOSITORY_ROOT).stems is not None
    True
    """
    dictionary = rollout.load_dictionary(repository / ".typos-oxendict-base.toml")
    local_overlay = repository / "typos.local.toml"
    if local_overlay.exists():
        dictionary = rollout.merge_dictionaries(
            dictionary,
            rollout.load_dictionary(local_overlay),
        )
    return dictionary


def render_config(repository: Path = REPOSITORY_ROOT) -> str:
    """Render deterministic configuration from the populated local cache.

    Parameters
    ----------
    repository
        Repository containing the base cache and local overlay.

    Returns
    -------
    str
        Deterministic parse-checked ``typos.toml`` content.

    Raises
    ------
    OSError, TypeError, ValueError, tomllib.TOMLDecodeError
        If policy loading, merging, or rendering fails.

    Examples
    --------
    >>> '[default]' in render_config(REPOSITORY_ROOT)
    True
    """
    return rollout.render_typos_config(dictionary_from_cache(repository))


def main(
    output: Path | None = None,
    *,
    repository: Path = REPOSITORY_ROOT,
    source: str | Path = DEFAULT_BASE_URL,
    offline: bool = False,
) -> rollout.RefreshResult:
    """Refresh the shared base cache and write the merged configuration.

    Parameters
    ----------
    output
        Optional generated configuration destination.
    repository
        Repository containing cache, metadata, and local policy paths.
    source
        Local or HTTPS shared dictionary authority.
    offline
        Reuse a valid cache without contacting the authority.

    Returns
    -------
    rollout.RefreshResult
        Refresh outcome for an updated, fresh, validated stale, or offline
        cache.

    Raises
    ------
    rollout.NetworkUnavailableError
        If the HTTPS authority is unavailable and no valid untracked cache
        exists.
    rollout.InsecureSourceError
        If an authority or redirect does not use HTTPS.
    OSError, TypeError, ValueError, tomllib.TOMLDecodeError
        If local persistence, policy validation, or rendering fails.

    Examples
    --------
    >>> main(repository=REPOSITORY_ROOT, offline=True).status
    'offline-cache'
    """
    destination = output if output is not None else repository / "typos.toml"
    result = rollout.refresh_base(
        source,
        repository / ".typos-oxendict-base.toml",
        rollout.RefreshOptions(
            metadata=repository / ".typos-oxendict-base.json",
            offline=offline,
        ),
    )
    rollout.write_config(destination, dictionary_from_cache(repository))
    return result


if __name__ == "__main__":
    refresh = main()
    print(f"{refresh.status}: {REPOSITORY_ROOT / 'typos.toml'}")
