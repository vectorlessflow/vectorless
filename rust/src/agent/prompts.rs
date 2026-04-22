// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Prompt templates for the retrieval agent.
//!
//! Prompts for agent-level operations:
//! 1. `worker_navigation` — Worker nav loop, every round
//! 2. `orchestrator_analysis` — Orchestrator Phase 1
//! 3. `worker_dispatch` — Worker first round (when dispatched by Orchestrator)
//! 4. `check_sufficiency` — evidence sufficiency evaluation
//!
//! Post-processing prompts (answer synthesis, multi-doc fusion) have been
//! moved to `rerank/synthesis.rs` and `rerank/fusion.rs`.

// ---------------------------------------------------------------------------
// Prompt 1: Worker Navigation (used every round in the nav loop)
// ---------------------------------------------------------------------------

/// Parameters for the sub-agent navigation prompt.
pub struct NavigationParams<'a> {
    pub query: &'a str,
    /// Sub-task description (None when Worker is called directly).
    pub task: Option<&'a str>,
    /// Current breadcrumb path.
    pub breadcrumb: &'a str,
    /// Summary of collected evidence.
    pub evidence_summary: &'a str,
    /// Description of what's still missing (empty string if nothing).
    pub missing_info: &'a str,
    /// Feedback from the last command execution.
    pub last_feedback: &'a str,
    /// Remaining rounds.
    pub remaining: u32,
    /// Maximum rounds.
    pub max_rounds: u32,
    /// ReAct history of recent rounds.
    pub history: &'a str,
    /// Titles of already-visited nodes.
    pub visited_titles: &'a str,
    /// Navigation plan from bird's-eye analysis (empty if no plan).
    pub plan: &'a str,
    /// Query intent context from QueryPlan (e.g. "factual — find specific answer").
    /// Empty string if not available.
    pub intent_context: &'a str,
    /// Formatted keyword index matches (empty if none).
    pub keyword_hints: &'a str,
}

pub fn worker_navigation(params: &NavigationParams) -> (String, String) {
    let query = params.query;
    let breadcrumb = params.breadcrumb;
    let evidence_summary = params.evidence_summary;
    let remaining = params.remaining;
    let max_rounds = params.max_rounds;

    let task_section = match params.task {
        Some(task) => format!(
            "\nYour specific task: {}\n(This is a sub-task for the original query.)",
            task
        ),
        None => String::new(),
    };

    let missing_section = if params.missing_info.is_empty() {
        String::new()
    } else {
        format!("\nPotentially missing info: {}", params.missing_info)
    };

    let last_feedback_section = if params.last_feedback.is_empty() {
        String::new()
    } else {
        format!("\nLast command result:\n{}\n", params.last_feedback)
    };

    let history_section = if params.history == "(no history yet)" {
        String::new()
    } else {
        format!("\nPrevious rounds:\n{}\n", params.history)
    };

    let visited_section = if params.visited_titles == "(none)" {
        String::new()
    } else {
        format!(
            "\nAlready visited (do not re-read these): {}",
            params.visited_titles
        )
    };

    let plan_section = if params.plan.is_empty() {
        String::new()
    } else {
        format!(
            "\nNavigation plan (follow this as guidance, adapt if needed):\n{}\n",
            params.plan
        )
    };

    let keyword_section = if params.keyword_hints.is_empty() {
        String::new()
    } else {
        format!("\n{}", params.keyword_hints)
    };

    let intent_section = if params.intent_context.is_empty() {
        String::new()
    } else {
        format!("\nQuery context: {}", params.intent_context)
    };

    let system = format!(
        "You are a document navigation assistant. You navigate inside a document to find \
         information that answers the user's question.

Available commands:
- ls                List children at current position (with summaries and leaf counts)
- cd <name>         Enter a child node (supports relative paths like Section/Sub and absolute paths like /root/Section)
- cd ..             Go back to parent node
- cat <name>        Read a child node's content (automatically collected as evidence)
- cat               Read the current node's content (useful at leaf nodes)
- head <name>       Preview first 20 lines of a node (does NOT collect evidence)
- find <keyword>    Search for a keyword in the document index (also supports multi-word like 'Lab C')
- findtree <pattern> Search for nodes by title pattern (case-insensitive)
- grep <pattern>    Regex search across all content in current subtree
- wc <name>         Show content size (lines, words, chars)
- pwd               Show current navigation path
- check             Evaluate if collected evidence is sufficient
- done              End navigation

SEARCH STRATEGY (important — follow this priority order):
- When keyword matches are shown below, navigate directly to the highest-weight matched node. \
Do NOT explore other branches first — the keyword index has already identified the most relevant location.
- When find results include content snippets that answer the question, cd to that node and cat it immediately.
- Use find with the EXACT keyword from the list (single word, \
not multi-word phrases). Example: if hint shows keyword 'performance' pointing to Performance section, \
use find performance, NOT find \"performance guide\".
- Use ls only when you have no keyword hints or need to discover the structure of an unknown section.
- Use findtree when you know a section title pattern but not the exact name.

NAVIGATION EFFICIENCY (critical — every round counts):
- Prefer cd with absolute paths (/root/Section/Subsection) or relative paths (Section/Sub) \
to reach target nodes in ONE command instead of multiple cd steps.
- Do NOT ls before cd if keyword hints or find results already tell you which node to enter.
- Do NOT cd into nodes one level at a time when you can use a multi-segment path.

Rules:
- Output exactly ONE command per response, nothing else.
- Content from cat is automatically saved as evidence — don't re-cat the same node.
- Do not cat or cd into nodes you have already visited.
- If the current branch has nothing relevant, use cd .. to go back.
- If you're at the root and no children seem relevant, use done.

STOPPING RULES (critical — follow these strictly):
- After cat collects evidence, immediately check: does the collected text contain information \
  that answers or relates to the user's question? If YES, output done. Do NOT continue searching.
- Do NOT run grep after cat — cat already collected the full content. grep is for locating \
  content BEFORE cat, not after.
- If ls shows '(no navigation data)' or no children, you are at a leaf node. Use cat to read it \
  or cd .. to go back. Do NOT ls again.
- When remaining rounds are low (≤2), prefer done over exploring new branches."
    );

    let user = format!(
        "{last_feedback_section}\
User question: {query}{task_section}{intent_section}

Current position: /{breadcrumb}
Collected evidence:
{evidence_summary}{missing_section}{keyword_section}{visited_section}{plan_section}
{history_section}
Remaining rounds: {remaining}/{max_rounds}

Command:"
    );

    (system, user)
}

