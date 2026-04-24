// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Frontend passes — parse document into AST.

mod parse;
mod build;

pub use parse::ParsePass;
pub use build::BuildPass;
