"""Internal types for the Python strategy layer."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass
class TraceStep:
    """A single step in the reasoning trace."""

    action: str
    observation: str
    round: int


@dataclass
class WorkerEvidence:
    """Evidence collected from a document node during navigation."""

    node_id: str
    title: str
    content: str
    source_path: str


@dataclass
class WorkerResult:
    """Result of a Worker's navigation over a single document."""

    evidence: list[WorkerEvidence] = field(default_factory=list)
    trace: list[TraceStep] = field(default_factory=list)
    rounds_used: int = 0
    llm_calls: int = 0
    nodes_visited: int = 0
    budget_exhausted: bool = False
