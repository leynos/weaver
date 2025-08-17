"""Shared helpers for BDD step implementations."""

from features.types import Context
from weaverd import server
from weaverd.rpc import RPCDispatcher


def register_production_handlers(context: Context) -> Context:
    """Register all production RPC handlers for the test dispatcher."""

    def setup(dispatcher: RPCDispatcher) -> None:
        for name, handler in server.HANDLERS:
            dispatcher.register(name)(handler)

    context["register"](setup)
    return context
