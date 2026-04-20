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
    /// Search for a keyword in the ReasoningIndex.
    Find { keyword: String },
    /// Regex search across node content in the current subtree.
    Grep { pattern: String },
    /// Preview first N lines of a node without collecting evidence.
    Head { target: String, lines: usize },
    /// Search for nodes by title pattern in the tree.
    FindTree { pattern: String },
    /// Show node content size (lines, chars).
    Wc { target: String },
    /// Show current navigation path.
    Pwd,
    /// Evaluate evidence sufficiency.
    Check,
    /// End navigation.
    Done,
}

/// Strip surrounding quotes from a target string.
///
/// Handles straight quotes (`"`, `'`) and Unicode smart quotes (U+201C/U+201D, U+2018/U+2019).
fn strip_quotes(s: &str) -> String {
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
        ["cat"] => Command::Cat {
            target: ".".to_string(),
        },
        ["cd", ".."] => Command::CdUp,
        ["cd", target] => Command::Cd {
            target: strip_quotes(target),
        },
        ["cd", _target, ..] => Command::Cd {
            // Handle "cd some name" by joining remaining parts
            target: strip_quotes(&parts[1..].join(" ")),
        },
        ["cat", target] => Command::Cat {
            target: strip_quotes(target),
        },
        ["cat", _target, ..] => Command::Cat {
            target: strip_quotes(&parts[1..].join(" ")),
        },
        ["find", keyword] => Command::Find {
            keyword: strip_quotes(keyword),
        },
        ["find", _keyword, ..] => Command::Find {
            keyword: strip_quotes(&parts[1..].join(" ")),
        },
        ["grep", pattern] => Command::Grep {
            pattern: strip_quotes(pattern),
        },
        ["grep", _pattern, ..] => Command::Grep {
            pattern: strip_quotes(&parts[1..].join(" ")),
        },
        ["head", target] => Command::Head {
            target: strip_quotes(target),
            lines: 20, // default
        },
        ["head", "-n", n, target @ ..] => Command::Head {
            target: strip_quotes(&target.join(" ")),
            lines: n.parse().unwrap_or(20),
        },
        ["head", _target, ..] => Command::Head {
            target: strip_quotes(&parts[1..].join(" ")),
            lines: 20,
        },
        ["findtree", pattern] => Command::FindTree {
            pattern: strip_quotes(pattern),
        },
        ["findtree", _pattern, ..] => Command::FindTree {
            pattern: strip_quotes(&parts[1..].join(" ")),
        },
        ["wc", target] => Command::Wc {
            target: strip_quotes(target),
        },
        ["wc", _target, ..] => Command::Wc {
            target: strip_quotes(&parts[1..].join(" ")),
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
    let target = strip_quotes(target);
    // Try the primary resolver first
    if let Some(id) = resolve_target(&target, nav_index, current_node) {
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
        // Quoted multi-word targets should have quotes stripped
        assert_eq!(
            parse_command("cd \"Vectorless Architecture Guide\""),
            Command::Cd {
                target: "Vectorless Architecture Guide".to_string()
            }
        );
        assert_eq!(
            parse_command("cd 'Vectorless Architecture Guide'"),
            Command::Cd {
                target: "Vectorless Architecture Guide".to_string()
            }
        );
        // Smart quotes
        assert_eq!(
            parse_command("\u{201c}Vectorless Architecture Guide\u{201d}"),
            Command::Ls // doesn't start with a command keyword
        );
    }

    #[test]
    fn test_strip_quotes_straight() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("'hello'"), "hello");
        assert_eq!(strip_quotes("hello"), "hello");
        assert_eq!(strip_quotes("\"only left"), "\"only left");
    }

    #[test]
    fn test_strip_quotes_smart() {
        assert_eq!(strip_quotes("\u{201c}hello\u{201d}"), "hello");
        assert_eq!(strip_quotes("\u{2018}hello\u{2019}"), "hello");
    }

    #[test]
    fn test_resolve_target_quoted() {
        use crate::document::{ChildRoute, DocumentTree};

        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "Vectorless Architecture Guide", "content");

        let mut nav_index = NavigationIndex::new();
        nav_index.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: c1,
                title: "Vectorless Architecture Guide".to_string(),
                description: "Main guide".to_string(),
                leaf_count: 5,
            }],
        );

        // Quoted target should still resolve
        assert_eq!(
            resolve_target("\"Vectorless Architecture Guide\"", &nav_index, root),
            Some(c1)
        );
        assert_eq!(
            resolve_target("'Vectorless Architecture Guide'", &nav_index, root),
            Some(c1)
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

    #[test]
    fn test_parse_grep() {
        assert_eq!(
            parse_command("grep EBITDA"),
            Command::Grep {
                pattern: "EBITDA".to_string()
            }
        );
        assert_eq!(
            parse_command("grep revenue.*2024"),
            Command::Grep {
                pattern: "revenue.*2024".to_string()
            }
        );
    }

    #[test]
    fn test_parse_head() {
        assert_eq!(
            parse_command("head Installation"),
            Command::Head {
                target: "Installation".to_string(),
                lines: 20
            }
        );
        assert_eq!(
            parse_command("head -n 5 API Reference"),
            Command::Head {
                target: "API Reference".to_string(),
                lines: 5
            }
        );
    }

    #[test]
    fn test_parse_findtree() {
        assert_eq!(
            parse_command("findtree revenue"),
            Command::FindTree {
                pattern: "revenue".to_string()
            }
        );
        assert_eq!(
            parse_command("findtree API Reference"),
            Command::FindTree {
                pattern: "API Reference".to_string()
            }
        );
    }

    #[test]
    fn test_parse_wc() {
        assert_eq!(
            parse_command("wc Installation"),
            Command::Wc {
                target: "Installation".to_string()
            }
        );
        assert_eq!(
            parse_command("wc API Reference"),
            Command::Wc {
                target: "API Reference".to_string()
            }
        );
    }
}
