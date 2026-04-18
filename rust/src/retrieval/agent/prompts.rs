// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Prompt templates for the retrieval agent.
//!
//! Five prompts, one per role:
//! 1. `subagent_navigation` — SubAgent nav loop, every round
//! 2. `orchestrator_analysis` — Orchestrator Phase 1
//! 3. `subagent_dispatch` — SubAgent first round (when dispatched by Orchestrator)
//! 4. `orchestrator_integration` — Orchestrator Phase 3
//! 5. `answer_synthesis` — final answer generation

// ---------------------------------------------------------------------------
// Prompt 1: SubAgent Navigation (used every round in the nav loop)
// ---------------------------------------------------------------------------

/// Parameters for the sub-agent navigation prompt.
pub struct NavigationParams<'a> {
    pub query: &'a str,
    /// Sub-task description (None when SubAgent is called directly).
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
}

pub fn subagent_navigation(params: &NavigationParams) -> (String, String) {
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
        format!("\nAlready visited (do not re-read these): {}", params.visited_titles)
    };

    let system = format!(
        "You are a document navigation assistant. You navigate inside a document to find \
         information that answers the user's question.

Available commands:
- ls                List children at current position (with summaries and leaf counts)
- cd <name>         Enter a child node (supports absolute paths like /root/Section)
- cd ..             Go back to parent node
- cat <name>        Read node content (automatically collected as evidence)
- head <name>       Preview first 20 lines of a node (does NOT collect evidence)
- find <keyword>    Search for a keyword in the document index
- findtree <pattern> Search for nodes by title pattern (case-insensitive)
- grep <pattern>    Regex search across all content in current subtree
- wc <name>         Show content size (lines, words, chars)
- pwd               Show current navigation path
- check             Evaluate if collected evidence is sufficient
- done              End navigation

Rules:
- Output exactly ONE command per response, nothing else.
- Always ls before cd — observe before descending.
- Content from cat is automatically saved as evidence — don't re-cat the same node.
- Use head to preview a node before cat to avoid collecting irrelevant large content.
- Use grep when find doesn't locate a specific term — grep searches actual content.
- Use findtree to discover nodes by name across the entire document.
- Do not cat or cd into nodes you have already visited.
- When evidence is sufficient, use check to verify, then done to finish.
- If the current branch has nothing relevant, use cd .. to go back.
- If you're at the root and no children seem relevant, use done."
    );

    let user = format!(
        "{last_feedback_section}\
User question: {query}{task_section}

Current position: /{breadcrumb}
Collected evidence:
{evidence_summary}{missing_section}{visited_section}
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
}

pub fn orchestrator_analysis(params: &OrchestratorAnalysisParams) -> (String, String) {
    let doc_cards = params.doc_cards;
    let find_results = params.find_results;
    let query = params.query;

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

User question: {query}

Relevant documents:"
    );

    (system, user)
}

// ---------------------------------------------------------------------------
// Prompt 3: SubAgent Dispatch (first-round prompt when Orchestrator dispatches)
// ---------------------------------------------------------------------------

/// Parameters for the dispatch prompt.
pub struct SubagentDispatchParams<'a> {
    pub original_query: &'a str,
    pub task: &'a str,
    pub doc_name: &'a str,
    pub breadcrumb: &'a str,
}

pub fn subagent_dispatch(params: &SubagentDispatchParams) -> (String, String) {
    let doc_name = params.doc_name;
    let original_query = params.original_query;
    let task = params.task;
    let breadcrumb = params.breadcrumb;

    let system = format!(
        "You are a document navigation assistant. You are searching inside the document \
         \"{doc_name}\" for specific information.

Available commands: ls, cd <name>, cd .., cat <name>, head <name>, find <keyword>, \
findtree <pattern>, grep <regex>, wc <name>, pwd, check, done

Rules:
- Output exactly ONE command per response.
- Always ls before cd.
- Content from cat is automatically saved as evidence.
- Use head to preview before cat for large nodes.
- Use grep to search content when find doesn't match.
- When evidence is sufficient, use check then done."
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
// Prompt 4: Orchestrator Integration (multi-doc Phase 3)
// ---------------------------------------------------------------------------

/// One sub-agent's results for the integration prompt.
pub struct SubAgentSummary<'a> {
    pub doc_name: &'a str,
    pub evidence_count: usize,
    pub evidence_text: &'a str,
    pub answer: &'a str,
}

/// Parameters for the orchestrator integration prompt.
pub struct OrchestratorIntegrationParams<'a> {
    pub query: &'a str,
    pub sub_results: &'a [SubAgentSummary<'a>],
}

pub fn orchestrator_integration(params: &OrchestratorIntegrationParams) -> (String, String) {
    let query = params.query;

    let system =
        "You are a multi-document analysis assistant. You are given evidence independently \
         collected from multiple documents. Your job is to integrate this evidence to answer \
         the user's question.

Requirements:
- Mark the source document for each piece of information.
- If different documents have conflicting data, point out the discrepancy.
- If units or measurement criteria differ, explain the difference.
- If evidence is missing for some aspect, state it clearly."
            .to_string();

    let mut evidence_sections = String::new();
    for result in params.sub_results {
        evidence_sections.push_str(&format!(
            "## Document: {} ({} evidence items)\n{}\n",
            result.doc_name, result.evidence_count, result.evidence_text
        ));
        if !result.answer.is_empty() {
            evidence_sections.push_str(&format!("Sub-answer: {}\n", result.answer));
        }
        evidence_sections.push('\n');
    }

    let user = format!(
        "User question: {query}\n\n\
         Collected evidence:\n\
         {evidence_sections}\n\
         Integrated analysis:"
    );

    (system, user)
}

