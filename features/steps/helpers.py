"""Shared helpers for BDD step implementations."""

from __future__ import annotations

from typing import TYPE_CHECKING  # noqa: ICN003

if TYPE_CHECKING:
    from features.types import Context
    from weaverd.rpc import RPCDispatcher

__all__: tuple[str, ...] = ("register_production_handlers",)


def register_production_handlers(context: Context) -> Context:
    """Register production RPC handlers on the test dispatcher.

    Parameters
    ----------
    context : Context
        Test context that provides a "register" hook accepting a callable
        of shape (dispatcher: RPCDispatcher) -> None.

    Returns
    -------
    Context
        The same context, to enable fluent usage in step setup.
    """

    def setup(dispatcher: RPCDispatcher) -> None:
        # Import lazily to avoid import-time side effects in tests.
        from weaverd import server

        for name, handler in server.HANDLERS:
            dispatcher.register(name)(handler)

    context["register"](setup)
    return context
