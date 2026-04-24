"""Verification module — multi-dimensional evidence verification pipeline."""

from vectorless.ask.verify.types import (
    DimensionScore,
    VerificationDimension,
    VerificationResult,
)
from vectorless.ask.verify.verifier import VerifyPipeline

__all__ = [
    "DimensionScore",
    "VerifyPipeline",
    "VerificationDimension",
    "VerificationResult",
]
