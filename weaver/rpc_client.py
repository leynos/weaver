from __future__ import annotations

import asyncio
import sys
import typing as t

if t.TYPE_CHECKING:
    from pathlib import Path
from msgspec import json

from weaverd.rpc import RPCRequest


async def call_rpc(
    socket_path: Path,
    method: str,
    params: dict[str, t.Any] | None = None,
    out: t.BinaryIO | None = None,
) -> None:
    """Send an RPC request and stream the raw JSONL response to ``out``."""
    output: t.BinaryIO = out or sys.stdout.buffer
    reader, writer = await asyncio.open_unix_connection(str(socket_path))
    writer.write(json.encode(RPCRequest(method=method, params=params)) + b"\n")
    await writer.drain()
    try:
        while line := await reader.readline():
            output.write(line)
            output.flush()
    finally:
        writer.close()
        await writer.wait_closed()
