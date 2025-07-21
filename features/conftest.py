import asyncio
import multiprocessing as mp
import os
from pathlib import Path

import pytest

from weaver import client
from weaverd.rpc import RPCDispatcher
from weaverd.server import start_server


@pytest.fixture()
def runtime_dir(tmp_path: Path, monkeypatch):
    os.environ["XDG_RUNTIME_DIR"] = str(tmp_path)
    sock = client.discover_socket()
    processes: list[mp.Process] = []
    handler = {"func": lambda dispatcher: None}

    def spawn_daemon(_: Path) -> mp.Process:
        def _run() -> None:
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)

            async def _serve() -> None:
                dispatcher = RPCDispatcher()
                handler["func"](dispatcher)
                server = await start_server(sock, dispatcher)
                async with server:
                    await server.serve_forever()

            try:
                loop.run_until_complete(_serve())
            finally:
                loop.close()

        proc = mp.Process(target=_run)
        proc.start()
        processes.append(proc)
        return proc

    monkeypatch.setattr(client, "spawn_daemon", spawn_daemon)

    yield {
        "sock": sock,
        "processes": processes,
        "set_handlers": lambda f: handler.update(func=f),
    }

    for proc in processes:
        if proc.is_alive():
            proc.terminate()
            try:
                proc.join(timeout=5)
            finally:
                if proc.is_alive():
                    proc.kill()
