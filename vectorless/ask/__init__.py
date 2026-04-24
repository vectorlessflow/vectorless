"""Ask pipeline — query reasoning, multi-agent retrieval, and answer synthesis."""

from vectorless.ask.dispatcher import dispatch
from vectorless.ask.errors import AskError, BudgetExceededError, LLMFailureError, NavigationError, ParseError, VerificationError
from vectorless.ask.evaluate import evaluate
from vectorless.ask.orchestrator import Orchestrator
from vectorless.ask.protocols import NavigableDocument
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

# New modules
from vectorless.ask.blackboard import Discovery, SharedBlackboard
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
from vectorless.ask.verify import (
    DimensionScore,
    VerificationDimension,
    VerificationResult,
    VerifyPipeline,
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
    # Worker types
    "WorkerOutput",
    "WorkerMetrics",
    "WorkerState",
    # Orchestrator types
    "Orchestrator",
    "OrchestratorState",
    "NavigableDocument",
    "DispatchEntry",
    "DocCard",
    "EvalResult",
    # Scope types
    "Scope",
    "Specified",
    "Workspace",
    # Query understanding (legacy)
    "QueryIntent",
    "QueryPlan",
    "SubQuery",
    "Complexity",
    # Query reasoning (new)
    "QueryAnalysis",
    "QueryAnalyzer",
    "EntityRef",
    "Ambiguity",
    "AmbiguityType",
    "TemporalConstraint",
    "RetrievalStrategy",
    # Agents
    "Worker",
    "dispatch",
    "evaluate",
    "understand",
    # Shared blackboard
    "Discovery",
    "SharedBlackboard",
    # Events
    "AskEvent",
    # Verification
    "VerifyPipeline",
    "VerificationDimension",
    "VerificationResult",
    "DimensionScore",
]
