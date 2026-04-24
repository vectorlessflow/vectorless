"""QueryAnalyzer — multi-stage query reasoning pipeline.

Replaces the single-call understand() with a multi-stage analysis that
adapts depth based on query complexity:
- Fast mode (SIMPLE): single LLM call, basic analysis
- Deep mode (MODERATE/COMPLEX): three sequential LLM calls
"""

from __future__ import annotations

import json
import logging
import re

from vectorless.llm_client import LLMClient
from vectorless.ask.utils import parse_json_response
from vectorless.ask.reasoning.types import (
    Ambiguity,
    AmbiguityType,
    Complexity,
    EntityRef,
    QueryAnalysis,
    QueryIntent,
    RetrievalStrategy,
    SubQuery,
    TemporalConstraint,
)
from vectorless.ask.reasoning.prompts import (
    stage1_classify_prompt,
    stage2_deep_analysis_prompt,
    stage3_strategy_prompt,
    re_analyze_strategy_prompt,
)

logger = logging.getLogger(__name__)


def _extract_keywords(query: str) -> list[str]:
    """Extract keywords from query using stop word filtering."""
    stop_words = {
        "what", "is", "the", "a", "an", "how", "does", "do", "are",
        "in", "on", "at", "to", "for", "of", "with", "and", "or",
        "this", "that", "it", "from", "by", "was", "were", "be",
        "can", "could", "would", "should", "will", "has", "have",
        "had", "not", "but", "if", "then", "than", "so", "as",
        "there", "their", "they", "its", "about", "which", "when",
        "who", "whom", "all", "each", "every", "both", "few",
        "more", "most", "other", "some", "such", "no", "nor",
        "only", "own", "same", "too", "very", "just", "because",
    }
    words = re.findall(r"\b\w+\b", query.lower())
    return list(dict.fromkeys(w for w in words if w not in stop_words and len(w) > 2))


def _parse_intent(raw: str) -> QueryIntent:
    """Parse intent string to QueryIntent enum."""
    mapping = {
        "factual": QueryIntent.FACTUAL,
        "analytical": QueryIntent.ANALYTICAL,
        "navigational": QueryIntent.NAVIGATIONAL,
        "summary": QueryIntent.SUMMARY,
        "comparative": QueryIntent.COMPARATIVE,
        "procedural": QueryIntent.PROCEDURAL,
    }
    return mapping.get(raw.lower(), QueryIntent.FACTUAL)


def _parse_complexity(raw: str) -> Complexity:
    """Parse complexity string to Complexity enum."""
    mapping = {
        "simple": Complexity.SIMPLE,
        "moderate": Complexity.MODERATE,
        "complex": Complexity.COMPLEX,
    }
    return mapping.get(raw.lower(), Complexity.SIMPLE)


