"""Verification prompt templates."""

from __future__ import annotations


def verify_prompt(
    query: str,
    evidence_text: str,
    query_intent: str,
    iteration: int,
) -> tuple[str, str]:
    """Build the verification prompt — single combined LLM call for all 4 dimensions.

    Returns (system, user) prompt pair.
    """
    system = (
        "You are an evidence verification engine. Assess whether the collected evidence "
        "can answer the user's question across four dimensions. Respond with a JSON object:\n"
        "\n"
        "{\n"
        '  "dimensions": {\n'
        '    "factual_accuracy": {\n'
        '      "score": 0.0-1.0,\n'
        '      "reasoning": "...",\n'
        '      "evidence_refs": ["doc_name/node_title", "doc_name/node_title"]\n'
        "    },\n"
        '    "completeness": {\n'
        '      "score": 0.0-1.0,\n'
        '      "reasoning": "...",\n'
        '      "evidence_refs": ["doc_name/node_title"]\n'
        "    },\n"
        '    "relevance": {\n'
        '      "score": 0.0-1.0,\n'
        '      "reasoning": "...",\n'
        '      "evidence_refs": ["doc_name/node_title"]\n'
        "    },\n"
        '    "coherence": {\n'
        '      "score": 0.0-1.0,\n'
        '      "reasoning": "...",\n'
        '      "evidence_refs": ["doc_name/node_title"]\n'
        "    }\n"
        "  },\n"
        '  "passed": true/false,\n'
        '  "overall_confidence": 0.0-1.0,\n'
        '  "gaps": ["specific gap 1", "specific gap 2"],\n'
        '  "re_retrieval_hints": ["what to search for next"]\n'
        "}\n"
        "\n"
        "Dimension guidelines:\n"
        "- factual_accuracy: Do the evidence texts support the claims needed to answer "
        "the question? Are facts verifiable from the evidence?\n"
        "- completeness: Does the evidence cover ALL aspects of the question? "
        "If the question has multiple parts, are all addressed?\n"
        "- relevance: Is the evidence directly on-topic for the question?\n"
        "- coherence: Can the evidence pieces be logically assembled into an answer?\n"
        "\n"
        "Scoring guidelines:\n"
        "- 0.0-0.3: Severely lacking\n"
        "- 0.3-0.5: Partially addressed\n"
        "- 0.5-0.7: Adequately addressed\n"
        "- 0.7-0.9: Well addressed\n"
        "- 0.9-1.0: Comprehensively addressed\n"
        "\n"
        'Set "passed" to true ONLY if all dimension scores are >= 0.5.\n'
        '"gaps" should list specific information still missing.\n'
        '"re_retrieval_hints" should describe what additional searches would help.\n'
        "\n"
        "Respond with ONLY the JSON object."
    )

    iteration_note = f"\n(This is verification iteration {iteration + 1})" if iteration > 0 else ""

    user = (
        f"Question: {query}\n"
        f"Query intent: {query_intent}{iteration_note}\n\n"
        f"Collected evidence:\n"
        f"{evidence_text}\n\n"
        f"Verify the evidence."
    )

    return system, user
