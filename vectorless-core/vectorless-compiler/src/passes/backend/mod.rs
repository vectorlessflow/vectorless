// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Backend passes — code generation (indexes), verification, and optimization.

mod chain;
mod concept;
mod navigation;
mod optimize;
mod overlap;
mod reasoning;
mod route;
mod score;
mod verify;

pub use chain::ChainPass;
pub use concept::ConceptPass;
pub use navigation::NavigationPass;
pub use optimize::OptimizePass;
pub use overlap::OverlapPass;
pub use reasoning::ReasoningPass;
pub use route::RoutePass;
pub use score::ScorePass;
pub use verify::VerifyPass;
