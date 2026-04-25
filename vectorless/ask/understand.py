"""Query understanding — LLM-driven analysis of user queries.

This module is now a thin wrapper around the multi-stage QueryAnalyzer.
Kept for backward compatibility — new code should use QueryAnalyzer directly.
"""

from __future__ import annotations

import logging
import warnings

from vectorless.llm_client import LLMClient
from vectorless.ask.plan import QueryPlan
from vectorless.ask.reasoning.analyzer import QueryAnalyzer

logger = logging.getLogger(__name__)


async def understand(
    query: str,
    llm: LLMClient,
) -> QueryPlan:
    """Analyze a user query using LLM to produce a structured QueryPlan.

    Delegates to QueryAnalyzer.analyze() and converts the result to QueryPlan
    for backward compatibility.

    Raises on LLM failure — no silent degradation.
    """
    warnings.warn(
        "understand() is deprecated — use QueryAnalyzer.analyze() directly",
        DeprecationWarning,
        stacklevel=2,
    )

    analyzer = QueryAnalyzer()
    analysis = await analyzer.analyze(query, llm)

    # Convert QueryAnalysis back to QueryPlan
    return QueryPlan(
        original=analysis.original,
        intent=_map_intent(analysis.intent),
        keywords=analysis.keywords,
        key_concepts=analysis.key_concepts,
        strategy_hint=analysis.strategy.strategy_type,
        complexity=_map_complexity(analysis.complexity),
        rewritten=analysis.rewritten,
        sub_queries=_map_sub_queries(analysis.sub_queries),
    )


def _map_intent(intent):
    """Map reasoning QueryIntent to plan QueryIntent."""
    from vectorless.ask.plan import QueryIntent as PlanIntent
    mapping = {
        "factual": PlanIntent.FACTUAL,
        "analytical": PlanIntent.ANALYTICAL,
        "navigational": PlanIntent.NAVIGATIONAL,
        "summary": PlanIntent.SUMMARY,
        "comparative": PlanIntent.ANALYTICAL,   # Map new to existing
        "procedural": PlanIntent.FACTUAL,        # Map new to existing
    }
    return mapping.get(intent.value, PlanIntent.FACTUAL)


def _map_complexity(complexity):
    """Map reasoning Complexity to plan Complexity."""
    from vectorless.ask.plan import Complexity as PlanComplexity
    mapping = {
        "simple": PlanComplexity.SIMPLE,
        "moderate": PlanComplexity.MODERATE,
        "complex": PlanComplexity.COMPLEX,
    }
    return mapping.get(complexity.value, PlanComplexity.SIMPLE)


def _map_sub_queries(sub_queries):
    """Map reasoning SubQueries to plan SubQueries."""
    from vectorless.ask.plan import SubQuery as PlanSubQuery
    return [
        PlanSubQuery(query=sq.query, target_docs=sq.target_docs)
        for sq in sub_queries
    ]
