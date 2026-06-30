"""Dispatcher — unified entry point for retrieval.

All queries go through dispatch():
1. Query reasoning -> QueryAnalysis (via QueryAnalyzer)
2. Scope resolution -> Specified | Workspace
3. Orchestrator.run() (always)
4. Return Output
"""

from __future__ import annotations

import logging

from vectorless.ask.protocols import DocLoader, EventCallback
from vectorless.ask.errors import AskError
from vectorless.ask.types import DocCard, Output, Specified, Workspace
from vectorless.ask.orchestrator import Orchestrator
from vectorless.ask.reasoning import QueryAnalysis, QueryAnalyzer
from vectorless.llm_client import LLMClient

logger = logging.getLogger(__name__)


async def dispatch(
    query: str,
    scope: Specified | Workspace,
    llm: LLMClient,
    doc_loader: DocLoader,
    event_callback: EventCallback | None = None,
) -> Output:
    """Unified entry point for retrieval.

    - Specified docs -> skip analysis, dispatch Scouts directly
    - Workspace -> analyze (select docs) -> dispatch Scouts -> synthesize
    """
    # Step 1: Query reasoning (multi-stage analysis)
    logger.info("dispatch: query reasoning started")
    analyzer = QueryAnalyzer()
    try:
        query_analysis = await analyzer.analyze(query, llm)
    except AskError:
        raise
    except Exception as e:
        from vectorless.ask.errors import LLMFailureError
        raise LLMFailureError(f"Query analysis failed: {e}") from e
    logger.info(
        "dispatch: query reasoning complete (intent=%s, complexity=%s)",
        query_analysis.intent.value,
        query_analysis.complexity.value,
    )

    # Step 2: Determine skip_analysis from scope
    skip_analysis = isinstance(scope, Specified)
    doc_cards = scope.docs
    logger.info(
        "dispatch: scope=%s docs=%d skip_analysis=%s",
        "specified" if skip_analysis else "workspace",
        len(doc_cards),
        skip_analysis,
    )

    # Step 3: Orchestrator (always)
    logger.info("dispatch: starting orchestrator")
    orchestrator = Orchestrator(
        query=query,
        doc_cards=doc_cards,
        doc_loader=doc_loader,
        llm_client=llm,
        skip_analysis=skip_analysis,
        query_analysis=query_analysis,
        event_callback=event_callback,
    )
    result = await orchestrator.run()
    logger.info("dispatch: orchestrator complete answer_len=%d", len(result.answer) if result.answer else 0)
    return result
