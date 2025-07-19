from __future__ import annotations

import asyncio
import contextlib
from argparse import ArgumentParser
from pathlib import Path

from .server import main


async def _entry() -> None:
    parser = ArgumentParser(description="Run the weaverd daemon")
    parser.add_argument("--socket-path", type=Path, default=None)
    args = parser.parse_args()

    with contextlib.suppress(asyncio.CancelledError, KeyboardInterrupt):
        await main(args.socket_path)


if __name__ == "__main__":
    asyncio.run(_entry())
