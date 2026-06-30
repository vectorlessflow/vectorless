"""Deterministic tests for evidence consolidation (no LLM)."""

from vectorless.consolidate import ConsolidatedOutput, consolidate, dedup, format_answer
from vectorless.ask.types import Evidence
from vectorless.ask.reasoning.types import QueryIntent


def _ev(content: str, path: str = "p", title: str = "t", doc: str = "d") -> Evidence:
    return Evidence(source_path=path, node_title=title, content=content, doc_name=doc)


# ── dedup ──────────────────────────────────────────────────────────────────

def test_quality_filter_drops_short_evidence():
    short = _ev("too short")          # < 50 chars
    long = _ev("x" * 60)
    out = dedup([short, long])
    assert len(out) == 1
    assert out[0].content == long.content


def test_source_dedup_keeps_first_per_key():
    a = _ev("A" * 60, path="s1", doc="d1")
    b = _ev("B" * 60, path="s1", doc="d1")   # same (doc, path) → duplicate
    c = _ev("C" * 60, path="s2", doc="d1")
    out = dedup([a, b, c])
    assert len(out) == 2
    assert {e.source_path for e in out} == {"s1", "s2"}


def test_jaccard_removes_near_duplicate_content():
    base = "the quarterly revenue grew strongly across all regions this fiscal year"
    a = _ev(base, path="s1")
    b = _ev(base + " indeed", path="s2")     # ~0.92 Jaccard → near-duplicate
    out = dedup([a, b])
    assert len(out) == 1


# ── format ─────────────────────────────────────────────────────────────────

def test_format_answer_factual_includes_attribution():
    s = format_answer([_ev("revenue is 5", title="Income", doc="Report")], QueryIntent.FACTUAL)
    assert "Income" in s and "Report" in s and "revenue is 5" in s


def test_format_answer_navigational_lists_locations():
    s = format_answer(
        [_ev("x" * 60, title="Risk", doc="Report", path="root/Risk")],
        QueryIntent.NAVIGATIONAL,
    )
    assert "Found at" in s and "Risk" in s and "root/Risk" in s


# ── consolidate ─────────────────────────────────────────────────────────────

def test_consolidate_empty():
    out = consolidate([])
    assert isinstance(out, ConsolidatedOutput)
    assert out.answer == "" and out.evidence == [] and out.llm_calls == 0


def test_consolidate_passes_through_confidence_and_is_llm_free():
    out = consolidate([_ev("x" * 60)], QueryIntent.FACTUAL, confidence=0.42)
    assert out.confidence == 0.42
    assert len(out.evidence) == 1
    assert out.llm_calls == 0