// ---------------------------------------------------------------------------
// Prompt 2: Orchestrator Analysis (multi-doc Phase 1)
// ---------------------------------------------------------------------------

/// Parameters for the orchestrator analysis prompt.
pub struct OrchestratorAnalysisParams<'a> {
    pub query: &'a str,
    /// Formatted DocCard listing from ls_docs.
    pub doc_cards: &'a str,
    /// Formatted cross-document search results.
    pub find_results: &'a str,
    /// Query understanding context (intent, concepts, strategy, complexity).
    pub intent_context: &'a str,
}

pub fn orchestrator_analysis(params: &OrchestratorAnalysisParams) -> (String, String) {
    let doc_cards = params.doc_cards;
    let find_results = params.find_results;
    let query = params.query;
    let intent_context = params.intent_context;

    let system =
        "You are a multi-document retrieval coordinator. Analyze the user's question, \
         review the available documents, and decide which documents to search and what to look for in each.

Output format — for each relevant document, output a block:
- doc: <number>
  reason: <why this document is relevant>
  task: <what specific information to find in this document>

Only include documents that are likely to contain relevant information.
If the cross-document search results already fully answer the question, respond with just: ALREADY_ANSWERED".to_string();

    let user = format!(
        "Available documents:
{doc_cards}

Cross-document search results:
{find_results}
{intent_context}

User question: {query}

Relevant documents:"
    );

    (system, user)
}

// ---------------------------------------------------------------------------
// Prompt 3: Worker Dispatch (first-round prompt when Orchestrator dispatches)
// ---------------------------------------------------------------------------

/// Parameters for the dispatch prompt.
pub struct WorkerDispatchParams<'a> {
    pub original_query: &'a str,
    pub task: &'a str,
    pub doc_name: &'a str,
    pub breadcrumb: &'a str,
}

pub fn worker_dispatch(params: &WorkerDispatchParams) -> (String, String) {
    let doc_name = params.doc_name;
    let original_query = params.original_query;
    let task = params.task;
    let breadcrumb = params.breadcrumb;

    let system = format!(
        "You are a document navigation assistant. You are searching inside the document \
         \"{doc_name}\" for specific information.

Available commands: ls, cd <name> (supports Section/Sub paths and /root/Section absolute paths), \
cd .., cat, cat <name>, head <name>, find <keyword>, findtree <pattern>, grep <regex>, wc <name>, \
pwd, check, done

SEARCH STRATEGY:
- Prefer find <keyword> to jump directly to relevant sections over manual ls→cd exploration.
- When find results include content snippets that answer your task, cd to that node and cat it immediately.
- Use multi-segment paths (e.g. cd Research Labs/Lab A) to reach targets in ONE command.
- Do NOT ls before cd if find results already tell you which node to enter.
- Use findtree when you know a section title pattern but not the exact name.

Rules:
- Output exactly ONE command per response.
- Content from cat is automatically saved as evidence.
- After cat collects evidence, if it relates to your task, use done immediately.
- Do NOT grep after cat — cat already collected the full content.
- If ls shows no children, use cat to read the current node or cd .. to go back.
- When evidence is sufficient, use done."
    );

    let user = format!(
        "Original question: {original_query}
Your task: {task}
Document: {doc_name}
Current position: /{breadcrumb}

Command:"
    );

    (system, user)
}

