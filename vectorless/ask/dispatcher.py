"""Dispatcher — unified entry point for retrieval.

Mirrors vectorless-core/vectorless-agent/src/dispatcher.rs.

All queries go through dispatch():
1. Query understanding -> QueryPlan
2. Scope resolution -> Specified | Workspace
3. Orchestrator.run() (always)
4. Return Output
"""

from __future__ import annotations

import logging
from typing import Any, Callable

from vectorless.ask.types import DocCard, Output, Specified, Workspace
from vectorless.ask.orchestrator import Orchestrator
from vectorless.ask.understand import understand
from vectorless.llm_client import LLMClient

logger = logging.getLogger(__name__)


async def dispatch(
    query: str,
    scope: Specified | Workspace,
    llm: LLMClient,
    doc_loader: Callable[[str], Any],
    event_callback: Any = None,
) -> Output:
    """Unified entry point — mirrors Rust dispatcher::dispatch().

    All queries go through Orchestrator:
    - Specified -> skip_analysis=True -> spawn Workers directly
    - Workspace -> skip_analysis=False -> analyze -> dispatch -> evaluate -> replan
    """
    # Step 1: Query understanding
    logger.info("dispatch: query understanding started")
    query_plan = await understand(query, llm)
    logger.info(
        "dispatch: query understanding complete (intent=%s, complexity=%s)",
        query_plan.intent.value,
        query_plan.complexity.value,
    )

    # Step 2: Determine skip_analysis from scope
    skip_analysis = isinstance(scope, Specified)
    doc_cards = scope.docs

    # Step 3: Orchestrator (always)
    orchestrator = Orchestrator(
        query=query,
        doc_cards=doc_cards,
        doc_loader=doc_loader,
        llm_client=llm,
        skip_analysis=skip_analysis,
        query_plan=query_plan,
        event_callback=event_callback,
    )
    return await orchestrator.run()
