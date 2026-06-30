"""Ask pipeline — query reasoning, plan-once retrieval, and answer synthesis."""

from vectorless.ask.dispatcher import dispatch
from vectorless.ask.errors import (
    AskError,
    BudgetExceededError,
    LLMFailureError,
    NavigationError,
    ParseError,
    VerificationError,
)
from vectorless.ask.orchestrator import Orchestrator
from vectorless.ask.protocols import NavigableDocument
from vectorless.ask.scout import Scout
from vectorless.ask.types import (
    DispatchEntry,
    DocCard,
    Evidence,
    Metrics,
    OrchestratorState,
    Output,
    Scope,
    Specified,
    TraceStep,
    WorkerMetrics,
    WorkerOutput,
    Workspace,
)
from vectorless.ask.events import AskEvent
from vectorless.ask.reasoning import (
    Ambiguity,
    AmbiguityType,
    EntityRef,
    QueryAnalysis,
    QueryAnalyzer,
    RetrievalStrategy,
    TemporalConstraint,
)

__all__ = [
    # Core output types
    "Output",
    "Evidence",
    "Metrics",
    "TraceStep",
    # Error types
    "AskError",
    "LLMFailureError",
    "ParseError",
    "BudgetExceededError",
    "NavigationError",
    "VerificationError",
    # Retrieval output types
    "WorkerOutput",
    "WorkerMetrics",
    # Orchestrator + agents
    "Orchestrator",
    "OrchestratorState",
    "Scout",
    "NavigableDocument",
    "DispatchEntry",
    "DocCard",
    "dispatch",
    # Scope types
    "Scope",
    "Specified",
    "Workspace",
    # Query reasoning
    "QueryAnalysis",
    "QueryAnalyzer",
    "EntityRef",
    "Ambiguity",
    "AmbiguityType",
    "TemporalConstraint",
    "RetrievalStrategy",
    # Events
    "AskEvent",
]
