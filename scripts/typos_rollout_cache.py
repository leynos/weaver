"""Provide cache support types and atomic writes for the spelling helper."""

from __future__ import annotations

import dataclasses as dc
import pathlib
import tempfile
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


@dc.dataclass(frozen=True)
class RefreshResult:
    """Describe whether the untracked shared dictionary cache changed.

    Attributes
    ----------
    status
        Stable refresh outcome such as ``current`` or ``refreshed``.
    cache
        Validated cache or tracked configuration used for the outcome.

    Examples
    --------
    >>> RefreshResult("current", pathlib.Path("base.toml")).status
    'current'
    """

    status: str
    cache: pathlib.Path


@dc.dataclass(frozen=True)
class CacheTargets:
    """Group the untracked dictionary cache and metadata sidecar paths.

    Attributes
    ----------
    cache
        Dictionary cache destination.
    metadata
        Freshness metadata sidecar.
    """

    cache: pathlib.Path
    metadata: pathlib.Path


class RemoteResponse(typ.Protocol):
    """Expose the HTTP response surface used by cache refresh.

    Implementations provide a status code, response headers, body reader, and
    context-manager lifecycle compatible with ``urllib`` responses.

    Attributes
    ----------
    status
        HTTP response status code.
    headers
        Case-sensitive response header mapping used by the refresh helper.

    Examples
    --------
    A refresh opener returns this protocol so callers can use
    ``with response as remote: remote.read()``.
    """

    status: int
    headers: cabc.Mapping[str, str]

    def __enter__(self) -> typ.Self:
        """Enter the response context.

        Returns
        -------
        RemoteResponse
            The open response.
        """
        ...

    def __exit__(self, *_args: object) -> None:
        """Close the response context.

        Returns
        -------
        None
            The response is closed without suppressing exceptions.
        """
        ...

    def read(self) -> bytes:
        """Read the response body.

        Returns
        -------
        bytes
            Raw dictionary response bytes.

        Raises
        ------
        OSError
            If the body cannot be read.
        """
        ...


def atomic_write(path: pathlib.Path, content: bytes) -> None:
    """Write content beside a path and atomically replace the destination.

    Parameters
    ----------
    path
        Destination replaced after the temporary file closes successfully.
    content
        Bytes to persist.

    Returns
    -------
    None
        The destination is replaced atomically.

    Raises
    ------
    OSError
        If directory creation, writing, closing, or replacement fails.

    Examples
    --------
    >>> destination = pathlib.Path("generated.toml")
    >>> atomic_write(destination, b"schema = 1\n")
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    stream = tempfile.NamedTemporaryFile(
        delete=False, dir=path.parent, prefix=f".{path.name}."
    )
    temporary = pathlib.Path(stream.name)
    try:
        with stream:
            stream.write(content)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)
