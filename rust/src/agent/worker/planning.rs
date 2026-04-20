// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Navigation planning prompts — initial plan, re-plan, semantic hints, deep expansion.

use std::collections::HashSet;

use crate::scoring::bm25::{Bm25Engine, FieldDocument, extract_keywords};

use super::super::config::DocContext;
use super::super::context::FindHit;
use super::super::state::WorkerState;
use super::format::format_visited_titles;

/// Maximum total chars for keyword + semantic sections in planning prompt.
const PLAN_CONTEXT_BUDGET: usize = 1500;

/// Build the navigation planning prompt (Phase 1.5).
pub fn build_plan_prompt(
    query: &str,
    task: Option<&str>,
    ls_output: &str,
    doc_name: &str,
    keyword_hits: &[FindHit],
    ctx: &DocContext<'_>,
) -> (String, String) {
    let task_section = match task {
        Some(t) => format!("\nYour specific task: {}", t),
        None => String::new(),
    };

    let query_keywords = extract_keywords(query);
    let query_lower = query.to_lowercase();

    let mut keyword_section = if keyword_hits.is_empty() {
        String::new()
    } else {
        let mut section =
            String::from("\nKeyword index matches (use these to prioritize navigation):\n");
        for hit in keyword_hits {
            let mut entries = hit.entries.clone();
            entries.sort_by(|a, b| {
                b.weight
                    .partial_cmp(&a.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut seen = HashSet::new();
            for entry in &entries {
                if !seen.insert(entry.node_id) {
                    continue;
                }
                let ancestor_path = build_ancestor_path(entry.node_id, ctx);
                section.push_str(&format!(
                    "  - keyword '{}' → {} (depth {}, weight {:.2})\n",
                    hit.keyword, ancestor_path, entry.depth, entry.weight
                ));
                if section.len() > PLAN_CONTEXT_BUDGET {
                    section.push_str("  ... (more hits truncated)\n");
                    break;
                }
            }
            if section.len() > PLAN_CONTEXT_BUDGET {
                break;
            }
        }
        section
    };

    let deep_expansion = build_deep_expansion(keyword_hits, ctx);
    if !deep_expansion.is_empty() {
        if keyword_section.len() + deep_expansion.len() <= PLAN_CONTEXT_BUDGET {
            keyword_section.push_str(&deep_expansion);
        }
    }

    let semantic_section = build_semantic_hints(&query_keywords, &query_lower, ctx);

    let system = "You are a document navigation planner. Given a user question, the top-level \
         document structure, keyword index matches, and semantic hints, output a brief navigation \
         plan: which sections to visit and in what order. Prioritize sections that matched keywords \
         or semantic hints. The plan should be 2-5 steps. Each step should be a specific action \
         like \"cd to X, then cat Y\" or \"grep for Z in current subtree\". \
         Pay attention to 'Can answer' and 'Topics' annotations in the structure listing — \
         they indicate what questions each section addresses. \
         Output only the plan, nothing else.\n\n\
         Example plan for \"What is the Q1 revenue?\":\n\
         1. cd to Revenue (matched keyword 'revenue')\n\
         2. ls to see sub-sections\n\
         3. cat Q1 Report\n\
         4. check\n\
         5. done".to_string();

    let user = format!(
        "Document: {doc_name}\n\
         Top-level structure:\n{ls_output}{keyword_section}{semantic_section}\
         User question: {query}{task_section}\n\n\
         Navigation plan:"
    );

    (system, user)
}

/// Build a focused re-planning prompt when check returns INSUFFICIENT.
pub fn build_replan_prompt(
    query: &str,
    task: Option<&str>,
    state: &WorkerState,
    ctx: &DocContext<'_>,
) -> (String, String) {
    let task_section = match task {
        Some(t) => format!("\nOriginal sub-task: {}", t),
        None => String::new(),
    };

    let visited = format_visited_titles(state, ctx);
    let evidence_summary = state.evidence_summary();

    let current_children = match ctx.ls(state.current_node) {
        Some(routes) if !routes.is_empty() => {
            let items: Vec<String> = routes
                .iter()
                .map(|r| format!("  - {} ({} leaves)", r.title, r.leaf_count))
                .collect();
            format!("Children at current position:\n{}\n", items.join("\n"))
        }
        _ => "Current position is a leaf node — consider cd .. to go back.\n".to_string(),
    };

    let sibling_hints = build_sibling_hints(state, ctx);

    let system = "You are re-planning a document navigation strategy. The previous plan did not \
         find sufficient evidence. Given what's been found and what's still missing, generate a \
         focused 2-3 step plan. Each step should be a specific action like \
         \"cd to X, then cat Y\" or \"grep for Z in current subtree\". \
         Prefer exploring unvisited branches. If current branch is exhausted, cd .. and try \
         a different path. Output only the plan, nothing else."
        .to_string();

    let user = format!(
        "Original question: {query}{task_section}\n\
         Current position: /{}\n\
         Evidence collected so far:\n{evidence_summary}\n\
         What's missing: {}\n\
         Already visited: {visited}\n\
         {current_children}\
         {sibling_hints}\
         Remaining rounds: {}/{}\n\n\
         Revised navigation plan:",
        state.path_str(),
        state.missing_info,
        state.remaining,
        state.max_rounds,
    );

    (system, user)
}

/// Build the ancestor path string for a node (e.g., "root > Chapter 1 > Section 1.2").
pub fn build_ancestor_path(node_id: crate::document::NodeId, ctx: &DocContext<'_>) -> String {
    let mut path: Vec<crate::document::NodeId> = ctx.tree.ancestors_iter(node_id).collect();
    path.reverse();
    path.iter()
        .filter_map(|&id| ctx.node_title(id))
        .collect::<Vec<_>>()
        .join(" > ")
}

/// Build semantic hints section using BM25 scoring over child routes.
fn build_semantic_hints(
    query_keywords: &[String],
    query_lower: &str,
    ctx: &DocContext<'_>,
) -> String {
    let root = ctx.root();
    let routes = match ctx.ls(root) {
        Some(r) => r,
        None => return String::new(),
    };

    if routes.is_empty() {
        return String::new();
    }

    let field_docs: Vec<FieldDocument<String>> = routes
        .iter()
        .map(|route| {
            let nav = ctx.nav_entry(route.node_id);
            let overview = nav.map(|n| n.overview.as_str()).unwrap_or("");
            let hints_text = nav.map(|n| n.question_hints.join(" ")).unwrap_or_default();
            let tags_text = nav.map(|n| n.topic_tags.join(" ")).unwrap_or_default();
            let content = if overview.is_empty() && hints_text.is_empty() && tags_text.is_empty() {
                String::new()
            } else {
                format!("{} {} {}", overview, hints_text, tags_text)
            };
            FieldDocument::new(
                route.title.clone(),
                route.title.clone(),
                route.description.clone(),
                content,
            )
        })
        .collect();

    let engine = Bm25Engine::fit_to_corpus(&field_docs);
    let bm25_results: std::collections::HashMap<String, f32> = engine
        .search_weighted(query_lower, routes.len())
        .into_iter()
        .collect();

    let mut section = String::new();
    let budget_remaining = PLAN_CONTEXT_BUDGET.saturating_sub(section.len());

    for route in routes {
        let nav = match ctx.nav_entry(route.node_id) {
            Some(n) => n,
            None => continue,
        };

        let bm25_score = bm25_results.get(&route.title).copied().unwrap_or(0.0);
        if bm25_score <= 0.0 {
            continue;
        }

        let mut annotations = Vec::new();

        for hint in &nav.question_hints {
            let hint_lower = hint.to_lowercase();
            for kw in query_keywords {
                if hint_lower.contains(&kw.to_lowercase()) {
                    annotations.push(format!("question \"{}\"", hint));
                    break;
                }
            }
            if !annotations.iter().any(|a| a.contains(&hint.clone())) {
                for word in hint_lower.split_whitespace() {
                    if word.len() > 3 && query_lower.contains(word) {
                        annotations.push(format!("question \"{}\"", hint));
                        break;
                    }
                }
            }
        }

        for tag in &nav.topic_tags {
            let tag_lower = tag.to_lowercase();
            for kw in query_keywords {
                if tag_lower.contains(&kw.to_lowercase()) || kw.to_lowercase().contains(&tag_lower)
                {
                    annotations.push(format!("topic \"{}\"", tag));
                    break;
                }
            }
            if !annotations
                .iter()
                .any(|a| a.contains(&format!("topic \"{}\"", tag)))
            {
                if query_lower.contains(&tag_lower) && tag.len() > 2 {
                    annotations.push(format!("topic \"{}\"", tag));
                }
            }
        }

        let annotation_str = if annotations.is_empty() {
            String::new()
        } else {
            format!(", {}", annotations.join(", "))
        };

        let line = format!(
            "  - Section '{}' — BM25: {:.2}{}\n",
            route.title, bm25_score, annotation_str
        );
        if section.len() + line.len() > budget_remaining {
            break;
        }
        section.push_str(&line);
    }

    if section.is_empty() {
        String::new()
    } else {
        format!(
            "\nSemantic hints (BM25-scored sections, higher = more relevant):\n{}",
            section
        )
    }
}

/// For keyword hits that land in deep nodes (depth >= 2), expand the parent node's children.
fn build_deep_expansion(keyword_hits: &[FindHit], ctx: &DocContext<'_>) -> String {
    if keyword_hits.is_empty() {
        return String::new();
    }

    let mut seen_parents = HashSet::new();
    let mut expansion = String::new();

    for hit in keyword_hits {
        for entry in &hit.entries {
            if entry.depth < 2 {
                continue;
            }
            let parent = match ctx.parent(entry.node_id) {
                Some(p) => p,
                None => continue,
            };
            if !seen_parents.insert(parent) {
                continue;
            }
            let routes = match ctx.ls(parent) {
                Some(r) => r,
                None => continue,
            };
            let parent_title = ctx.node_title(parent).unwrap_or("unknown");
            expansion.push_str(&format!(
                "Siblings near keyword hit '{}' (under {}):\n",
                hit.keyword, parent_title
            ));
            for route in routes {
                let marker = if ctx.node_title(entry.node_id) == Some(&route.title) {
                    " ← keyword hit"
                } else {
                    ""
                };
                expansion.push_str(&format!(
                    "  - {} ({} leaves){}\n",
                    route.title, route.leaf_count, marker
                ));
            }
            expansion.push('\n');
            if expansion.len() > 500 {
                expansion.push_str("  ... (more expansions truncated)\n");
                break;
            }
        }
        if expansion.len() > 500 {
            break;
        }
    }

    expansion
}

/// Build unvisited sibling branch hints for structured backtracking.
fn build_sibling_hints(state: &WorkerState, ctx: &DocContext<'_>) -> String {
    let mut hints = String::new();

    if let Some(parent) = ctx.parent(state.current_node) {
        if let Some(routes) = ctx.ls(parent) {
            let unvisited: Vec<&crate::document::ChildRoute> = routes
                .iter()
                .filter(|r| !state.visited.contains(&r.node_id))
                .collect();
            if !unvisited.is_empty() {
                hints.push_str("Unvisited sibling branches at current level:\n");
                for route in &unvisited {
                    hints.push_str(&format!(
                        "  - {} ({} leaves)\n",
                        route.title, route.leaf_count
                    ));
                }
            }
        }

        if let Some(grandparent) = ctx.parent(parent) {
            if let Some(routes) = ctx.ls(grandparent) {
                let unvisited_parent_siblings: Vec<&crate::document::ChildRoute> = routes
                    .iter()
                    .filter(|r| !state.visited.contains(&r.node_id) && r.node_id != parent)
                    .collect();
                if !unvisited_parent_siblings.is_empty() {
                    hints.push_str("Unvisited branches at parent level (cd .. then explore):\n");
                    for route in &unvisited_parent_siblings {
                        hints.push_str(&format!(
                            "  - {} ({} leaves)\n",
                            route.title, route.leaf_count
                        ));
                    }
                }
            }
        }
    }

    if hints.is_empty() {
        String::new()
    } else {
        format!("\n{}", hints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::DocContext;
    use crate::agent::config::Evidence;
    use crate::agent::state::WorkerState;
    use crate::document::{ChildRoute, NavEntry, NodeId};
    use crate::scoring::bm25::extract_keywords;

    fn build_semantic_test_tree() -> (
        crate::document::DocumentTree,
        crate::document::NavigationIndex,
        NodeId,
        NodeId,
        NodeId,
    ) {
        let mut tree = crate::document::DocumentTree::new("Root", "root content");
        let root = tree.root();
        let revenue = tree.add_child(root, "Revenue", "revenue content");
        let expenses = tree.add_child(root, "Expenses", "expense content");

        let mut nav = crate::document::NavigationIndex::new();
        nav.add_entry(
            root,
            NavEntry {
                overview: "Annual financial report".to_string(),
                question_hints: vec!["What is the financial overview?".to_string()],
                topic_tags: vec!["finance".to_string()],
                leaf_count: 4,
                level: 0,
            },
        );
        nav.add_child_routes(
            root,
            vec![
                ChildRoute {
                    node_id: revenue,
                    title: "Revenue".to_string(),
                    description: "Revenue breakdown".to_string(),
                    leaf_count: 2,
                },
                ChildRoute {
                    node_id: expenses,
                    title: "Expenses".to_string(),
                    description: "Cost analysis".to_string(),
                    leaf_count: 2,
                },
            ],
        );
        nav.add_entry(
            revenue,
            NavEntry {
                overview: "Revenue figures for 2024".to_string(),
                question_hints: vec![
                    "What is the total revenue?".to_string(),
                    "What was the Q1 revenue?".to_string(),
                ],
                topic_tags: vec![
                    "revenue".to_string(),
                    "sales".to_string(),
                    "income".to_string(),
                ],
                leaf_count: 2,
                level: 1,
            },
        );
        nav.add_entry(
            expenses,
            NavEntry {
                overview: "Operating expenses".to_string(),
                question_hints: vec!["What are the operating costs?".to_string()],
                topic_tags: vec!["expenses".to_string(), "costs".to_string()],
                leaf_count: 2,
                level: 1,
            },
        );

        (tree, nav, root, revenue, expenses)
    }

    #[test]
    fn test_build_ancestor_path() {
        let (tree, nav, root, revenue, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        assert_eq!(build_ancestor_path(revenue, &ctx), "Root > Revenue");
        assert_eq!(build_ancestor_path(root, &ctx), "Root");
    }

    #[test]
    fn test_semantic_hints_keyword_match() {
        let (tree, nav, _, _, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let keywords = extract_keywords("What is the revenue?");
        let hints = build_semantic_hints(&keywords, &"what is the revenue".to_lowercase(), &ctx);
        assert!(
            hints.contains("Revenue"),
            "Should match Revenue section, got: {}",
            hints
        );
        assert!(hints.contains("BM25"));
    }

    #[test]
    fn test_semantic_hints_topic_match() {
        let (tree, nav, _, _, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let keywords = extract_keywords("operating costs analysis");
        let hints =
            build_semantic_hints(&keywords, &"operating costs analysis".to_lowercase(), &ctx);
        assert!(
            hints.contains("Expenses"),
            "Should match Expenses via topic 'costs', got: {}",
            hints
        );
    }

    #[test]
    fn test_semantic_hints_no_match() {
        let (tree, nav, _, _, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let keywords = extract_keywords("xyzzy foobar");
        let hints = build_semantic_hints(&keywords, &"xyzzy foobar".to_lowercase(), &ctx);
        assert!(hints.is_empty(), "Should not match, got: {}", hints);
    }

    #[test]
    fn test_build_replan_prompt() {
        let (tree, nav, root, _, _) = build_semantic_test_tree();
        let mut state = WorkerState::new(root, 8);
        state.missing_info = "Need Q2 revenue figures".to_string();
        state.add_evidence(Evidence {
            source_path: "root/Revenue".to_string(),
            node_title: "Revenue".to_string(),
            content: "Q1 revenue was $2.5M".to_string(),
            doc_name: None,
        });
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let (system, user) = build_replan_prompt("What is total revenue?", None, &state, &ctx);
        assert!(system.contains("re-planning"));
        assert!(user.contains("What is total revenue?"));
        assert!(user.contains("Q2 revenue"));
    }

    #[test]
    fn test_build_plan_prompt_with_semantic_hints() {
        let (tree, nav, _, _, _) = build_semantic_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "Financial Report",
        };
        let ls_output =
            "[1] Revenue — Revenue breakdown (2 leaves)\n[2] Expenses — Cost analysis (2 leaves)\n";
        let (system, user) = build_plan_prompt(
            "What is the revenue?",
            None,
            ls_output,
            "Financial Report",
            &[],
            &ctx,
        );
        assert!(system.contains("semantic hints"));
        assert!(user.contains("What is the revenue?"));
    }
}