// ---------------------------------------------------------------------------
// Prompt 4: Check (evidence sufficiency evaluation)
// ---------------------------------------------------------------------------

/// Build the check prompt for LLM-based sufficiency evaluation.
pub fn check_sufficiency(query: &str, evidence_summary: &str) -> (String, String) {
    let system = "You evaluate whether collected evidence contains information that can answer or \
         relate to the user's question. The evidence is raw document text — it does not need to be \
         a complete or perfect answer. If the evidence mentions or addresses the key concepts from \
         the question, it is sufficient.

Respond with ONLY 'SUFFICIENT' or 'INSUFFICIENT' followed by a one-line reason.

Guidelines:
- If the evidence text contains any information directly related to the question's key terms, \
respond SUFFICIENT.
- If the evidence is completely unrelated or empty, respond INSUFFICIENT.
- Default to SUFFICIENT unless the evidence is clearly irrelevant."
        .to_string();

    let user = format!(
        "Question: {query}\n\n\
         Collected evidence:\n\
         {evidence_summary}\n\n\
         Is this sufficient?"
    );

    (system, user)
}

// ---------------------------------------------------------------------------
// Dispatch plan parsing
// ---------------------------------------------------------------------------

/// A single dispatch entry parsed from orchestrator analysis.
#[derive(Debug, Clone)]
pub struct DispatchEntry {
    /// Document index (0-based).
    pub doc_idx: usize,
    /// Why this document was selected.
    pub reason: String,
    /// What to search for in this document.
    pub task: String,
}

