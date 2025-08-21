import builtins

import pytest

from weaver_schemas.primitives import Location, Position, Range
from weaver_schemas.references import Reference
from weaverd import server
from weaverd.serena_tools import SerenaAgentNotFoundError, SerenaTool
from weaverd.server import ReferenceLookupError


class StubTool:
    def list_references(
        self, *, file: str, line: int, char: int, include_definition: bool = False
    ) -> list[Reference]:
        loc = Location(
            file=file,
            range=Range(start=Position(line, char), end=Position(line, char + 1)),
        )
        return [Reference(location=loc)]


@pytest.fixture
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_handle_list_references(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: StubTool())
    results = server.handle_list_references("foo.py", 1, 0)
    ref = await builtins.anext(results)
    with pytest.raises(StopAsyncIteration):
        await builtins.anext(results)
    assert ref.location.file == "foo.py"


class EmptyTool:
    def list_references(
        self, *, file: str, line: int, char: int, include_definition: bool = False
    ) -> list[Reference]:
        return []


@pytest.mark.anyio
async def test_handle_list_references_no_results(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: EmptyTool())
    results = server.handle_list_references("foo.py", 1, 0)
    with pytest.raises(StopAsyncIteration):
        await builtins.anext(results)


@pytest.mark.anyio
async def test_handle_list_references_missing_dependency(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def raise_error(_: SerenaTool) -> None:
        raise SerenaAgentNotFoundError()

    monkeypatch.setattr(server, "create_serena_tool", raise_error)

    with pytest.raises(SerenaAgentNotFoundError, match="serena-agent not found"):
        await builtins.anext(server.handle_list_references("foo.py", 1, 0))


class FailingTool:
    def list_references(
        self, *, file: str, line: int, char: int, include_definition: bool = False
    ) -> list[Reference]:
        raise RuntimeError("boom")


@pytest.mark.anyio
async def test_handle_list_references_wraps_error(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: FailingTool())
    with pytest.raises(ReferenceLookupError, match="Reference lookup failed: boom"):
        await builtins.anext(server.handle_list_references("foo.py", 1, 0))
