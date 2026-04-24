"""Prompt templates for query reasoning stages."""

from __future__ import annotations


def stage1_classify_prompt(query: str, keywords: list[str]) -> tuple[str, str]:
    """Stage 1: Classify intent, complexity, and decompose into sub-queries.

    Returns (system, user) prompt pair.
    """
    system = (
        'You are a query analysis engine. Analyze the user\'s query and respond with a JSON object:\n'
        '\n'
        '{\n'
        '  "intent": one of "factual", "analytical", "navigational", "summary", '
        '"comparative", "procedural",\n'
        '  "complexity": one of "simple", "moderate", "complex",\n'
        '  "key_concepts": array of main concepts/entities (distinct from keywords),\n'
        '  "rewritten": optional rewritten version of the query for better retrieval '
        '(null if not needed),\n'
        '  "sub_queries": array of sub-query strings if decomposable (empty if not)\n'
        '}\n'
        '\n'
        'Intent guidelines:\n'
        '- "factual": looking for specific facts or definitions\n'
        '- "analytical": requires analysis, comparison, or evaluation\n'
        '- "navigational": looking for where to find something\n'
        '- "summary": wants a summary or overview\n'
        '- "comparative": explicit cross-reference between items\n'
        '- "procedural": how-to or step-by-step instructions\n'
        '\n'
        'Complexity guidelines:\n'
        '- "simple": single concept, direct answer expected\n'
        '- "moderate": multi-concept, requires some synthesis\n'
        '- "complex": requires multi-step reasoning, cross-referencing\n'
        '\n'
        'Respond with ONLY the JSON object, no additional text.'
    )

    user = f"Query: {query}\nExtracted keywords: [{', '.join(keywords)}]"
    return system, user


def stage2_deep_analysis_prompt(
    query: str,
    stage1_result: dict,
) -> tuple[str, str]:
    """Stage 2: Deep analysis — entities, ambiguities, temporal constraints.

    Returns (system, user) prompt pair.
    """
    import json

    system = (
        'You are a deep query analysis engine. Given a query and initial classification, '
        'perform deep analysis and respond with a JSON object:\n'
        '\n'
        '{\n'
        '  "entities": [\n'
        '    {"name": "...", "type": "person|org|product|concept", '
        '"aliases": [...], "definition_hint": "..."}\n'
        '  ],\n'
        '  "ambiguities": [\n'
        '    {"type": "lexical|scope|reference|temporal", '
        '"description": "...", "interpretations": [...], "resolution_query": "..."}\n'
        '  ],\n'
        '  "temporal_constraints": [\n'
        '    {"raw": "...", "resolved": "... or null", "is_relative": true/false}\n'
        '  ],\n'
        '  "key_concepts": ["concept1", "concept2"]\n'
        '}\n'
        '\n'
        'Entity guidelines:\n'
        '- Extract named entities (people, organizations, products, technical terms)\n'
        '- Include common aliases or abbreviations\n'
        '- definition_hint should be a brief description\n'
        '\n'
        'Ambiguity guidelines:\n'
        '- Only flag genuine ambiguities that affect retrieval\n'
        '- "lexical": word has multiple meanings\n'
        '- "scope": unclear what scope/level of detail is wanted\n'
        '- "reference": unclear what a pronoun/reference points to\n'
        '- "temporal": unclear time period\n'
        '\n'
        'Respond with ONLY the JSON object.'
    )

    user = (
        f"Query: {query}\n"
        f"Initial classification: {json.dumps(stage1_result, ensure_ascii=False)}"
    )
    return system, user


def stage3_strategy_prompt(
    query: str,
    stage1_result: dict,
    stage2_result: dict,
) -> tuple[str, str]:
    """Stage 3: Strategy formation — how to retrieve.

    Returns (system, user) prompt pair.
    """
    import json

    system = (
        'You are a retrieval strategy planner. Given a query, its analysis, and entity information, '
        'formulate a retrieval strategy and respond with a JSON object:\n'
        '\n'
        '{\n'
        '  "strategy_type": "focused|exploratory|comparative|summary",\n'
        '  "sub_strategies": ["strategy1", "strategy2"],\n'
        '  "target_sections": ["likely section titles or topics to look for"],\n'
        '  "requires_cross_doc": true/false,\n'
        '  "estimated_depth": "shallow|medium|deep"\n'
        '}\n'
        '\n'
        'Strategy guidelines:\n'
        '- "focused": single topic, targeted retrieval\n'
        '- "exploratory": broad scan needed, multiple angles\n'
        '- "comparative": cross-reference between items/documents\n'
        '- "summary": aggregate information from multiple sources\n'
        '\n'
        'sub_strategies are specific approaches within the main strategy.\n'
        'target_sections are hints about what document sections to look for.\n'
        '\n'
        'Respond with ONLY the JSON object.'
    )

    user = (
        f"Query: {query}\n"
        f"Classification: {json.dumps(stage1_result, ensure_ascii=False)}\n"
        f"Deep analysis: {json.dumps(stage2_result, ensure_ascii=False)}"
    )
    return system, user


def re_analyze_strategy_prompt(
    query: str,
    current_analysis: dict,
    gaps: list[str],
    evidence_summary: str,
) -> tuple[str, str]:
    """Re-analyze: update strategy based on verification gaps.

    Only runs Stage 3 (strategy update) with gap context.
    Returns (system, user) prompt pair.
    """
    import json

    system = (
        'You are a retrieval strategy planner. The current retrieval strategy has gaps. '
        'Given the original query, current analysis, identified gaps in evidence, and '
        'a summary of evidence collected so far, update the retrieval strategy.\n'
        '\n'
        'Respond with a JSON object:\n'
        '{\n'
        '  "strategy_type": "focused|exploratory|comparative|summary",\n'
        '  "sub_strategies": ["strategy1", "strategy2"],\n'
        '  "target_sections": ["sections to look for based on gaps"],\n'
        '  "requires_cross_doc": true/false,\n'
        '  "estimated_depth": "shallow|medium|deep"\n'
        '}\n'
        '\n'
        'Focus the updated strategy on addressing the identified gaps.\n'
        'Respond with ONLY the JSON object.'
    )

    gaps_text = "\n".join(f"- {g}" for g in gaps)
    user = (
        f"Query: {query}\n"
        f"Current analysis: {json.dumps(current_analysis, ensure_ascii=False)}\n"
        f"\nEvidence gaps:\n{gaps_text}\n"
        f"\nEvidence summary:\n{evidence_summary}"
    )
    return system, user
