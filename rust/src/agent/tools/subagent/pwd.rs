// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! `pwd` — show current navigation path.

use crate::agent::state::State;

use super::super::ToolResult;

/// Execute `pwd` — show current navigation path.
pub fn pwd(state: &State) -> ToolResult {
    ToolResult::ok(format!("Current path: {}", state.path_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{ChildRoute, DocumentTree, NavigationIndex};
    use crate::agent::config::DocContext;
    use crate::agent::tools::subagent::cd::cd;

    fn build_test_tree() -> (DocumentTree, NavigationIndex) {
        let mut tree = DocumentTree::new("Root", "root content");
        let root = tree.root();
        let c1 = tree.add_child(root, "API Reference", "api content");

        let mut nav = NavigationIndex::new();
        nav.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: c1,
                title: "API Reference".to_string(),
                description: "API docs".to_string(),
                leaf_count: 7,
            }],
        );

        (tree, nav)
    }

    #[test]
    fn test_pwd() {
        let (tree, nav) = build_test_tree();
        let root = tree.root();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let mut state = State::new(root, 8);
        cd("API Reference", &ctx, &mut state);

        let result = pwd(&state);
        assert!(result.success);
        assert!(result.feedback.contains("API Reference"));
    }
}
