// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Stage execution outcomes for retrieval pipeline.
//!
//! The [`StageOutcome`] enum controls the flow of the retrieval pipeline,
//! allowing stages to continue, complete, request more data, or backtrack.

/// Result of a stage execution, controlling pipeline flow.
#[derive(Debug, Clone)]
pub enum StageOutcome {
    /// Stage completed successfully, continue to next stage.
    Continue,

    /// Entire retrieval is complete, return results.
    Complete,

    /// Need more data, go back to Search stage for another iteration.
    ///
    /// This enables incremental retrieval where the Evaluate stage can
    /// request additional search rounds if current results are insufficient.
    NeedMoreData {
        /// Additional beam width to add for next search iteration.
        additional_beam: usize,
        /// Whether to search deeper in the tree.
        go_deeper: bool,
    },

    /// Backtrack to a specific stage for re-planning.
    ///
    /// Used when current strategy isn't working and a different approach
    /// is needed.
    Backtrack {
        /// Target stage name to backtrack to.
        target_stage: String,
        /// Reason for backtracking.
        reason: String,
    },

    /// Skip remaining stages and return current results.
    ///
    /// Used when results are "good enough" or when further processing
    /// wouldn't improve the outcome.
    Skip {
        /// Reason for skipping.
        reason: String,
    },
}

impl StageOutcome {
    /// Create a Continue outcome.
    pub fn cont() -> Self {
        Self::Continue
    }

    /// Create a Complete outcome.
    pub fn complete() -> Self {
        Self::Complete
    }

    /// Create a NeedMoreData outcome.
    pub fn need_more(additional_beam: usize, go_deeper: bool) -> Self {
        Self::NeedMoreData {
            additional_beam,
            go_deeper,
        }
    }

    /// Create a Backtrack outcome.
    pub fn backtrack(target: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::Backtrack {
            target_stage: target.into(),
            reason: reason.into(),
        }
    }

    /// Create a Skip outcome.
    pub fn skip(reason: impl Into<String>) -> Self {
        Self::Skip {
            reason: reason.into(),
        }
    }

    /// Check if this outcome indicates pipeline completion.
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete | Self::Skip { .. })
    }

    /// Check if this outcome requires backtracking.
    pub fn needs_backtrack(&self) -> bool {
        matches!(self, Self::Backtrack { .. } | Self::NeedMoreData { .. })
    }

    /// Get the target stage for backtracking, if any.
    pub fn backtrack_target(&self) -> Option<&str> {
        match self {
            Self::Backtrack { target_stage, .. } => Some(target_stage),
            Self::NeedMoreData { .. } => Some("search"),
            _ => None,
        }
    }
}

impl Default for StageOutcome {
    fn default() -> Self {
        Self::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outcome_constructors() {
        assert!(matches!(StageOutcome::cont(), StageOutcome::Continue));
        assert!(matches!(StageOutcome::complete(), StageOutcome::Complete));
    }

    #[test]
    fn test_need_more() {
        let outcome = StageOutcome::need_more(2, true);
        assert!(outcome.needs_backtrack());
        assert_eq!(outcome.backtrack_target(), Some("search"));
    }

    #[test]
    fn test_backtrack() {
        let outcome = StageOutcome::backtrack("plan", "strategy not working");
        assert!(outcome.needs_backtrack());
        assert_eq!(outcome.backtrack_target(), Some("plan"));
    }

    #[test]
    fn test_is_complete() {
        assert!(StageOutcome::complete().is_complete());
        assert!(StageOutcome::skip("reason").is_complete());
        assert!(!StageOutcome::cont().is_complete());
    }
}
