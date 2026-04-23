"""Ask pipeline — query understanding, multi-agent retrieval, and answer synthesis."""

from vectorless.ask.dispatcher import dispatch
from vectorless.ask.evaluate import evaluate
from vectorless.ask.orchestrator import Orchestrator
from vectorless.ask.plan import Complexity, QueryIntent, QueryPlan, SubQuery
from vectorless.ask.types import (
    DispatchEntry,
    DocCard,
    EvalResult,
    Evidence,
    Metrics,
    OrchestratorState,
    Output,
    Scope,
    Specified,
    TraceStep,
    WorkerMetrics,
    WorkerOutput,
    WorkerState,
    Workspace,
)
from vectorless.ask.understand import understand
from vectorless.ask.worker import Worker

__all__ = [
    # Core output types
    "Output",
    "Evidence",
    "Metrics",
    "TraceStep",
    # Worker types
    "WorkerOutput",
    "WorkerMetrics",
    "WorkerState",
    # Orchestrator types
    "Orchestrator",
    "OrchestratorState",
    "DispatchEntry",
    "DocCard",
    "EvalResult",
    # Scope types
    "Scope",
    "Specified",
    "Workspace",
    # Query understanding
    "QueryIntent",
    "QueryPlan",
    "SubQuery",
    "Complexity",
    # Agents
    "Worker",
    "dispatch",
    "evaluate",
    "understand",
]
