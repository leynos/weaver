import builtins

import pytest

from weaver_schemas.primitives import Location, Position, Range
from weaver_schemas.references import Symbol
from weaverd import server
from weaverd.serena_tools import SerenaTool

try:
    _anext = builtins.anext  # type: ignore[attr-defined]
except AttributeError:  # Python < 3.10

    async def _anext(it):
        return await it.__anext__()


globals()["anext"] = _anext


class StubTool:
    def get_definition(self, *, file: str, line: int, char: int) -> list[Symbol]:
        loc = Location(
            file=file,
            range=Range(start=Position(line, char), end=Position(line, char + 1)),
        )
        return [Symbol(name="foo", kind="function", location=loc)]


class EmptyTool:
    def get_definition(self, *, file: str, line: int, char: int) -> list[Symbol]:
        return []


class MultiTool:
    def get_definition(self, *, file: str, line: int, char: int) -> list[Symbol]:
        loc1 = Location(
            file=file,
            range=Range(start=Position(line, char), end=Position(line, char + 1)),
        )
        loc2 = Location(
            file=file,
            range=Range(start=Position(line, char + 2), end=Position(line, char + 3)),
        )
        return [
            Symbol(name="foo", kind="function", location=loc1),
            Symbol(name="bar", kind="class", location=loc2),
        ]


class DummyTool:
    def get_definition(self, *, file: str, line: int, char: int) -> list[Symbol]:
        return []


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_handle_get_definition(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: StubTool())
    results = server.handle_get_definition("foo.py", 1, 0)
    sym = await anext(results)
    with pytest.raises(StopAsyncIteration):
        await anext(results)
    assert sym.name == "foo"
    assert sym.kind == "function"
    assert sym.location.file == "foo.py"
    assert sym.location.range.start.line == 1
    assert sym.location.range.start.character == 0
    assert sym.location.range.end.line == 1
    assert sym.location.range.end.character == 1


@pytest.mark.anyio
async def test_handle_get_definition_no_symbols(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: EmptyTool())
    results = server.handle_get_definition("foo.py", 1, 0)
    with pytest.raises(StopAsyncIteration):
        await anext(results)


@pytest.mark.anyio
async def test_handle_get_definition_multiple_symbols(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: MultiTool())
    results = server.handle_get_definition("foo.py", 1, 0)
    first = await anext(results)
    second = await anext(results)
    with pytest.raises(StopAsyncIteration):
        await anext(results)
    assert [first.name, second.name] == ["foo", "bar"]
    assert [first.kind, second.kind] == ["function", "class"]
    assert first.location.file == "foo.py"
    assert first.location.range.start.character == 0
    assert second.location.range.start.character == 2


@pytest.mark.anyio
async def test_handle_get_definition_missing_dependency(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def raise_error(_: SerenaTool) -> None:
        raise RuntimeError("serena-agent not found")

    monkeypatch.setattr(server, "create_serena_tool", raise_error)

    with pytest.raises(RuntimeError, match="serena-agent not found"):
        await anext(server.handle_get_definition("foo.py", 1, 0))


@pytest.mark.anyio
@pytest.mark.parametrize("line,char", [(-1, 0), (1, -5), (-2, -3)])
async def test_handle_get_definition_invalid_position(
    monkeypatch: pytest.MonkeyPatch, line: int, char: int
) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: DummyTool())
    with pytest.raises(ValueError):
        await anext(server.handle_get_definition("foo.py", line, char))
