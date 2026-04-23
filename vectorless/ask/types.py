"""Agent types — mirrors vectorless-agent/src/config.rs and state.rs.

Type hierarchy (matching Rust exactly):

    Evidence          — single piece of collected evidence (mirrors rerank::types::Evidence)
    TraceStep         — single reasoning step
    WorkerMetrics     — per-Worker execution metrics
    WorkerOutput      — Worker output: evidence only, no answer
    Metrics           — aggregated Orchestrator-level metrics
    Output            — final result of a retrieval operation

    DocCard           — lightweight document metadata for analysis
    DispatchEntry     — single dispatch target from Orchestrator analysis
    EvalResult        — evidence sufficiency evaluation result
"""

from __future__ import annotations

from dataclasses import dataclass, field


# ---------------------------------------------------------------------------
# Evidence — mirrors vectorless-rerank/src/types::Evidence
# ---------------------------------------------------------------------------

@dataclass
class Evidence:
    """A single piece of evidence collected during navigation.

    Replaces the old WorkerEvidence. The key difference:
    - source_path: navigation breadcrumb (e.g. "Root/Chapter 1/Section 1.2")
    - node_title: title of the node (replaces old 'title')
    - doc_name: source document name (set by Orchestrator in multi-doc scenarios)
    """

    source_path: str
    node_title: str
    content: str
    doc_name: str | None = None


# ---------------------------------------------------------------------------
# TraceStep — mirrors vectorless-document::TraceStep
# ---------------------------------------------------------------------------

@dataclass
class TraceStep:
    """A single step in the reasoning trace."""

    action: str
    observation: str
    round: int


# ---------------------------------------------------------------------------
# Worker metrics — mirrors config::WorkerMetrics
# ---------------------------------------------------------------------------

@dataclass
class WorkerMetrics:
    """Metrics specific to a single Worker's execution."""

    rounds_used: int = 0
    llm_calls: int = 0
    nodes_visited: int = 0
    budget_exhausted: bool = False
    plan_generated: bool = False
    check_count: int = 0
    evidence_chars: int = 0


# ---------------------------------------------------------------------------
# Worker output — mirrors config::WorkerOutput
# ---------------------------------------------------------------------------

@dataclass
class WorkerOutput:
    """Output from a single Worker — pure evidence, no answer synthesis.

    Rerank handles all answer generation.
    """

    evidence: list[Evidence] = field(default_factory=list)
    metrics: WorkerMetrics = field(default_factory=WorkerMetrics)
    doc_name: str = ""
    trace_steps: list[TraceStep] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Aggregated metrics — mirrors config::Metrics
# ---------------------------------------------------------------------------

@dataclass
class Metrics:
    """Agent execution metrics — aggregated across all Workers."""

    rounds_used: int = 0
    llm_calls: int = 0
    nodes_visited: int = 0
    budget_exhausted: bool = False
    plan_generated: bool = False
    check_count: int = 0
    evidence_chars: int = 0


# ---------------------------------------------------------------------------
# Output — mirrors config::Output (the final result)
# ---------------------------------------------------------------------------

@dataclass
class Output:
    """Final result of a retrieval operation.

    This is what Engine.ask() returns — aligned with Rust config::Output.
    """

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
# DocCard — lightweight document metadata for Orchestrator analysis
# ---------------------------------------------------------------------------

@dataclass
class DocCard:
    """Summary of an ingested document, used for Orchestrator analysis.

    Built from DocumentInfo in Engine._ask_python().
    """

    doc_id: str
    name: str
    summary: str
    section_count: int
    concepts: list[str] = field(default_factory=list)


# ---------------------------------------------------------------------------
# DispatchEntry — mirrors prompts::DispatchEntry
# ---------------------------------------------------------------------------

@dataclass
class DispatchEntry:
    """A single dispatch target parsed from Orchestrator analysis."""

    doc_idx: int
    reason: str
    task: str


# ---------------------------------------------------------------------------
# EvalResult — evidence sufficiency evaluation
# ---------------------------------------------------------------------------

@dataclass
class EvalResult:
    """Structured result of evidence sufficiency evaluation."""

    sufficient: bool
    missing_info: str
    coverage: float = 0.0
    quality_score: float = 0.0
    missing_aspects: list[str] = field(default_factory=list)
    relevant_evidence_ids: list[str] = field(default_factory=list)

    @property
    def needs_replan(self) -> bool:
        """Whether the Orchestrator should replan and dispatch more Workers."""
        return not self.sufficient and bool(self.missing_aspects)


