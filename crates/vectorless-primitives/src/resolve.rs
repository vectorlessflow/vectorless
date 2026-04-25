// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Target resolution — map a user-provided string to a NodeId.
//!
//! Extracted from `vectorless-agent/src/command.rs` (strategy layer).
//! Command parsing stays in agent; only resolution logic lives here.

use vectorless_document::{DocumentTree, NavigationIndex, NodeId};

/// Strip surrounding quotes from a target string.
///
/// Handles straight quotes (`"`, `'`) and Unicode smart quotes.
pub fn strip_quotes(s: &str) -> String {
    let trimmed = s.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() < 2 {
        return trimmed.to_string();
    }
    let (first, last) = (chars[0], chars[chars.len() - 1]);
    let matching = (first == '"' && last == '"')
        || (first == '\'' && last == '\'')
        || (first == '\u{201c}' && last == '\u{201d}')
        || (first == '\u{2018}' && last == '\u{2019}');
    if matching {
        trimmed[chars[0].len_utf8()..trimmed.len() - chars[chars.len() - 1].len_utf8()].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Resolve a target string to a NodeId using multi-level matching.
///
/// Matching priority:
/// 1. Exact title match
/// 2. Case-insensitive title match
/// 3. Substring (contains) match
/// 4. Numeric index match ("1" → first child, "2" → second, etc.)
pub fn resolve_target(
    target: &str,
    nav_index: &NavigationIndex,
    current_node: NodeId,
) -> Option<NodeId> {
    let target = strip_quotes(target);
    let routes = nav_index.get_child_routes(current_node)?;

    // 1. Exact match
    if let Some(r) = routes.iter().find(|r| r.title == target) {
        return Some(r.node_id);
    }

    // 2. Case-insensitive match
    let target_lower = target.to_lowercase();
    if let Some(r) = routes
        .iter()
        .find(|r| r.title.to_lowercase() == target_lower)
    {
        return Some(r.node_id);
    }

    // 3. Substring (contains) match
    if let Some(r) = routes
        .iter()
        .find(|r| r.title.to_lowercase().contains(&target_lower))
    {
        return Some(r.node_id);
    }

    // 4. Numeric index match ("1" → first child)
    if let Ok(idx) = target.parse::<usize>() {
        if idx > 0 && idx <= routes.len() {
            return Some(routes[idx - 1].node_id);
        }
    }

    None
}

/// Resolve a target with additional context from tree node titles.
///
/// Matching priority:
/// 1. Direct children via NavigationIndex (exact, case-insensitive, substring, numeric)
/// 2. Direct children via TreeNode titles (case-insensitive contains)
/// 3. Deep descendant search (BFS, up to depth 4)
pub fn resolve_target_extended(
    target: &str,
    nav_index: &NavigationIndex,
    current_node: NodeId,
    tree: &DocumentTree,
) -> Option<NodeId> {
    let target = strip_quotes(target);
    // Try the primary resolver first
    if let Some(id) = resolve_target(&target, nav_index, current_node) {
        return Some(id);
    }

    let target_lower = target.to_lowercase();

    // Extended: check all direct children by their TreeNode titles
    for child_id in tree.children_iter(current_node) {
        if let Some(node) = tree.get(child_id) {
            if node.title.to_lowercase().contains(&target_lower) {
                return Some(child_id);
            }
        }
    }

    // Deep search: BFS through descendants up to depth 4.
    search_descendants(&target_lower, current_node, tree, 4)
}

/// BFS search through descendants, returning the shallowest matching NodeId.
fn search_descendants(
    target_lower: &str,
    start: NodeId,
    tree: &DocumentTree,
    max_depth: usize,
) -> Option<NodeId> {
    let mut queue: Vec<(NodeId, usize)> = vec![(start, 0)];

    while let Some((node_id, depth)) = queue.pop() {
        if depth >= max_depth {
            continue;
        }
        for child_id in tree.children_iter(node_id) {
            if let Some(node) = tree.get(child_id) {
                if node.title.to_lowercase().contains(target_lower) {
                    return Some(child_id);
                }
            }
            queue.push((child_id, depth + 1));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorless_document::{ChildRoute, DocumentTree, NavigationIndex};

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'hello'"), "hello");
        assert_eq!(strip_quotes("hello"), "hello");
    }

    #[test]
    fn test_resolve_target_exact() {
        let mut tree = DocumentTree::new("Root", "root");
        let root = tree.root();
        let c1 = tree.add_child(root, "Getting Started", "gs");

        let mut nav = NavigationIndex::new();
        nav.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: c1,
                title: "Getting Started".into(),
                description: "Setup".into(),
                leaf_count: 3,
            }],
        );

        assert_eq!(resolve_target("Getting Started", &nav, root), Some(c1));
    }

    #[test]
    fn test_resolve_target_case_insensitive() {
        let mut tree = DocumentTree::new("Root", "root");
        let root = tree.root();
        let c1 = tree.add_child(root, "Getting Started", "gs");

        let mut nav = NavigationIndex::new();
        nav.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: c1,
                title: "Getting Started".into(),
                description: "Setup".into(),
                leaf_count: 3,
            }],
        );

        assert_eq!(resolve_target("getting started", &nav, root), Some(c1));
    }

    #[test]
    fn test_resolve_target_numeric() {
        let mut tree = DocumentTree::new("Root", "root");
        let root = tree.root();
        let c1 = tree.add_child(root, "First", "1");
        let c2 = tree.add_child(root, "Second", "2");

        let mut nav = NavigationIndex::new();
        nav.add_child_routes(
            root,
            vec![
                ChildRoute {
                    node_id: c1,
                    title: "First".into(),
                    description: "1".into(),
                    leaf_count: 1,
                },
                ChildRoute {
                    node_id: c2,
                    title: "Second".into(),
                    description: "2".into(),
                    leaf_count: 1,
                },
            ],
        );

        assert_eq!(resolve_target("1", &nav, root), Some(c1));
        assert_eq!(resolve_target("2", &nav, root), Some(c2));
        assert_eq!(resolve_target("3", &nav, root), None);
    }

    #[test]
    fn test_resolve_target_extended_deep() {
        let mut tree = DocumentTree::new("Root", "root");
        let root = tree.root();
        let wrapper = tree.add_child(root, "Wrapper", "w");
        let labs = tree.add_child(wrapper, "Research Labs", "labs");
        let lab_b = tree.add_child(labs, "Lab B", "lb");

        let mut nav = NavigationIndex::new();
        nav.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: wrapper,
                title: "Wrapper".into(),
                description: "W".into(),
                leaf_count: 2,
            }],
        );
        nav.add_child_routes(
            wrapper,
            vec![ChildRoute {
                node_id: labs,
                title: "Research Labs".into(),
                description: "Labs".into(),
                leaf_count: 1,
            }],
        );
        nav.add_child_routes(
            labs,
            vec![ChildRoute {
                node_id: lab_b,
                title: "Lab B".into(),
                description: "LB".into(),
                leaf_count: 1,
            }],
        );

        assert_eq!(
            resolve_target_extended("Research Labs", &nav, root, &tree),
            Some(labs)
        );
        assert_eq!(
            resolve_target_extended("Lab B", &nav, root, &tree),
            Some(lab_b)
        );
    }
}
