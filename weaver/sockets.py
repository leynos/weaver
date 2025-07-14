from __future__ import annotations

import anyio


async def can_connect(path: str) -> bool:
    """Return True if a UNIX socket exists and accepts a connection."""
    try:
        stream = await anyio.connect_unix(path)
    except FileNotFoundError:
        return False
    else:
        await stream.aclose()
        return True