// ---------------------------------------------------------------------------
// Prompt 5: Answer Synthesis
// ---------------------------------------------------------------------------

/// Parameters for the answer synthesis prompt.
pub struct SynthesisParams<'a> {
    pub query: &'a str,
    /// All evidence items, pre-formatted.
    pub evidence_text: &'a str,
    /// What information might be missing (empty if complete).
    pub missing_info: &'a str,
}

pub fn answer_synthesis(params: &SynthesisParams) -> (String, String) {
    let query = params.query;
    let evidence_text = params.evidence_text;

    let system =
        "You are an expert analyst. Based on the provided evidence, directly answer the user's \
         question. Cite the source section for each piece of information you use. \
         If the evidence is insufficient to fully answer the question, clearly state what is known \
         and what is missing."
            .to_string();

    let missing_section = if params.missing_info.is_empty() {
        String::new()
    } else {
        format!(
            "\nNote: The following information may be missing: {}",
            params.missing_info
        )
    };

    let user = format!(
        "User question: {query}\n\n\
         Evidence:\n\
         {evidence_text}{missing_section}\n\n\
         Answer:"
    );

    (system, user)
}

// ---------------------------------------------------------------------------
// Prompt 6: Check (evidence sufficiency evaluation)
// ---------------------------------------------------------------------------

/// Build the check prompt for LLM-based sufficiency evaluation.
pub fn check_sufficiency(query: &str, evidence_summary: &str) -> (String, String) {
    let system = "You evaluate whether collected evidence is sufficient to answer a question. \
         Respond with ONLY 'SUFFICIENT' or 'INSUFFICIENT' followed by a one-line reason."
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
    fn test_subagent_navigation_without_task() {
        let params = NavigationParams {
            query: "What is the revenue?",
            task: None,
            breadcrumb: "root/Financial Statements",
            evidence_summary: "- [Revenue] 200 chars",
            missing_info: "2024 comparison",
            last_feedback: "[1] Q1 Report — Q1 data (5 leaves)\n[2] Q2 Report — Q2 data (5 leaves)",
            remaining: 5,
            max_rounds: 8,
            history: "(no history yet)",
            visited_titles: "(none)",
        };

        let (system, user) = subagent_navigation(&params);
        assert!(system.contains("document navigation"));
        assert!(user.contains("What is the revenue?"));
        assert!(user.contains("root/Financial Statements"));
        assert!(user.contains("200 chars"));
        assert!(user.contains("2024 comparison"));
        assert!(user.contains("5/8"));
        assert!(!user.contains("sub-task"));
    }

    #[test]
    fn test_subagent_navigation_with_task() {
        let params = NavigationParams {
            query: "Compare 2024 and 2023 revenue",
            task: Some("Find revenue data in this document"),
            breadcrumb: "root",
            evidence_summary: "(none)",
            missing_info: "",
            last_feedback: "",
            remaining: 8,
            max_rounds: 8,
            history: "(no history yet)",
            visited_titles: "(none)",
        };

        let (_, user) = subagent_navigation(&params);
        assert!(user.contains("Find revenue data"));
        assert!(user.contains("sub-task"));
    }

    #[test]
    fn test_orchestrator_analysis() {
        let params = OrchestratorAnalysisParams {
            query: "Compare 2024 and 2023 revenue",
            doc_cards: "[1] 2024 Report\n[2] 2023 Report",
            find_results: "doc 1: keyword 'revenue' matched",
        };

        let (system, user) = orchestrator_analysis(&params);
        assert!(system.contains("multi-document"));
        assert!(user.contains("2024 Report"));
        assert!(user.contains("revenue"));
    }

    #[test]
    fn test_subagent_dispatch() {
        let params = SubagentDispatchParams {
            original_query: "Compare revenue",
            task: "Find 2024 revenue figures",
            doc_name: "2024 Annual Report",
            breadcrumb: "root",
        };

        let (system, user) = subagent_dispatch(&params);
        assert!(system.contains("2024 Annual Report"));
        assert!(user.contains("Compare revenue"));
        assert!(user.contains("Find 2024 revenue"));
    }

    #[test]
    fn test_orchestrator_integration() {
        let sub_a = SubAgentSummary {
            doc_name: "2024 Report",
            evidence_count: 2,
            evidence_text: "[Revenue] $10.2M\n[Q1] $2.5M",
            answer: "Revenue is $10.2M",
        };
        let sub_b = SubAgentSummary {
            doc_name: "2023 Report",
            evidence_count: 1,
            evidence_text: "[Net Sales] $9.8M",
            answer: "",
        };

        let params = OrchestratorIntegrationParams {
            query: "Compare revenue",
            sub_results: &[sub_a, sub_b],
        };

        let (_, user) = orchestrator_integration(&params);
        assert!(user.contains("2024 Report"));
        assert!(user.contains("2023 Report"));
        assert!(user.contains("$10.2M"));
        assert!(user.contains("$9.8M"));
    }

    #[test]
    fn test_answer_synthesis() {
        let params = SynthesisParams {
            query: "What is the revenue?",
            evidence_text: "[Revenue] $10.2M\n[Q1] $2.5M",
            missing_info: "",
        };

        let (system, user) = answer_synthesis(&params);
        assert!(system.contains("expert analyst"));
        assert!(user.contains("$10.2M"));
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
