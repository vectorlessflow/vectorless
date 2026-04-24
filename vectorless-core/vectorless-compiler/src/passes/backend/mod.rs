// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Backend passes — code generation (indexes), verification, and optimization.

mod reasoning;
mod concept;
mod navigation;
mod verify;
mod optimize;
mod route;
mod chain;
mod overlap;
mod score;

pub use reasoning::ReasoningPass;
pub use concept::ConceptPass;
pub use navigation::NavigationPass;
pub use verify::VerifyPass;
pub use optimize::OptimizePass;
pub use route::RoutePass;
pub use chain::ChainPass;
pub use overlap::OverlapPass;
pub use score::ScorePass;