class QueryAnalyzer:
    """Multi-stage query reasoning pipeline.

    Usage::

        analyzer = QueryAnalyzer()
        analysis = await analyzer.analyze("What is Q1 revenue?", llm)
        # For re-analysis after verification failure:
        analysis = await analyzer.re_analyze(analysis, gaps, evidence_summary, llm)
    """

    async def analyze(self, query: str, llm: LLMClient) -> QueryAnalysis:
        """Analyze a user query using multi-stage LLM pipeline.

        Fast mode (complexity=SIMPLE after Stage 1): single LLM call.
        Deep mode (MODERATE/COMPLEX): three sequential LLM calls.

        Raises on LLM failure — no silent degradation.
        """
        keywords = _extract_keywords(query)

        # Stage 1: Classify + Decompose
        system, user = stage1_classify_prompt(query, keywords)
        response = await llm.complete(system, user)

        if not response.strip():
            raise ValueError(
                "Query analysis failed: LLM returned an empty response. "
                "Check your API key, model, and endpoint configuration."
            )

        stage1 = parse_json_response(response)
        intent = _parse_intent(stage1.get("intent", "factual"))
        complexity = _parse_complexity(stage1.get("complexity", "simple"))
        key_concepts = stage1.get("key_concepts", [])
        rewritten = _parse_rewritten(stage1.get("rewritten"))
        sub_queries = _parse_sub_queries(stage1.get("sub_queries", []))

        # Fast mode: SIMPLE queries don't need deep analysis
        if complexity == Complexity.SIMPLE:
            return QueryAnalysis(
                original=query,
                rewritten=rewritten,
                intent=intent,
                complexity=complexity,
                keywords=keywords,
                key_concepts=key_concepts,
                sub_queries=sub_queries,
                strategy=RetrievalStrategy(
                    strategy_type=_map_strategy(stage1.get("strategy_hint", "focused")),
                ),
            )

        # Deep mode: Stage 2 + Stage 3
        analysis_complete = True

        stage1_summary = {
            "intent": intent.value,
            "complexity": complexity.value,
            "key_concepts": key_concepts,
            "rewritten": rewritten,
            "sub_queries": [sq.query for sq in sub_queries],
        }

        # Stage 2: Deep Analysis
        entities: list[EntityRef] = []
        ambiguities: list[Ambiguity] = []
        temporal_constraints: list[TemporalConstraint] = []

        try:
            system2, user2 = stage2_deep_analysis_prompt(query, stage1_summary)
            response2 = await llm.complete(system2, user2)
            stage2 = parse_json_response(response2)
            entities = _parse_entities(stage2.get("entities", []))
            ambiguities = _parse_ambiguities(stage2.get("ambiguities", []))
            temporal_constraints = _parse_temporal(stage2.get("temporal_constraints", []))
            # Update key_concepts if stage 2 provides them
            if stage2.get("key_concepts"):
                key_concepts = stage2["key_concepts"]
        except Exception as e:
            logger.warning("Stage 2 (deep analysis) failed: %s — continuing with partial results", e)
            analysis_complete = False

        # Stage 3: Strategy Formation
        strategy = RetrievalStrategy(
            strategy_type=_map_strategy(stage1.get("strategy_hint", "focused")),
        )

        stage2_summary = {
            "entities": [{"name": e.name, "type": e.entity_type} for e in entities],
            "ambiguities": [{"type": a.ambiguity_type.value, "desc": a.description} for a in ambiguities],
            "key_concepts": key_concepts,
        }

        try:
            system3, user3 = stage3_strategy_prompt(query, stage1_summary, stage2_summary)
            response3 = await llm.complete(system3, user3)
            stage3 = parse_json_response(response3)
            strategy = _parse_strategy(stage3)
        except Exception as e:
            logger.warning("Stage 3 (strategy formation) failed: %s — using default strategy", e)
            analysis_complete = False

        return QueryAnalysis(
            original=query,
            rewritten=rewritten,
            intent=intent,
            complexity=complexity,
            keywords=keywords,
            key_concepts=key_concepts,
            entities=entities,
            ambiguities=ambiguities,
            temporal_constraints=temporal_constraints,
            sub_queries=sub_queries,
            strategy=strategy,
            analysis_complete=analysis_complete,
        )

    async def re_analyze(
        self,
        analysis: QueryAnalysis,
        gaps: list[str],
        evidence_summary: str,
        llm: LLMClient,
    ) -> QueryAnalysis:
        """Re-analyze for verification-driven re-retrieval.

        Runs only Stage 3 (strategy update) with gap context.
        Increments iteration. Always deep (1 LLM call).
        """
        current = {
            "intent": analysis.intent.value,
            "complexity": analysis.complexity.value,
            "strategy": analysis.strategy.strategy_type,
            "entities": [{"name": e.name, "type": e.entity_type} for e in analysis.entities],
        }

        system, user = re_analyze_strategy_prompt(
            query=analysis.original,
            current_analysis=current,
            gaps=gaps,
            evidence_summary=evidence_summary,
        )

        strategy_ok = True
        try:
            response = await llm.complete(system, user)
            stage3 = parse_json_response(response)
            new_strategy = _parse_strategy(stage3)
        except Exception as e:
            logger.warning("Re-analyze strategy update failed: %s — keeping current strategy", e)
            new_strategy = analysis.strategy
            strategy_ok = False

        return QueryAnalysis(
            original=analysis.original,
            rewritten=analysis.rewritten,
            intent=analysis.intent,
            complexity=analysis.complexity,
            keywords=analysis.keywords,
            key_concepts=analysis.key_concepts,
            entities=analysis.entities,
            ambiguities=analysis.ambiguities,
            temporal_constraints=analysis.temporal_constraints,
            sub_queries=analysis.sub_queries,
            strategy=new_strategy,
            iteration=analysis.iteration + 1,
            additional_context="; ".join(gaps),
            previous_evidence_summary=evidence_summary,
            analysis_complete=analysis.analysis_complete and strategy_ok,
        )


