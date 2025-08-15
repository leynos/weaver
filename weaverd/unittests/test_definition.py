import builtins

import pytest

from weaver_schemas.primitives import Location, Position, Range
from weaver_schemas.references import Symbol
from weaverd import server
from weaverd.serena_tools import SerenaTool


class StubTool:
    def get_definition(self, *, file: str, line: int, char: int) -> list[Symbol]:
        loc = Location(
            file=file,
            range=Range(start=Position(line, char), end=Position(line, char + 1)),
        )
        return [Symbol(name="foo", kind="function", location=loc)]


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_handle_get_definition(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: StubTool())
    results = server.handle_get_definition("foo.py", 1, 0)
    sym = await builtins.anext(results)
    with pytest.raises(StopAsyncIteration):
        await builtins.anext(results)
    assert sym.name == "foo"


class EmptyTool:
    def get_definition(self, *, file: str, line: int, char: int) -> list[Symbol]:
        return []


@pytest.mark.anyio
async def test_handle_get_definition_no_symbols(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: EmptyTool())
    results = server.handle_get_definition("foo.py", 1, 0)
    with pytest.raises(StopAsyncIteration):
        await builtins.anext(results)


@pytest.mark.anyio
async def test_handle_get_definition_missing_dependency(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def raise_error(_: SerenaTool) -> None:
        raise RuntimeError("serena-agent not found")

    monkeypatch.setattr(server, "create_serena_tool", raise_error)

    with pytest.raises(RuntimeError, match="serena-agent not found"):
        await builtins.anext(server.handle_get_definition("foo.py", 1, 0))