/// Parse the LLM output from orchestrator analysis into dispatch entries.
///
/// Returns `None` if the response is "ALREADY_ANSWERED".
/// Returns empty vec if no valid dispatch entries found.
pub fn parse_dispatch_plan(llm_output: &str, total_docs: usize) -> Option<Vec<DispatchEntry>> {
    let trimmed = llm_output.trim();

    if trimmed.starts_with("ALREADY_ANSWERED") {
        return None;
    }

    let mut entries = Vec::new();
    let mut current_doc_idx: Option<usize> = None;
    let mut current_reason = String::new();
    let mut current_task = String::new();

    for line in trimmed.lines() {
        let line = line.trim();

        if let Some(rest) = line.strip_prefix("- doc:") {
            // Flush previous entry
            if let Some(idx) = current_doc_idx.take() {
                entries.push(DispatchEntry {
                    doc_idx: idx,
                    reason: std::mem::take(&mut current_reason),
                    task: std::mem::take(&mut current_task),
                });
            }

            let doc_num: usize = rest.trim().trim_end_matches(',').parse().unwrap_or(0);
            if doc_num > 0 && doc_num <= total_docs {
                current_doc_idx = Some(doc_num - 1); // Convert to 0-based
            } else if doc_num > 0 {
                tracing::warn!(
                    requested_doc = doc_num,
                    total_docs,
                    "Dispatch plan references out-of-range document, skipping"
                );
            }
        } else if let Some(rest) = line.strip_prefix("reason:") {
            current_reason = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("task:") {
            current_task = rest.trim().to_string();
        }
    }

    // Flush last entry
    if let Some(idx) = current_doc_idx {
        entries.push(DispatchEntry {
            doc_idx: idx,
            reason: current_reason,
            task: current_task,
        });
    }

    Some(entries)
}

/// Parse the sufficiency check response.
pub fn parse_sufficiency_response(response: &str) -> bool {
    let upper = response.trim().to_uppercase();
    upper.starts_with("SUFFICIENT") && !upper.starts_with("INSUFFICIENT")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_navigation_without_task() {
        let params = NavigationParams {
            query: "What is the revenue?",
            task: None,
            breadcrumb: "root/Financial Statements",
            evidence_summary: "- [Revenue] 200 chars",
            missing_info: "2024 comparison",
            last_feedback: "[1] Q1 Report — Q1 data (5 leaves)\n[2] Q2 Report — Q2 data (5 leaves)",
            remaining: 5,
            max_rounds: 15,
            history: "(no history yet)",
            visited_titles: "(none)",
            plan: "",
            intent_context: "",
            keyword_hints: "",
        };

        let (system, user) = worker_navigation(&params);
        assert!(system.contains("document navigation"));
        assert!(system.contains("SEARCH STRATEGY"));
        assert!(user.contains("What is the revenue?"));
        assert!(user.contains("root/Financial Statements"));
        assert!(user.contains("200 chars"));
        assert!(user.contains("2024 comparison"));
        assert!(user.contains("5/15"));
        assert!(!user.contains("sub-task"));
    }

    #[test]
    fn test_worker_navigation_with_keyword_hints() {
        let params = NavigationParams {
            query: "What is the revenue?",
            task: None,
            breadcrumb: "root",
            evidence_summary: "(none)",
            missing_info: "",
            last_feedback: "",
            remaining: 8,
            max_rounds: 15,
            history: "(no history yet)",
            visited_titles: "(none)",
            plan: "",
            intent_context: "",
            keyword_hints: "Keyword matches (use find <keyword> to jump directly):\n  - 'revenue' → root > Revenue (weight 0.85)\n",
        };

        let (_, user) = worker_navigation(&params);
        assert!(user.contains("revenue"));
        assert!(user.contains("find"));
    }

    #[test]
    fn test_worker_navigation_with_task() {
        let params = NavigationParams {
            query: "Compare 2024 and 2023 revenue",
            task: Some("Find revenue data in this document"),
            breadcrumb: "root",
            evidence_summary: "(none)",
            missing_info: "",
            last_feedback: "",
            remaining: 8,
            max_rounds: 15,
            history: "(no history yet)",
            visited_titles: "(none)",
            plan: "",
            intent_context: "analytical — comparative analysis",
            keyword_hints: "",
        };

        let (_, user) = worker_navigation(&params);
        assert!(user.contains("Find revenue data"));
        assert!(user.contains("sub-task"));
    }

    #[test]
    fn test_orchestrator_analysis() {
        let params = OrchestratorAnalysisParams {
            query: "Compare 2024 and 2023 revenue",
            doc_cards: "[1] 2024 Report\n[2] 2023 Report",
            find_results: "doc 1: keyword 'revenue' matched",
            intent_context: "\nQuery intent: analytical (complexity: moderate)",
        };

        let (system, user) = orchestrator_analysis(&params);
        assert!(system.contains("multi-document"));
        assert!(user.contains("2024 Report"));
        assert!(user.contains("revenue"));
        assert!(user.contains("analytical"));
    }

    #[test]
    fn test_worker_dispatch() {
        let params = WorkerDispatchParams {
            original_query: "Compare revenue",
            task: "Find 2024 revenue figures",
            doc_name: "2024 Annual Report",
            breadcrumb: "root",
        };

        let (system, user) = worker_dispatch(&params);
        assert!(system.contains("2024 Annual Report"));
        assert!(user.contains("Compare revenue"));
        assert!(user.contains("Find 2024 revenue"));
    }

    #[test]
    fn test_check_sufficiency() {
        let (system, user) = check_sufficiency("What is X?", "- [A] some data");
        assert!(system.contains("SUFFICIENT"));
        assert!(user.contains("What is X?"));
    }

    // --- Dispatch plan parsing ---

    #[test]
    fn test_parse_dispatch_plan_basic() {
        let output = "\
- doc: 1
  reason: Contains revenue data
  task: Find 2024 revenue figures
- doc: 2
  reason: Contains comparison data
  task: Find 2023 revenue figures";

        let entries = parse_dispatch_plan(output, 3).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].doc_idx, 0);
        assert_eq!(entries[0].task, "Find 2024 revenue figures");
        assert_eq!(entries[1].doc_idx, 1);
        assert_eq!(entries[1].reason, "Contains comparison data");
    }

    #[test]
    fn test_parse_dispatch_plan_already_answered() {
        let output = "ALREADY_ANSWERED";
        assert!(parse_dispatch_plan(output, 3).is_none());
    }

    #[test]
    fn test_parse_dispatch_plan_empty() {
        let entries = parse_dispatch_plan("no relevant documents", 3).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_dispatch_plan_out_of_range() {
        let output = "\
- doc: 99
  reason: test
  task: test";

        let entries = parse_dispatch_plan(output, 3).unwrap();
        assert!(entries.is_empty()); // doc 99 is out of range, skipped
    }

    // --- Sufficiency parsing ---

    #[test]
    fn test_parse_sufficiency_sufficient() {
        assert!(parse_sufficiency_response("SUFFICIENT - we have all data"));
        assert!(parse_sufficiency_response("Sufficient"));
    }

    #[test]
    fn test_parse_sufficiency_insufficient() {
        assert!(!parse_sufficiency_response("INSUFFICIENT - missing data"));
        assert!(!parse_sufficiency_response("Insufficient"));
    }
}
