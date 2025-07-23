from __future__ import annotations

import typing as typ

from msgspec import Struct, json

from weaver_schemas.error import SchemaError

Handler = typ.Callable[..., typ.Awaitable[typ.Any]]


class RPCRequest(Struct):
    """JSON-RPC style request."""

    method: str
    params: dict[str, typ.Any] | None = None


class RPCDispatcher:
    """Simple RPC dispatcher mapping method names to handlers."""

    def __init__(self) -> None:
        self._handlers: dict[str, Handler] = {}

    def register(self, name: str) -> typ.Callable[[Handler], Handler]:
        def decorator(func: Handler) -> Handler:
            self._handlers[name] = func
            return func

        return decorator

    async def handle(self, data: bytes) -> bytes:
        try:
            request = json.decode(data, type=RPCRequest)
        except Exception as exc:  # noqa: BLE001 - fallback for malformed input
            return json.encode(SchemaError(message=f"invalid request: {exc}"))

        handler = self._handlers.get(request.method)
        if handler is None:
            return json.encode(SchemaError(message=f"unknown method: {request.method}"))

        try:
            result = await handler(**(request.params or {}))
        except Exception as exc:  # noqa: BLE001 - ensure structured errors
            return json.encode(SchemaError(message=str(exc)))

        return json.encode(result)
