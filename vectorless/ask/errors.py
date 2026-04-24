"""Structured error types for the ask pipeline.

Replaces bare ``except Exception`` with a typed hierarchy.
"""

from __future__ import annotations


class AskError(Exception):
    """Base error for all ask pipeline failures."""


class LLMFailureError(AskError):
    """LLM call failed after retries."""

    def __init__(self, message: str, *, attempts: int = 0) -> None:
        super().__init__(message)
        self.attempts = attempts


class ParseError(AskError):
    """Failed to parse LLM output into structured data."""

    def __init__(self, message: str, *, raw_output: str = "") -> None:
        super().__init__(message)
        self.raw_output = raw_output


class BudgetExceededError(AskError):
    """Token or call budget exceeded."""


class NavigationError(AskError):
    """Document navigation failure (load, cd, cat, etc.)."""


class VerificationError(AskError):
    """Verification pipeline failure."""
