"""Shared Blackboard — Worker-to-Worker information sharing.

The SharedBlackboard enables Workers operating on different documents to
share discoveries via Orchestrator-mediated context. Workers write
discoveries; the Orchestrator formats them for subsequent Workers.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger(__name__)


@dataclass
class Discovery:
    """A finding from a Worker that may be relevant to other Workers."""
    worker_id: str           # Doc name of the Worker that found this
    doc_name: str
    node_title: str
    finding_type: str        # "evidence", "lead", "dead_end", "cross_ref"
    summary: str
    relevance_to: list[str] = field(default_factory=list)  # Other doc_names


@dataclass
class SharedBlackboard:
    """Accumulates discoveries across Workers for cross-document context.

    Usage::

        blackboard = SharedBlackboard()
        blackboard.add_discovery(Discovery(...))
        context = blackboard.format_for_worker("doc_B.md")
    """

    discoveries: list[Discovery] = field(default_factory=list)
    cross_references: dict[str, list[str]] = field(default_factory=dict)
    dead_ends: set[str] = field(default_factory=set)
    active_leads: list[str] = field(default_factory=list)

    def add_discovery(self, discovery: Discovery) -> None:
        """Add a discovery to the blackboard."""
        self.discoveries.append(discovery)

        # Update cross-references
        if discovery.finding_type == "cross_ref" and discovery.relevance_to:
            src = discovery.doc_name
            if src not in self.cross_references:
                self.cross_references[src] = []
            self.cross_references[src].extend(discovery.relevance_to)

        # Track leads
        if discovery.finding_type == "lead" and discovery.summary:
            if discovery.summary not in self.active_leads:
                self.active_leads.append(discovery.summary)

        # Track dead ends
        if discovery.finding_type == "dead_end":
            self.dead_ends.add(f"{discovery.doc_name}/{discovery.node_title}")

    def format_for_worker(self, worker_doc_name: str) -> str:
        """Format a read-only view of the blackboard for a specific Worker.

        Includes discoveries from OTHER documents that are relevant.
        Excludes the Worker's own previous discoveries.
        """
        relevant = [
            d for d in self.discoveries
            if d.doc_name != worker_doc_name
        ]

        if not relevant and not self.active_leads:
            return ""

        parts: list[str] = []

        # Discoveries from other Workers
        if relevant:
            parts.append("Other Workers have found information in related documents:")
            for d in relevant[:10]:
                parts.append(
                    f"  - [{d.doc_name}] {d.finding_type}: {d.summary}"
                    f"{' (relevant to: ' + ', '.join(d.relevance_to) + ')' if d.relevance_to else ''}"
                )

        # Active leads
        if self.active_leads:
            parts.append("\nActive leads to investigate:")
            for lead in self.active_leads[:5]:
                parts.append(f"  - {lead}")

        # Dead ends
        if self.dead_ends:
            dead_end_list = ", ".join(sorted(self.dead_ends)[:5])
            parts.append(f"\nDead ends (avoid): {dead_end_list}")

        return "\n".join(parts)

    def format_for_all(self) -> str:
        """Format the full blackboard for all Workers in parallel dispatch."""
        if not self.discoveries and not self.active_leads:
            return ""
        return self.format_for_worker("")


def extract_discoveries(worker_output, doc_name: str) -> list[Discovery]:
    """Extract discoveries from a WorkerOutput for the blackboard.

    Converts Worker evidence into Discovery objects:
    - "evidence": direct evidence findings
    - "cross_ref": evidence mentioning other documents by name
    - "dead_end": nodes visited but no evidence collected (trace steps with empty results)
    """
    discoveries: list[Discovery] = []
    evidence_docs_referenced: set[str] = set()

    for evidence in worker_output.evidence:
        # Check if evidence content references other documents
        referenced_docs: list[str] = []
        if evidence.content:
            import re
            doc_refs = re.findall(
                r'(?:see|refer to|in|from|document(?:ed)? in)\s+["\']?([\w\-\.]+\.(?:md|txt|pdf|doc))["\']?',
                evidence.content,
                re.IGNORECASE,
            )
            if doc_refs:
                referenced_docs = doc_refs
                evidence_docs_referenced.update(doc_refs)

        finding_type = "cross_ref" if referenced_docs else "evidence"
        discoveries.append(Discovery(
            worker_id=doc_name,
            doc_name=doc_name,
            node_title=evidence.node_title,
            finding_type=finding_type,
            summary=f"Found: {evidence.node_title} ({len(evidence.content)} chars)",
            relevance_to=referenced_docs,
        ))

    # Generate "lead" discoveries from cross-references found in evidence
    if evidence_docs_referenced:
        lead_docs = sorted(evidence_docs_referenced)
        discoveries.append(Discovery(
            worker_id=doc_name,
            doc_name=doc_name,
            node_title="cross_document_leads",
            finding_type="lead",
            summary=f"Evidence references other documents: {', '.join(lead_docs[:5])}",
            relevance_to=lead_docs,
        ))

    return discoveries


async def extract_llm_insights(
    worker_output,
    doc_name: str,
    query: str,
    llm: Any,
) -> list[Discovery]:
    """Use LLM to extract cross-document insights from Worker output.

    More sophisticated than regex-based ``extract_discoveries``: asks the LLM
    to identify findings that might be relevant to other documents being
    searched in the same query.

    Cost: 1 additional LLM call per Worker.
    """
    from vectorless.ask.types import WorkerOutput as WO

    if not worker_output.evidence:
        return []

    # Build a condensed summary of what the Worker found
    evidence_parts: list[str] = []
    for ev in worker_output.evidence[:8]:
        preview = ev.content[:300] + "..." if len(ev.content) > 300 else ev.content
        evidence_parts.append(f"[{ev.node_title}]: {preview}")
    evidence_summary = "\n\n".join(evidence_parts)

    system = (
        "You analyze search results from a document and identify findings that might be "
        "relevant when searching OTHER documents for the same question. "
        "Focus on cross-references, shared concepts, and information gaps.\n\n"
        "Respond with a JSON array of objects, each with:\n"
        '- "summary": brief description of the finding (one sentence)\n'
        '- "finding_type": one of "lead", "cross_ref", "evidence"\n'
        '- "relevance_to": list of document names or topics this relates to\n\n'
        "If nothing is relevant across documents, respond with an empty array: []"
    )
    user = (
        f"Query: {query}\n"
        f"Document: {doc_name}\n\n"
        f"Evidence collected:\n{evidence_summary}"
    )

    try:
        from vectorless.ask.utils import parse_json_response
        response = await llm.complete(system, user)
        items = parse_json_response(response)
        if not isinstance(items, list):
            return []
    except Exception as e:
        logger.warning("LLM insight extraction failed for %s: %s", doc_name, e)
        return []

    discoveries: list[Discovery] = []
    for item in items[:5]:
        if not isinstance(item, dict):
            continue
        summary = str(item.get("summary", "")).strip()
        if not summary:
            continue
        finding_type = str(item.get("finding_type", "lead"))
        relevance_to = [str(r) for r in item.get("relevance_to", []) if r]
        discoveries.append(Discovery(
            worker_id=doc_name,
            doc_name=doc_name,
            node_title="llm_insight",
            finding_type=finding_type,
            summary=summary,
            relevance_to=relevance_to,
        ))

    return discoveries
