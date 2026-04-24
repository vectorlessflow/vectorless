"""VerifyPipeline — multi-dimensional evidence verification.

Single combined LLM call assessing all 4 dimensions simultaneously.
Configurable thresholds per intent. Max 2 verification iterations.
"""

from __future__ import annotations

import json
import logging
import re

from vectorless.llm_client import LLMClient
from vectorless.ask.types import Evidence
from vectorless.ask.verify.types import (
    DimensionScore,
    VerificationDimension,
    VerificationResult,
)
from vectorless.ask.verify.prompts import verify_prompt

logger = logging.getLogger(__name__)

MAX_VERIFICATION_ITERATIONS = 2

# Per-intent, per-dimension thresholds
_DEFAULT_THRESHOLD = 0.5
_INTENT_THRESHOLDS: dict[str, dict[str, float]] = {
    "factual": {
        "factual_accuracy": 0.7,
        "completeness": 0.5,
        "relevance": 0.6,
        "coherence": 0.5,
    },
    "analytical": {
        "factual_accuracy": 0.6,
        "completeness": 0.7,
        "relevance": 0.6,
        "coherence": 0.7,
    },
    "comparative": {
        "factual_accuracy": 0.6,
        "completeness": 0.7,
        "relevance": 0.6,
        "coherence": 0.7,
    },
    "summary": {
        "factual_accuracy": 0.5,
        "completeness": 0.7,
        "relevance": 0.5,
        "coherence": 0.6,
    },
    "procedural": {
        "factual_accuracy": 0.7,
        "completeness": 0.7,
        "relevance": 0.6,
        "coherence": 0.7,
    },
    "navigational": {
        "factual_accuracy": 0.5,
        "completeness": 0.5,
        "relevance": 0.7,
        "coherence": 0.5,
    },
}


def _format_evidence(evidence: list[Evidence]) -> str:
    """Format evidence for the verification prompt."""
    if not evidence:
        return "(no evidence)"
    return "\n\n".join(
        f"[{e.node_title}] (from {e.doc_name or 'unknown'})\n{e.content}"
        for e in evidence
    )


def _parse_json_response(response: str) -> dict:
    """Parse LLM response as JSON, handling markdown-wrapped output."""
    trimmed = response.strip()

    if trimmed.startswith("```"):
        match = re.search(r"```(?:json)?\s*\n?(.*?)```", trimmed, re.DOTALL)
        if match:
            trimmed = match.group(1).strip()

    start = trimmed.find("{")
    if start != -1:
        depth = 0
        for i in range(start, len(trimmed)):
            if trimmed[i] == "{":
                depth += 1
            elif trimmed[i] == "}":
                depth -= 1
                if depth == 0:
                    candidate = trimmed[start : i + 1]
                    try:
                        return json.loads(candidate)
                    except json.JSONDecodeError:
                        break

    return json.loads(trimmed)


class VerifyPipeline:
    """Multi-dimensional evidence verification pipeline.

    Usage::

        pipeline = VerifyPipeline()
        result = await pipeline.verify(
            query="What is Q1 revenue?",
            evidence=collected_evidence,
            query_intent="factual",
            iteration=0,
            llm=llm_client,
        )
    """

    async def verify(
        self,
        query: str,
        evidence: list[Evidence],
        query_intent: str,
        iteration: int,
        llm: LLMClient,
    ) -> VerificationResult:
        """Run verification on collected evidence.

        Single combined LLM call assessing all 4 dimensions.
        Returns VerificationResult with pass/fail, scores, and gaps.
        """
        evidence_text = _format_evidence(evidence)

        if not evidence:
            return VerificationResult(
                passed=False,
                overall_confidence=0.0,
                gaps=["No evidence collected"],
                re_retrieval_hints=[query],
                iteration=iteration,
            )

        system, user = verify_prompt(
            query=query,
            evidence_text=evidence_text,
            query_intent=query_intent,
            iteration=iteration,
        )

        response = await llm.complete(system, user)

        if not response.strip():
            return VerificationResult(
                passed=False,
                overall_confidence=0.0,
                gaps=["Verification LLM returned empty response"],
                re_retrieval_hints=[],
                iteration=iteration,
            )

        try:
            data = _parse_json_response(response)
        except (json.JSONDecodeError, ValueError) as e:
            logger.warning("Verification response parse failed: %s", e)
            return VerificationResult(
                passed=False,
                overall_confidence=0.0,
                gaps=["Failed to parse verification response"],
                re_retrieval_hints=[],
                iteration=iteration,
            )

        return self._build_result(data, query_intent, iteration)

    def _build_result(
        self,
        data: dict,
        query_intent: str,
        iteration: int,
    ) -> VerificationResult:
        """Build VerificationResult from parsed LLM response."""
        dimensions_data = data.get("dimensions", {})
        thresholds = _INTENT_THRESHOLDS.get(query_intent, {})

        scores: list[DimensionScore] = []
        low_dimensions: list[str] = []
        gaps: list[str] = []
        re_retrieval_hints: list[str] = []

        for dim in VerificationDimension:
            dim_data = dimensions_data.get(dim.value, {})
            raw_score = float(dim_data.get("score", 0.0))
            score = max(0.0, min(1.0, raw_score))
            reasoning = dim_data.get("reasoning", "")
            refs = dim_data.get("evidence_refs", [])

            scores.append(DimensionScore(
                dimension=dim,
                score=score,
                reasoning=reasoning,
                evidence_refs=refs if isinstance(refs, list) else [],
            ))

            threshold = thresholds.get(dim.value, _DEFAULT_THRESHOLD)
            if score < threshold:
                low_dimensions.append(dim.value)
                if reasoning:
                    gaps.append(f"[{dim.value}] {reasoning}")

        passed = len(low_dimensions) == 0
        overall_confidence = max(0.0, min(1.0, float(data.get("overall_confidence", 0.5))))

        # Synthesize re_retrieval_hints from gaps or explicit hints
        raw_hints = data.get("re_retrieval_hints", [])
        if isinstance(raw_hints, list):
            re_retrieval_hints = [str(h) for h in raw_hints if h]
        elif not passed:
            re_retrieval_hints = gaps

        # Override passed if LLM says passed but dimensions disagree
        if data.get("passed", False) and low_dimensions:
            passed = False

        return VerificationResult(
            passed=passed,
            overall_confidence=overall_confidence,
            dimension_scores=scores,
            gaps=gaps,
            re_retrieval_hints=re_retrieval_hints,
            iteration=iteration,
        )
