"""Event types for the ask pipeline.

Defines the event protocol and enum of known event names.
The Orchestrator emits events at each stage so users can hook into
monitoring, logging, cost tracking, etc.
"""

from __future__ import annotations

from enum import Enum


class AskEvent(str, Enum):
    """Events emitted during the ask pipeline lifecycle."""

    QUERY_ANALYZED = "query_analyzed"
    WORKERS_DISPATCHED = "workers_dispatched"
    WORKER_COMPLETED = "worker_completed"
    EVIDENCE_COLLECTED = "evidence_collected"
    VERIFICATION_PASSED = "verification_passed"
    VERIFICATION_FAILED = "verification_failed"
    REPLAN_TRIGGERED = "replan_triggered"
    COMPLETED = "completed"
    ERROR = "error"
