// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Frontend passes — parse document into AST.

mod build;
mod parse;

pub use build::BuildPass;
pub use parse::ParsePass;
