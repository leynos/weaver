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

    @staticmethod
    def _encode_error(message: str) -> bytes:
        return msjson.encode(SchemaError(message=message))

    def _decode_request(self, data: bytes) -> tuple[RPCRequest | None, bytes | None]:
        try:
            return msjson.decode(data, type=RPCRequest), None
        except ms.DecodeError as exc:
            return None, self._encode_error(f"invalid request: {exc}")

    async def _execute_handler(
        self, handler: Handler | None, request: RPCRequest
    ) -> tuple[typ.Any | None, bytes | None]:
        if handler is None:
            return None, self._encode_error(f"unknown method: {request.method}")
        try:
            result = handler(**(request.params or {}))
            if inspect.isawaitable(result):
                result = await typ.cast("typ.Awaitable[typ.Any]", result)
        except Exception as exc:  # noqa: BLE001 - ensure structured errors
            return None, self._encode_error(str(exc))
        return result, None

    async def _process_result(self, result: typ.Any) -> typ.AsyncIterator[bytes]:
        if isinstance(result, (bytes | bytearray)):
            yield typ.cast("bytes", result)
            return
        if hasattr(result, "__aiter__"):
            try:
                async for item in typ.cast("typ.AsyncIterable[typ.Any]", result):
                    yield msjson.encode(item)
            except Exception as exc:  # noqa: BLE001 - ensure structured errors
                yield self._encode_error(str(exc))
            return
        if isinstance(result, typ.Iterable) and not isinstance(
            result, (str | bytes | bytearray)
        ):
            try:
                for item in result:
                    yield msjson.encode(item)
            except Exception as exc:  # noqa: BLE001 - ensure structured errors
                yield self._encode_error(str(exc))
            return
        yield msjson.encode(result)

    async def handle(self, data: bytes) -> typ.AsyncIterator[bytes]:
        request, err = self._decode_request(data)
        if err is not None:
            yield err
            return

        result, err = await self._execute_handler(
            self._handlers.get(request.method), request
        )
        if err is not None:
            yield err
            return

        async for chunk in self._process_result(result):
            yield chunk
