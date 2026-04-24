"""Verification types — dimensions, scores, and results."""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum


class VerificationDimension(str, Enum):
    """Dimensions along which answers are verified."""
    FACTUAL_ACCURACY = "factual_accuracy"   # Does evidence support the claims?
    COMPLETENESS = "completeness"            # Does it cover all query aspects?
    RELEVANCE = "relevance"                  # Is it on-topic?
    COHERENCE = "coherence"                  # Is the reasoning trace logical?


@dataclass
class DimensionScore:
    """Score for a single verification dimension."""
    dimension: VerificationDimension
    score: float              # 0.0 - 1.0
    reasoning: str            # LLM explanation
    evidence_refs: list[str] = field(default_factory=list)  # "doc_name/node_title" or "node_title"


@dataclass
class VerificationResult:
    """Result of the verification pipeline."""
    passed: bool
    overall_confidence: float
    dimension_scores: list[DimensionScore] = field(default_factory=list)
    gaps: list[str] = field(default_factory=list)
    re_retrieval_hints: list[str] = field(default_factory=list)
    iteration: int = 0

    @property
    def needs_re_retrieval(self) -> bool:
        """Whether re-retrieval should be triggered."""
        return not self.passed and bool(self.re_retrieval_hints)
