import asyncio
import collections.abc as cabc
import multiprocessing as mp
import os
import time
import typing as t
from pathlib import Path

import pytest

from weaver import client
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server


@pytest.fixture()
def runtime_dir(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> cabc.Generator[dict[str, t.Any], None, None]:
    os.environ["XDG_RUNTIME_DIR"] = str(tmp_path)
    sock = client.discover_socket()
    processes: list[mp.Process] = []
    handler = {"func": lambda dispatcher: None}

    async def _serve_daemon(path: Path) -> None:
        """Run a minimal daemon for tests."""
        dispatcher = RPCDispatcher()
        handler["func"](dispatcher)
        server = await start_server(path, dispatcher)
        async with server:
            await server.serve_forever()

    def spawn_daemon(_: Path) -> mp.Process:
        def _run() -> None:
            try:
                loop = asyncio.new_event_loop()
                asyncio.set_event_loop(loop)
                loop.run_until_complete(_serve_daemon(sock))
            except Exception:  # noqa: BLE001,S110 - best effort for tests
                pass
            finally:
                loop.close()

        proc = mp.Process(target=_run)
        proc.start()
        processes.append(proc)
        time.sleep(0.1)
        return proc

    monkeypatch.setattr(client, "spawn_daemon", spawn_daemon)

    yield {
        "sock": sock,
        "processes": processes,
        "set_handlers": lambda f: handler.update(func=f),
    }

    for proc in processes:
        if proc and proc.is_alive():
            proc.terminate()
            try:
                proc.join(timeout=5)
            except Exception:  # noqa: BLE001,S110 - cleanup best effort
                pass
            finally:
                if proc.is_alive():
                    try:
                        proc.kill()
                        proc.join(timeout=1)
                    except Exception:  # noqa: BLE001,S110 - cleanup best effort
                        pass
