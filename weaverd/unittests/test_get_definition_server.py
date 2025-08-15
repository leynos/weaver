import builtins

import pytest

from weaver_schemas.primitives import Location, Position, Range
from weaver_schemas.references import Symbol
from weaverd import server


class Tool:
    def get_definition(self, file: str, line: int, char: int) -> list[Symbol]:
        loc = Location(
            file=file,
            range=Range(start=Position(line, char), end=Position(line, char + 1)),
        )
        return [Symbol(name="spam", kind="function", location=loc)]


@pytest.mark.anyio
async def test_handle_get_definition(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(server, "create_serena_tool", lambda _: Tool())
    results = server.handle_get_definition("foo.py", line=1, char=0)
    sym = await builtins.anext(results)
    with pytest.raises(StopAsyncIteration):
        await builtins.anext(results)
    assert sym.name == "spam"


@pytest.fixture()
def anyio_backend() -> str:
    return "asyncio"
