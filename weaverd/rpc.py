from __future__ import annotations

import typing as typ

import msgspec as ms
import msgspec.json as msjson

from weaver_schemas.error import SchemaError

Handler = typ.Callable[..., typ.Awaitable[typ.Any]]


class RPCRequest(ms.Struct, frozen=True):
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
            request = msjson.decode(data, type=RPCRequest)
        except ms.DecodeError as exc:
            return msjson.encode(SchemaError(message=f"invalid request: {exc}"))

        handler = self._handlers.get(request.method)
        if handler is None:
            return msjson.encode(
                SchemaError(message=f"unknown method: {request.method}")
            )

        try:
            result = await handler(**(request.params or {}))
        except Exception as exc:  # noqa: BLE001 - ensure structured errors
            return msjson.encode(SchemaError(message=str(exc)))

        return msjson.encode(result)
