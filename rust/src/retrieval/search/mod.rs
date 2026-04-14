// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Search algorithms for tree traversal.
//!
//! This module contains only tree traversal strategies (Beam, MCTS, Greedy).
//! All scoring intelligence lives in the `pilot` module.
//! BM25 and keyword utilities live in the `scoring` module.

mod beam;
mod greedy;
mod mcts;
mod toc_navigator;
mod r#trait;

pub use beam::BeamSearch;
pub use greedy::PurePilotSearch;
pub use mcts::MctsSearch;
pub use toc_navigator::{SearchCue, ToCNavigator};
pub use r#trait::{SearchConfig, SearchResult, SearchTree};
