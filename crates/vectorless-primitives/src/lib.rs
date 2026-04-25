// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document navigation primitives for AI agents.
//!
//! Provides [`DocumentNavigator`] — a stateful navigator over an understood
//! document, with methods for tree traversal, content reading, regex search,
//! evidence collection, and index queries.
//!
//! All methods are `async` for compatibility with the PyO3 async bridge.

pub mod navigator;
pub mod resolve;
pub mod subtree;
pub mod types;

pub use navigator::DocumentNavigator;
pub use types::*;