# ---------------------------------------------------------------------------
# Parsing helpers
# ---------------------------------------------------------------------------

def _parse_rewritten(raw: str | list | None) -> list[str]:
    """Extract rewritten queries from LLM response."""
    if raw is None:
        return []
    if isinstance(raw, list):
        return [r.strip() for r in raw if isinstance(r, str) and r.strip()]
    if isinstance(raw, str) and raw.strip():
        return [raw.strip()]
    return []


def _parse_sub_queries(raw: list) -> list[SubQuery]:
    """Parse sub_queries from LLM response."""
    if not isinstance(raw, list):
        return []
    result = []
    for item in raw:
        if isinstance(item, str) and item.strip():
            result.append(SubQuery(query=item.strip()))
        elif isinstance(item, dict):
            result.append(SubQuery(
                query=item.get("query", ""),
                intent=_parse_intent(item.get("intent", "factual")),
                target_docs=item.get("target_docs"),
            ))
    return result


def _parse_entities(raw: list) -> list[EntityRef]:
    """Parse entities from Stage 2 response."""
    if not isinstance(raw, list):
        return []
    result = []
    for item in raw:
        if not isinstance(item, dict):
            continue
        result.append(EntityRef(
            name=item.get("name", ""),
            entity_type=item.get("type", "concept"),
            aliases=item.get("aliases", []),
            definition_hint=item.get("definition_hint", ""),
        ))
    return result


def _parse_ambiguities(raw: list) -> list[Ambiguity]:
    """Parse ambiguities from Stage 2 response."""
    if not isinstance(raw, list):
        return []
    result = []
    for item in raw:
        if not isinstance(item, dict):
            continue
        amb_type_str = item.get("type", "lexical")
        try:
            amb_type = AmbiguityType(amb_type_str)
        except ValueError:
            amb_type = AmbiguityType.LEXICAL
        result.append(Ambiguity(
            ambiguity_type=amb_type,
            description=item.get("description", ""),
            possible_interpretations=item.get("interpretations", []),
            resolution_query=item.get("resolution_query", ""),
        ))
    return result


def _parse_temporal(raw: list) -> list[TemporalConstraint]:
    """Parse temporal constraints from Stage 2 response."""
    if not isinstance(raw, list):
        return []
    result = []
    for item in raw:
        if not isinstance(item, dict):
            continue
        result.append(TemporalConstraint(
            raw=item.get("raw", ""),
            resolved=item.get("resolved"),
            is_relative=bool(item.get("is_relative", False)),
        ))
    return result


def _parse_strategy(raw: dict) -> RetrievalStrategy:
    """Parse strategy from Stage 3 response."""
    if not isinstance(raw, dict):
        return RetrievalStrategy()
    return RetrievalStrategy(
        strategy_type=raw.get("strategy_type", "focused"),
        sub_strategies=raw.get("sub_strategies", []),
        target_sections=raw.get("target_sections", []),
        requires_cross_doc=bool(raw.get("requires_cross_doc", False)),
        estimated_depth=raw.get("estimated_depth", "medium"),
    )


def _map_strategy(hint: str) -> str:
    """Map strategy_hint from Stage 1 to strategy_type."""
    mapping = {
        "focused": "focused",
        "exploratory": "exploratory",
        "comparative": "comparative",
        "summary": "summary",
    }
    return mapping.get(hint.lower(), "focused")
