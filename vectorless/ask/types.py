"""Ask pipeline types.

    Evidence          — a single piece of collected evidence
    TraceStep         — a single reasoning step
    WorkerMetrics     — per-document (Scout) execution metrics
    WorkerOutput      — per-document retrieval output: evidence only, no answer
    Metrics           — aggregated orchestrator-level metrics
    Output            — final result of a retrieval operation

    DocCard           — lightweight document metadata for analysis
    DispatchEntry     — a single dispatch target from orchestrator analysis
    OrchestratorState — mutable orchestrator state
"""

from __future__ import annotations

from dataclasses import dataclass, field


# ---------------------------------------------------------------------------
# Evidence
# ---------------------------------------------------------------------------

@dataclass
class Evidence:
    """A single piece of evidence collected during navigation.

    - source_path: navigation breadcrumb (e.g. "Root/Chapter 1/Section 1.2")
    - node_title: title of the node
    - doc_name: source document name (set by the orchestrator in multi-doc runs)
    """

    source_path: str
    node_title: str
    content: str
    doc_name: str | None = None


# ---------------------------------------------------------------------------
# TraceStep
# ---------------------------------------------------------------------------

@dataclass
class TraceStep:
    """A single step in the reasoning trace."""

    action: str
    observation: str
    round: int


# ---------------------------------------------------------------------------
# Per-document metrics
# ---------------------------------------------------------------------------

@dataclass
class WorkerMetrics:
    """Metrics for a single document's retrieval (one Scout)."""

    rounds_used: int = 0
    llm_calls: int = 0
    nodes_visited: int = 0
    budget_exhausted: bool = False
    plan_generated: bool = False
    check_count: int = 0
    evidence_chars: int = 0


# ---------------------------------------------------------------------------
# Per-document output
# ---------------------------------------------------------------------------

@dataclass
class WorkerOutput:
    """Output from a single document's retrieval — pure evidence, no answer.

    Answer synthesis happens at the orchestrator (consolidate + synthesis).
    """

    evidence: list[Evidence] = field(default_factory=list)
    metrics: WorkerMetrics = field(default_factory=WorkerMetrics)
    doc_name: str = ""
    trace_steps: list[TraceStep] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Aggregated metrics
# ---------------------------------------------------------------------------

@dataclass
class Metrics:
    """Execution metrics — aggregated across all documents."""

    rounds_used: int = 0
    llm_calls: int = 0
    nodes_visited: int = 0
    budget_exhausted: bool = False
    plan_generated: bool = False
    check_count: int = 0
    evidence_chars: int = 0


# ---------------------------------------------------------------------------
# Output — the final result
# ---------------------------------------------------------------------------

@dataclass
class Output:
    """Final result of a retrieval operation — what Engine.ask() returns."""

    answer: str
    evidence: list[Evidence] = field(default_factory=list)
    metrics: Metrics = field(default_factory=Metrics)
    confidence: float = 0.0
    trace_steps: list[TraceStep] = field(default_factory=list)

    @staticmethod
    def empty() -> Output:
        """Create an empty output (no evidence found)."""
        return Output(answer="")


# ---------------------------------------------------------------------------
# DocCard — lightweight document metadata for orchestrator analysis
# ---------------------------------------------------------------------------

@dataclass
class DocCard:
    """Summary of an ingested document, used for orchestrator analysis."""

    doc_id: str
    name: str
    summary: str
    section_count: int
    concepts: list[str] = field(default_factory=list)


# ---------------------------------------------------------------------------
# DispatchEntry — a single dispatch target
# ---------------------------------------------------------------------------

@dataclass
class DispatchEntry:
    """A single dispatch target parsed from orchestrator analysis."""

    doc_idx: int
    reason: str
    task: str


# ---------------------------------------------------------------------------
# Scope
# ---------------------------------------------------------------------------

@dataclass
class Specified:
    """User specified one or more documents — skip analysis, dispatch directly."""

    docs: list[DocCard]


@dataclass
class Workspace:
    """Workspace scope — analyze DocCards and select relevant ones."""

    docs: list[DocCard]


Scope = Specified | Workspace


# ---------------------------------------------------------------------------
# OrchestratorState
# ---------------------------------------------------------------------------

@dataclass
class OrchestratorState:
    """Mutable state for the orchestrator: tracks dispatches and collects results."""

    dispatched: list[int] = field(default_factory=list)
    sub_results: list[WorkerOutput] = field(default_factory=list)
    all_evidence: list[Evidence] = field(default_factory=list)
    analyze_done: bool = False
    total_llm_calls: int = 0

    def record_dispatch(self, doc_idx: int) -> None:
        if doc_idx not in self.dispatched:
            self.dispatched.append(doc_idx)

    def collect_result(self, doc_idx: int, result: WorkerOutput) -> None:
        """Collect a per-document result, including its LLM call count."""
        for e in result.evidence:
            if e.doc_name is None:
                e.doc_name = result.doc_name
        self.total_llm_calls += result.metrics.llm_calls
        self.all_evidence.extend(result.evidence)
        self.sub_results.append(result)
        self.record_dispatch(doc_idx)

    def into_output(self, answer: str) -> Output:
        """Merge all sub-results into a single Output."""
        trace_steps = [s for r in self.sub_results for s in r.trace_steps]
        return Output(
            answer=answer,
            evidence=list(self.all_evidence),
            metrics=Metrics(
                llm_calls=self.total_llm_calls,
                rounds_used=sum(r.metrics.rounds_used for r in self.sub_results),
                nodes_visited=sum(r.metrics.nodes_visited for r in self.sub_results),
                budget_exhausted=any(r.metrics.budget_exhausted for r in self.sub_results),
                plan_generated=any(r.metrics.plan_generated for r in self.sub_results),
                check_count=sum(r.metrics.check_count for r in self.sub_results),
                evidence_chars=sum(r.metrics.evidence_chars for r in self.sub_results),
            ),
            confidence=0.0,
            trace_steps=trace_steps,
        )
