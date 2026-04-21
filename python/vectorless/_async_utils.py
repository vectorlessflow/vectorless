"""Shared async utilities for sync/async context bridging."""

from __future__ import annotations

import asyncio
import concurrent.futures
from typing import Coroutine, TypeVar

T = TypeVar("T")


def run_async(coro: Coroutine[object, object, T]) -> T:
    """Run an async coroutine synchronously.

    Handles both pure-script and Jupyter (existing event loop) contexts.

    - No running event loop: uses ``asyncio.run()``.
    - Running event loop (Jupyter): runs the coroutine in a new thread
      with its own event loop, then waits for the result.
    """
    try:
        asyncio.get_running_loop()
    except RuntimeError:
        return asyncio.run(coro)

    # Running loop exists — offload to a new thread.
    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
        future = pool.submit(asyncio.run, coro)
        return future.result()
