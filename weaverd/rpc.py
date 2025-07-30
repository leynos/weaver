from __future__ import annotations

import inspect
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

    async def handle(self, data: bytes) -> typ.AsyncIterator[bytes]:
        try:
            request = msjson.decode(data, type=RPCRequest)
        except ms.DecodeError as exc:
            yield msjson.encode(SchemaError(message=f"invalid request: {exc}"))
            return

        handler = self._handlers.get(request.method)
        if handler is None:
            yield msjson.encode(
                SchemaError(message=f"unknown method: {request.method}")
            )
            return

        try:
            result = handler(**(request.params or {}))
            if inspect.isawaitable(result):
                result = await typ.cast("typ.Awaitable[typ.Any]", result)
        except Exception as exc:  # noqa: BLE001 - ensure structured errors
            yield msjson.encode(SchemaError(message=str(exc)))
            return

        if isinstance(result, (bytes | bytearray)):
            yield result
            return
        if hasattr(result, "__aiter__"):
            try:
                async for item in typ.cast("typ.AsyncIterable[typ.Any]", result):
                    yield msjson.encode(item)
            except Exception as exc:  # noqa: BLE001 - ensure structured errors
                yield msjson.encode(SchemaError(message=str(exc)))
        elif isinstance(result, typ.Iterable) and not isinstance(
            result, (str | bytes | bytearray)
        ):
            try:
                for item in result:
                    yield msjson.encode(item)
            except Exception as exc:  # noqa: BLE001 - ensure structured errors
                yield msjson.encode(SchemaError(message=str(exc)))
        else:
            yield msjson.encode(result)