# ---------------------------------------------------------------------------
# Scope — mirrors config::Scope
# ---------------------------------------------------------------------------

@dataclass
class Specified:
    """User specified one or more documents.

    Orchestrator skips analysis, spawns Workers directly.
    """

    docs: list[DocCard]


@dataclass
class Workspace:
    """Workspace scope — user didn't specify documents.

    Orchestrator analyzes DocCards and selects relevant ones.
    """

    docs: list[DocCard]


# Union type for scope
Scope = Specified | Workspace


# ---------------------------------------------------------------------------
# WorkerState — mirrors state::WorkerState
# ---------------------------------------------------------------------------

@dataclass
class WorkerState:
    """Mutable navigation state for a Worker loop.

    Created at loop start, destroyed at loop end. Never escapes the call.
    """

    breadcrumb: list[str] = field(default_factory=lambda: ["root"])
    evidence: list[Evidence] = field(default_factory=list)
    visited: set[str] = field(default_factory=set)
    collected_nodes: set[str] = field(default_factory=set)
    remaining: int = 15
    max_rounds: int = 15
    last_feedback: str = ""
    missing_info: str = ""
    history: list[str] = field(default_factory=list)
    plan: str = ""
    check_count: int = 0
    plan_generated: bool = False
    trace_steps: list[TraceStep] = field(default_factory=list)

    def dec_round(self) -> None:
        if self.remaining > 0:
            self.remaining -= 1

    def set_feedback(self, feedback: str) -> None:
        self.last_feedback = feedback

    def add_evidence(self, ev: Evidence) -> None:
        self.evidence.append(ev)

    def has_evidence_for(self, node_id: str) -> bool:
        return node_id in self.collected_nodes

    def push_history(self, entry: str) -> None:
        if len(self.history) >= MAX_HISTORY_ENTRIES:
            self.history.pop(0)
        self.history.append(entry)

    def path_str(self) -> str:
        return "/".join(self.breadcrumb)

    def evidence_summary(self) -> str:
        if not self.evidence:
            return "(none)"
        return "\n".join(
            f"- [{e.node_title}] {len(e.content)} chars" for e in self.evidence
        )

    def evidence_for_check(self) -> str:
        if not self.evidence:
            return "(no evidence collected yet)"
        return "\n\n".join(
            f"[{e.node_title}]\n{e.content}" for e in self.evidence
        )

    def history_text(self) -> str:
        if not self.history:
            return "(no history yet)"
        return "\n".join(
            f"{i + 1}. {h}" for i, h in enumerate(self.history)
        )

    def into_worker_output(
        self,
        llm_calls: int,
        budget_exhausted: bool,
        doc_name: str,
    ) -> WorkerOutput:
        """Convert this state into a WorkerOutput (consuming the evidence).

        Worker returns evidence only — no answer synthesis.
        """
        evidence_chars: int = sum(len(e.content) for e in self.evidence)
        return WorkerOutput(
            evidence=list(self.evidence),
            metrics=WorkerMetrics(
                rounds_used=self.max_rounds - self.remaining,
                llm_calls=llm_calls,
                nodes_visited=len(self.visited),
                budget_exhausted=budget_exhausted,
                plan_generated=self.plan_generated,
                check_count=self.check_count,
                evidence_chars=evidence_chars,
            ),
            doc_name=doc_name,
            trace_steps=list(self.trace_steps),
        )


MAX_HISTORY_ENTRIES: int = 6


# ---------------------------------------------------------------------------
# OrchestratorState — mirrors state::OrchestratorState
# ---------------------------------------------------------------------------

@dataclass
class OrchestratorState:
    """Mutable state for the Orchestrator loop.

    Tracks which documents have been dispatched and collects Worker results.
    """

    dispatched: list[int] = field(default_factory=list)
    sub_results: list[WorkerOutput] = field(default_factory=list)
    all_evidence: list[Evidence] = field(default_factory=list)
    analyze_done: bool = False
    total_llm_calls: int = 0

    def record_dispatch(self, doc_idx: int) -> None:
        if doc_idx not in self.dispatched:
            self.dispatched.append(doc_idx)

    def collect_result(self, doc_idx: int, result: WorkerOutput) -> None:
        """Collect a Worker result, including its LLM call count."""
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
