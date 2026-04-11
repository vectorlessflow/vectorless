// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Re-export shim — event types and emitter live in the top-level
//! [`events`](crate::events) module.

pub use crate::events::{Event, EventEmitter, IndexEvent, QueryEvent, WorkspaceEvent};
