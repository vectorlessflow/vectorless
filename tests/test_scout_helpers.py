"""Deterministic tests for Scout pure-logic helpers (no LLM, no document)."""

from vectorless.ask.scout import Scout, _Cand, _clamp, _clamp_preview, _routing_hint


def _scout(query: str, intent_context: str = "") -> Scout:
    # document/llm are unused by the pure helpers under test.
    return Scout(document=None, query=query, llm_client=None, intent_context=intent_context)


# ── _confident (fast-path gate) ──────────────────────────────────────────────

def test_confident_single_candidate():
    s = _scout("revenue")
    c = _Cand(nid="n1", title="Rev", score=0.9, why="search")
    assert s._confident([c]) is c


def test_confident_corroborated_simple_query():
    s = _scout("total revenue")  # <=4 keywords → simple
    top = _Cand(nid="n1", title="Rev", score=0.7, why="search", hits=2)  # corroborated
    second = _Cand(nid="n2", title="Other", score=0.69, why="kw")
    assert s._confident([top, second]) is top


def test_confident_dominant_by_margin():
    s = _scout("revenue")
    top = _Cand(nid="n1", title="Rev", score=0.85, why="search")  # >=0.8 dominant
    second = _Cand(nid="n2", title="Other", score=0.6, why="kw")
    assert s._confident([top, second]) is top


def test_not_confident_when_complex_and_uncorroborated():
    s = _scout(
        "a long complex multi part analytical question about many separate things",
        intent_context="analytical",
    )
    top = _Cand(nid="n1", title="x", score=0.55, why="search", hits=1)
    second = _Cand(nid="n2", title="y", score=0.54, why="kw")  # margin 0.01, not dominant
    assert s._confident([top, second]) is None


def test_not_confident_when_score_too_low():
    s = _scout("revenue")
    assert s._confident([_Cand(nid="n1", title="x", score=0.4, why="s")]) is None


def test_confident_empty():
    assert _scout("revenue")._confident([]) is None


# ── _routing_hint ────────────────────────────────────────────────────────────

def test_routing_hint_prefers_questions():
    c = _Cand(nid="n1", title="x", score=1.0, why="s",
              questions=["What is revenue?", "How much profit?"])
    h = _routing_hint(c)
    assert "answers:" in h and "What is revenue?" in h


def test_routing_hint_falls_back_to_summary():
    c = _Cand(nid="n1", title="x", score=1.0, why="s", summary="A section about revenue.")
    h = _routing_hint(c)
    assert "revenue" in h and "answers" not in h


def test_routing_hint_empty_when_no_signal():
    assert _routing_hint(_Cand(nid="n1", title="x", score=1.0, why="s")) == ""


# ── clamps ───────────────────────────────────────────────────────────────────

def test_clamp_trims_and_truncates():
    assert _clamp("  hi  ") == "hi"
    long = _clamp("a" * 100)
    assert long.endswith("…") and len(long) <= 91


def test_clamp_preview_collapses_whitespace():
    assert _clamp_preview("  multi   line\n text ") == "multi line text"
