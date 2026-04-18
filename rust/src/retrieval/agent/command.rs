// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Command parsing for the agent navigation loop.
//!
//! LLM output is parsed into `Command` variants. The parser is intentionally
//! simple and forgiving — unknown input falls back to `Ls` so the agent can
//! re-observe its surroundings.

use crate::document::{NavigationIndex, NodeId};

/// Parsed command from LLM output.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// List children of the current node.
    Ls,
    /// Navigate into a child node by name.
    Cd { target: String },
    /// Navigate back to parent.
    CdUp,
    /// Read node content (collects as evidence).
    Cat { target: String },
    /// Search for a keyword in the document.
    Find { keyword: String },
    /// Show current navigation path.
    Pwd,
    /// Evaluate evidence sufficiency.
    Check,
    /// End navigation.
    Done,
}

/// Parse the first non-empty line of LLM output into a Command.
pub fn parse_command(llm_output: &str) -> Command {
    let line = llm_output
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();

    // Remove common wrapping (markdown code blocks, etc.)
    let line = line.trim_start_matches('`').trim_end_matches('`').trim();

    let parts: Vec<&str> = line.split_whitespace().collect();

    match parts.as_slice() {
        ["ls"] => Command::Ls,
        ["cd", ".."] => Command::CdUp,
        ["cd", target] => Command::Cd {
            target: (*target).to_string(),
        },
        ["cd", _target, ..] => Command::Cd {
            // Handle "cd some name" by joining remaining parts
            target: parts[1..].join(" "),
        },
        ["cat", target] => Command::Cat {
            target: (*target).to_string(),
        },
        ["cat", _target, ..] => Command::Cat {
            target: parts[1..].join(" "),
        },
        ["find", keyword] => Command::Find {
            keyword: (*keyword).to_string(),
        },
        ["find", _keyword, ..] => Command::Find {
            keyword: parts[1..].join(" "),
        },
        ["pwd"] => Command::Pwd,
        ["check"] => Command::Check,
        ["done"] => Command::Done,
        _ => Command::Ls, // fallback: re-observe
    }
}

/// Resolve a cd/cat target string to a NodeId using multi-level matching.
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

/// Resolve a cd/cat target with additional context from the tree node titles.
///
/// This extended resolver also checks against the actual tree node titles
/// (in case NavEntry titles differ from TreeNode titles).
pub fn resolve_target_extended(
    target: &str,
    nav_index: &NavigationIndex,
    current_node: NodeId,
    tree: &crate::document::DocumentTree,
) -> Option<NodeId> {
    // Try the primary resolver first
    if let Some(id) = resolve_target(target, nav_index, current_node) {
        return Some(id);
    }

    // Extended: check all children by their TreeNode titles
    let children: Vec<NodeId> = tree.children_iter(current_node).collect();
    let target_lower = target.to_lowercase();

    for child_id in &children {
        if let Some(node) = tree.get(*child_id) {
            if node.title.to_lowercase().contains(&target_lower) {
                return Some(*child_id);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ls() {
        assert_eq!(parse_command("ls"), Command::Ls);
        assert_eq!(parse_command("  ls  "), Command::Ls);
    }

    #[test]
    fn test_parse_cd() {
        assert_eq!(parse_command("cd .."), Command::CdUp);
        assert_eq!(
            parse_command("cd Getting Started"),
            Command::Cd {
                target: "Getting Started".to_string()
            }
        );
        assert_eq!(
            parse_command("cd some long name"),
            Command::Cd {
                target: "some long name".to_string()
            }
        );
    }

    #[test]
    fn test_parse_cat() {
        assert_eq!(
            parse_command("cat Installation"),
            Command::Cat {
                target: "Installation".to_string()
            }
        );
        assert_eq!(
            parse_command("cat API Reference"),
            Command::Cat {
                target: "API Reference".to_string()
            }
        );
    }

    #[test]
    fn test_parse_find() {
        assert_eq!(
            parse_command("find authentication"),
            Command::Find {
                keyword: "authentication".to_string()
            }
        );
    }

    #[test]
    fn test_parse_misc() {
        assert_eq!(parse_command("pwd"), Command::Pwd);
        assert_eq!(parse_command("check"), Command::Check);
        assert_eq!(parse_command("done"), Command::Done);
    }

    #[test]
    fn test_parse_fallback() {
        assert_eq!(parse_command(""), Command::Ls);
        assert_eq!(parse_command("unknown command"), Command::Ls);
        assert_eq!(parse_command("blah blah"), Command::Ls);
    }

    #[test]
    fn test_parse_with_wrapping() {
        assert_eq!(parse_command("`ls`"), Command::Ls);
        assert_eq!(parse_command("```ls```"), Command::Ls);
    }

    #[test]
    fn test_parse_multiline() {
        // Should parse the first non-empty line
        assert_eq!(parse_command("\n\nls\n\n// listing children"), Command::Ls);
    }

    #[test]
    fn test_resolve_target_numeric() {
        use crate::document::{ChildRoute, DocumentTree};

        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "Getting Started", "content");
        let c2 = tree.add_child(root, "API Reference", "content");

        let mut nav_index = NavigationIndex::new();
        nav_index.add_child_routes(
            root,
            vec![
                ChildRoute {
                    node_id: c1,
                    title: "Getting Started".to_string(),
                    description: "Setup guide".to_string(),
                    leaf_count: 3,
                },
                ChildRoute {
                    node_id: c2,
                    title: "API Reference".to_string(),
                    description: "API docs".to_string(),
                    leaf_count: 7,
                },
            ],
        );

        assert_eq!(resolve_target("1", &nav_index, root), Some(c1));
        assert_eq!(resolve_target("2", &nav_index, root), Some(c2));
        assert_eq!(resolve_target("3", &nav_index, root), None);
    }

    #[test]
    fn test_resolve_target_exact() {
        use crate::document::{ChildRoute, DocumentTree};

        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "Getting Started", "content");

        let mut nav_index = NavigationIndex::new();
        nav_index.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: c1,
                title: "Getting Started".to_string(),
                description: "Setup".to_string(),
                leaf_count: 3,
            }],
        );

        assert_eq!(
            resolve_target("Getting Started", &nav_index, root),
            Some(c1)
        );
    }

    #[test]
    fn test_resolve_target_case_insensitive() {
        use crate::document::{ChildRoute, DocumentTree};

        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "Getting Started", "content");

        let mut nav_index = NavigationIndex::new();
        nav_index.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: c1,
                title: "Getting Started".to_string(),
                description: "Setup".to_string(),
                leaf_count: 3,
            }],
        );

        assert_eq!(
            resolve_target("getting started", &nav_index, root),
            Some(c1)
        );
        assert_eq!(
            resolve_target("GETTING STARTED", &nav_index, root),
            Some(c1)
        );
    }

    #[test]
    fn test_resolve_target_contains() {
        use crate::document::{ChildRoute, DocumentTree};

        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "API Reference", "content");

        let mut nav_index = NavigationIndex::new();
        nav_index.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: c1,
                title: "API Reference".to_string(),
                description: "API docs".to_string(),
                leaf_count: 7,
            }],
        );

        assert_eq!(resolve_target("api", &nav_index, root), Some(c1));
        assert_eq!(resolve_target("reference", &nav_index, root), Some(c1));
    }

    #[test]
    fn test_resolve_target_no_routes() {
        let nav_index = NavigationIndex::new();
        let tree = crate::document::DocumentTree::new("Root", "");
        assert!(resolve_target("anything", &nav_index, tree.root()).is_none());
    }
}
