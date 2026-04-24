"""Shared Blackboard — Worker-to-Worker information sharing.

The SharedBlackboard enables Workers operating on different documents to
share discoveries via Orchestrator-mediated context. Workers write
discoveries; the Orchestrator formats them for subsequent Workers.
"""

from __future__ import annotations

from dataclasses import dataclass, field


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

    Converts Worker evidence into Discovery objects based on
    evidence content and source paths.
    """
    discoveries: list[Discovery] = []

    for evidence in worker_output.evidence:
        # Every evidence item is a potential cross-reference
        discoveries.append(Discovery(
            worker_id=doc_name,
            doc_name=doc_name,
            node_title=evidence.node_title,
            finding_type="evidence",
            summary=f"Found: {evidence.node_title} ({len(evidence.content)} chars)",
        ))

    return discoveries
