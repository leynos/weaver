from __future__ import annotations

import typing as t

from msgspec import Struct, json


class RPCRequest(Struct):
    """JSON-RPC style request."""

    method: str
    params: dict[str, t.Any] | None = None


class RPCDispatcher:
    """Simple RPC dispatcher mapping method names to handlers."""

    def __init__(self) -> None:
        self._handlers: dict[str, t.Callable[..., t.Awaitable[t.Any]]] = {}

    def register(
        self, name: str
    ) -> t.Callable[
        [t.Callable[..., t.Awaitable[t.Any]]], t.Callable[..., t.Awaitable[t.Any]]
    ]:
        def decorator(
            func: t.Callable[..., t.Awaitable[t.Any]],
        ) -> t.Callable[..., t.Awaitable[t.Any]]:
            self._handlers[name] = func
            return func

        return decorator

    async def handle(self, data: bytes) -> bytes:
        request = json.decode(data, type=RPCRequest)
        handler = self._handlers.get(request.method)
        if handler is None:
            raise ValueError(f"unknown method: {request.method}")
        if request.params:
            result = await handler(**request.params)
        else:
            result = await handler()
        return json.encode(result)
