"""Deterministic tests for the adaptive route cache (no LLM)."""

import pytest

from vectorless.ask.route_cache import RouteCache, _jaccard, _norm_keywords


# ── pure helpers ─────────────────────────────────────────────────────────────

def test_norm_keywords_is_order_and_case_independent():
    assert _norm_keywords("Revenue Q1 Growth") == _norm_keywords("growth  q1 revenue")


def test_jaccard():
    assert _jaccard(set(), set()) == 1.0
    assert _jaccard({"a"}, set()) == 0.0
    assert _jaccard({"a", "b"}, {"a", "b"}) == 1.0
    assert abs(_jaccard({"a", "b", "c"}, {"a", "b"}) - 2 / 3) < 1e-9


# ── lookup / record ──────────────────────────────────────────────────────────

@pytest.mark.asyncio
async def test_record_then_lookup_hit(tmp_path):
    c = RouteCache(tmp_path / "rc.json")
    await c.record("doc1", "total revenue 2025", "factual", ["n4", "n7"], confidence=0.85)
    assert c.lookup("doc1", "total revenue 2025", "factual") == ["n4", "n7"]


@pytest.mark.asyncio
async def test_lookup_miss_below_confidence_threshold(tmp_path):
    c = RouteCache(tmp_path / "rc.json")
    # 0.6 < LOOKUP_MIN_CONFIDENCE (0.7) → stored but not reusable
    await c.record("doc1", "total revenue", "factual", ["n4"], confidence=0.6)
    assert c.lookup("doc1", "total revenue", "factual") is None


@pytest.mark.asyncio
async def test_lookup_miss_on_intent_mismatch(tmp_path):
    c = RouteCache(tmp_path / "rc.json")
    await c.record("doc1", "total revenue", "factual", ["n4"], confidence=0.85)
    assert c.lookup("doc1", "total revenue", "navigational") is None


@pytest.mark.asyncio
async def test_lookup_miss_on_unknown_doc(tmp_path):
    c = RouteCache(tmp_path / "rc.json")
    await c.record("doc1", "total revenue", "factual", ["n4"], confidence=0.85)
    assert c.lookup("other-doc", "total revenue", "factual") is None


@pytest.mark.asyncio
async def test_record_merges_and_upgrades_confidence(tmp_path):
    c = RouteCache(tmp_path / "rc.json")
    await c.record("doc1", "total revenue", "factual", ["n4"], confidence=0.6)
    assert c.lookup("doc1", "total revenue", "factual") is None       # low conf
    await c.record("doc1", "total revenue", "factual", ["n9"], confidence=0.85)
    assert c.lookup("doc1", "total revenue", "factual") == ["n9"]      # upgraded + new ids


@pytest.mark.asyncio
async def test_persistence_across_instances(tmp_path):
    path = tmp_path / "rc.json"
    c = RouteCache(path)
    await c.record("doc1", "total revenue", "factual", ["n4"], confidence=0.85)
    reloaded = RouteCache(path)
    assert reloaded.lookup("doc1", "total revenue", "factual") == ["n4"]
