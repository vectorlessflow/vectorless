"""Scout — a plan-once navigation agent (replacement for the greedy Worker loop).

The greedy Worker drives the document one primitive at a time, each step an LLM
round-trip (~10-15 calls/doc). The Scout collapses that into:

    1. gather candidates deterministically (0 LLM) from the compile-time
       acceleration indexes — intent routes, concept routes, keyword entries,
       and ranked evidence scores;
    2. ONE structured LLM call to pick which candidates' content answers the
       query (and, optionally, which container sections to drill into);
    3. read the chosen nodes with ``cat`` (0 LLM) → Evidence;
    4. a bounded ``expand`` fallback (default ≤2 extra picks) only when the
       model says it needs to look deeper.

Typical cost: 1 LLM call for a single document (2-3 with one expansion),
versus 10-15 for the greedy loop. It is a drop-in for ``Worker`` — same
constructor surface, same ``run() -> WorkerOutput`` contract.
"""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass

from pydantic import BaseModel, Field

from vectorless.ask.protocols import NavigableDocument
from vectorless.ask.types import Evidence, TraceStep, WorkerMetrics, WorkerOutput
from vectorless.ask.utils import extract_keywords
from vectorless.llm_client import LLMClient

logger = logging.getLogger(__name__)

# Tuning knobs ---------------------------------------------------------------
MAX_CANDIDATES = 24        # cap on candidates shown to the picker
MAX_EXPANSIONS = 2         # extra pick rounds allowed when drilling down
MAX_TARGETS_PER_PICK = 6   # safety cap on nodes read per pick
TITLE_CLAMP = 90           # max chars of a title shown to the picker


# ---------------------------------------------------------------------------
# Candidate model
# ---------------------------------------------------------------------------

@dataclass
class _Cand:
    """A candidate node the Scout may read or expand."""

    nid: str               # node id string ("n{u64}") as returned by the navigator
    title: str
    score: float
    why: str               # provenance, e.g. "intent", "kw:revenue", "score:0.82"
    hits: int = 1          # how many distinct indexes proposed this node (corroboration)

    @property
    def key(self) -> str:
        return self.nid


# ---------------------------------------------------------------------------
# Structured pick result (instructor / pydantic)
# ---------------------------------------------------------------------------

class ScoutPick(BaseModel):
    """The picker's decision over the candidate list."""

    targets: list[str] = Field(
        default_factory=list,
        description="IDs of candidate sections whose CONTENT answers the question — read these now. Prefer the fewest that fully answer.",
    )
    expand: list[str] = Field(
        default_factory=list,
        description="IDs of broad/container candidates whose children likely hold the answer — drill into these instead of reading them.",
    )
    done: bool = Field(
        default=True,
        description="True if the chosen targets fully answer the question; false if more navigation is needed.",
    )
    reasoning: str = Field(default="", description="One short sentence on the choice.")


class VerifyQuick(BaseModel):
    """Fast-path grounding check: does the already-read evidence answer the query?"""

    answers: bool = Field(
        description="True only if the evidence contains the specific information the question asks for.",
    )
    missing: str = Field(default="", description="What is missing, if anything.")


class _BeamPick(BaseModel):
    """Beam decide: from drilled-down subsections + previews, which to read fully."""

    targets: list[str] = Field(
        default_factory=list,
        description="IDs of the subsections whose full content answers the question.",
    )


# ---------------------------------------------------------------------------
# Scout
# ---------------------------------------------------------------------------

class Scout:
    """Plan-once navigation agent for a single document.

    The per-document agent for retrieval — gathers candidates from the compile-time
    acceleration indexes, makes one structured pick, reads the chosen nodes, and
    optionally drills down once or twice.
    """

    def __init__(
        self,
        document: NavigableDocument,
        query: str,
        llm_client: LLMClient,
        *,
        max_rounds: int = 15,          # accepted for Worker compat; used as a hard ceiling
        max_llm_calls: int = 0,
        task: str | None = None,
        intent_context: str = "",
        shared_context: str = "",
        max_expansions: int = MAX_EXPANSIONS,
        fast_path: bool = True,
        doc_id: str = "",
        intent: str = "",
        route_cache: object | None = None,
        use_cache: bool = True,
    ) -> None:
        self._doc = document
        self._query = query
        self._llm = llm_client
        self._task = task
        self._intent_context = intent_context
        self._shared_context = shared_context
        self._max_llm_calls = max_llm_calls
        self._max_expansions = max(0, min(max_expansions, max_rounds))
        self._fast_path = fast_path
        self._doc_id = doc_id
        self._intent = intent or "factual"
        self._cache = route_cache
        self._use_cache = use_cache and route_cache is not None and bool(doc_id)

    # -- public ------------------------------------------------------------

    async def run(self) -> WorkerOutput:
        doc = self._doc
        doc_name = await _safe(doc.doc_name(), "")

        evidence: list[Evidence] = []
        trace: list[TraceStep] = []
        collected: set[str] = set()
        llm_calls = 0
        round_no = 0

        # 1. deterministic candidate gathering (0 LLM)
        pool: dict[str, _Cand] = {}
        await self._gather(doc, pool)
        if not pool:
            await self._fallback_toc(doc, pool)

        if not pool:
            logger.info("Scout: no candidates for doc=%s — empty result", doc_name)
            return _empty_output(doc_name)

        logger.info("Scout: %d candidates gathered for doc=%s", len(pool), doc_name)

        def budget_left() -> bool:
            return not (self._max_llm_calls and llm_calls >= self._max_llm_calls)

        # 0. Route cache: a verified route for a near-identical past query → 0 LLM.
        if self._use_cache:
            cached = self._cache.lookup(self._doc_id, self._query, self._intent)
            if cached:
                for nid in cached:
                    cand = pool.get(nid)
                    if cand is None:
                        title = await _safe(doc.node_title(nid), "")
                        if not title:
                            continue
                        cand = _Cand(nid=nid, title=_clamp(title), score=1.0, why="cache")
                    await self._read(doc, cand, evidence, trace, collected, round_no)
                if evidence:
                    logger.info("Scout route-cache hit: doc=%s (0 LLM)", doc_name)
                    return self._finish(evidence, trace, collected, round_no, llm_calls, doc_name)

        # 2. Fast path (adaptive depth): when a confident deterministic route exists
        #    for a simple/factual query, read it and confirm with ONE verify call.
        if self._fast_path and budget_left():
            ranked0 = sorted(pool.values(), key=lambda c: c.score, reverse=True)[:MAX_CANDIDATES]
            top = self._confident(ranked0)
            if top is not None:
                round_no += 1
                await self._read(doc, top, evidence, trace, collected, round_no)
                if len(ranked0) > 1 and ranked0[1].score >= top.score - 0.1:
                    await self._read(doc, ranked0[1], evidence, trace, collected, round_no)
                ok = await self._verify(doc_name, evidence)
                llm_calls += 1
                if ok:
                    await self._record(collected, 0.85)  # verified route → reusable
                    logger.info("Scout fast-path hit: doc=%s (%d LLM call)", doc_name, llm_calls)
                    return self._finish(evidence, trace, collected, round_no, llm_calls, doc_name)
                # not grounded — fall through to full pick, keeping what we read

        # 3. Pick (1 LLM) → parallel read; then at most ONE beam expand (1 LLM).
        ranked = sorted(pool.values(), key=lambda c: c.score, reverse=True)[:MAX_CANDIDATES]
        pick = await self._pick(doc_name, ranked)
        llm_calls += 1
        round_no += 1

        if pick is None:
            # picker failed — deterministic fallback: read the top candidates
            await self._read_many(doc, ranked[:3], evidence, trace, collected, round_no)
            await self._record(collected, 0.6)
            return self._finish(evidence, trace, collected, round_no, llm_calls, doc_name)

        by_key = {c.key: c for c in ranked}
        targets = [by_key[str(t)] for t in pick.targets[:MAX_TARGETS_PER_PICK] if str(t) in by_key]
        await self._read_many(doc, targets, evidence, trace, collected, round_no)

        # Beam expand: drill ALL requested containers at once, preview their children
        # in parallel, then ONE decide call selects what to read fully. Bounded to +1 LLM
        # regardless of how wide the tree is (replaces the per-level pick loop).
        if not pick.done and pick.expand and self._max_expansions > 0 and budget_left():
            children = await self._beam_expand(doc, pick.expand, by_key, pool)
            if children:
                round_no += 1
                chosen_ids = await self._beam_decide(doc, doc_name, children)
                llm_calls += 1
                chosen = (
                    [pool[str(t)] for t in chosen_ids if str(t) in pool]
                    if chosen_ids else children[:3]
                )
                await self._read_many(doc, chosen, evidence, trace, collected, round_no)

        await self._record(collected, 0.6)  # unverified route — stored, not reused (< threshold)
        return self._finish(evidence, trace, collected, round_no, llm_calls, doc_name)

    # -- route cache -------------------------------------------------------

    async def _record(self, collected: set[str], confidence: float) -> None:
        if not (self._use_cache and collected):
            return
        try:
            await self._cache.record(
                self._doc_id, self._query, self._intent, list(collected), confidence,
            )
        except Exception as e:  # noqa: BLE001
            logger.warning("route cache record failed: %s", e)

    # -- fast path: confidence + verify ------------------------------------

    def _confident(self, ranked: list[_Cand]) -> _Cand | None:
        """Return the top candidate iff it is a confident answer for a simple query.

        Confident = (corroborated by ≥2 indexes OR dominant by score/margin) AND the
        query looks simple/factual. Deterministic — costs no LLM call.
        """
        if not ranked:
            return None
        top = ranked[0]
        if top.score < 0.5:
            return None
        ctx = self._intent_context.lower()
        simple = len(extract_keywords(self._query)) <= 4 or "factual" in ctx or "procedural" in ctx
        corroborated = top.hits >= 2
        if len(ranked) == 1:
            dominant = True
        else:
            dominant = top.score >= 0.8 or (top.score - ranked[1].score) >= 0.2
        return top if (simple and (corroborated or dominant)) else None

    async def _verify(self, doc_name: str, evidence: list[Evidence]) -> bool:
        """One LLM call: does the read evidence actually answer the query?"""
        if not evidence:
            return False
        text = "\n\n".join(f"[{e.node_title}]\n{e.content[:1500]}" for e in evidence)
        system = (
            "You check whether the provided evidence fully answers the question. Be strict: "
            "set answers=true only if the evidence contains the specific information requested."
        )
        user = f"Question: {self._query}\n\nEvidence:\n{text}\n\nDoes the evidence fully answer the question?"
        try:
            v = await self._llm.complete_structured(system, user, VerifyQuick)
            return bool(v.answers)
        except Exception as e:  # noqa: BLE001 — accept deterministic evidence rather than burn calls
            logger.warning("Scout fast-path verify failed: %s", e)
            return True

    def _finish(
        self,
        evidence: list[Evidence],
        trace: list[TraceStep],
        collected: set[str],
        round_no: int,
        llm_calls: int,
        doc_name: str,
    ) -> WorkerOutput:
        logger.info(
            "Scout done: doc=%s rounds=%d llm_calls=%d evidence=%d",
            doc_name, round_no, llm_calls, len(evidence),
        )
        return WorkerOutput(
            evidence=evidence,
            metrics=WorkerMetrics(
                rounds_used=round_no,
                llm_calls=llm_calls,
                nodes_visited=len(collected),
                budget_exhausted=False,
                plan_generated=True,
                check_count=0,
                evidence_chars=sum(len(e.content) for e in evidence),
            ),
            doc_name=doc_name,
            trace_steps=trace,
        )

    # -- candidate gathering ----------------------------------------------

    async def _gather(self, doc: NavigableDocument, pool: dict[str, _Cand]) -> None:
        """Populate ``pool`` from the compile-time acceleration indexes (0 LLM)."""
        keywords = extract_keywords(self._query)

        async def add(nid: object, score: float, why: str) -> None:
            if nid is None:
                return
            key = str(nid)
            title = await _safe(doc.node_title(nid), "")
            if not title:
                return
            existing = pool.get(key)
            if existing is None:
                pool[key] = _Cand(nid=key, title=_clamp(title), score=score, why=why, hits=1)
            else:
                existing.hits += 1
                if score > existing.score:
                    existing.score = score
                    existing.why = why

        # ranked full-text search (BM25) — the locate-after-understanding signal
        for i, h in enumerate(await _safe(doc.search(self._query, 12), []) or []):
            await add(getattr(h, "node_id", None), 0.9 - i * 0.03, "search")

        # intent routes (whole-doc precomputed shortcuts)
        for r in await _safe(doc.intent_routes(), []) or []:
            for t in (getattr(r, "targets", []) or [])[:3]:
                await add(getattr(t, "node_id", None), float(getattr(t, "relevance", 0.5)), "intent")

        # concept routes + keyword entries (per query keyword)
        for kw in keywords[:5]:
            for r in await _safe(doc.concept_routes(kw), []) or []:
                for t in (getattr(r, "targets", []) or [])[:3]:
                    await add(getattr(t, "node_id", None), float(getattr(t, "relevance", 0.5)), f"concept:{kw}")
            for e in (await _safe(doc.keyword_entries(kw), []) or [])[:3]:
                await add(getattr(e, "node_id", None), float(getattr(e, "weight", 0.5)), f"kw:{kw}")

        # ranked evidence scores (quality prior); only the top slice
        for s in (await _safe(doc.evidence_scores_ranked(), []) or [])[:12]:
            await add(getattr(s, "node_id", None), float(getattr(s, "composite", 0.3)), f"score:{float(getattr(s, 'composite', 0.0)):.2f}")

    async def _fallback_toc(self, doc: NavigableDocument, pool: dict[str, _Cand]) -> None:
        """No acceleration data — fall back to top-level structure."""
        entries = await _safe(doc.toc(2), []) or []
        for i, e in enumerate(entries[:MAX_CANDIDATES]):
            nid = getattr(e, "node_id", None)
            title = getattr(e, "title", "") or await _safe(doc.node_title(nid), "")
            if nid is None or not title:
                continue
            pool[str(nid)] = _Cand(nid=nid, title=_clamp(title), score=1.0 - i * 0.01, why="toc")
        if pool:
            return
        # last resort: children of root
        for i, c in enumerate(await _safe(doc.ls(), []) or []):
            nid = getattr(c, "node_id", None)
            title = getattr(c, "title", "")
            if nid is None or not title:
                continue
            pool[str(nid)] = _Cand(nid=nid, title=_clamp(title), score=1.0 - i * 0.01, why="root")

    async def _expand(self, doc: NavigableDocument, cand: _Cand, pool: dict[str, _Cand]) -> int:
        """Add a container's children to the pool. Returns count added."""
        children = []
        try:
            await doc.cd(cand.nid)
            children = await doc.ls()
        except Exception:
            return 0
        added = 0
        for i, c in enumerate(children or []):
            nid = getattr(c, "node_id", None)
            title = getattr(c, "title", "")
            if nid is None or not title:
                continue
            key = str(nid)
            if key in pool:
                continue
            pool[key] = _Cand(nid=nid, title=_clamp(title), score=cand.score - 0.01 * (i + 1), why=f"child:{cand.title[:20]}")
            added += 1
        return added

    # -- pick (the single LLM call per round) ------------------------------

    async def _pick(self, doc_name: str, cands: list[_Cand]) -> ScoutPick | None:
        # Enrich each candidate with its compile-time routing signal (what the
        # section can answer) so the picker decides from coverage, not just titles.
        routings = await asyncio.gather(*(
            _safe(self._doc.node_routing(c.nid), None) for c in cands
        ))
        lines = []
        for c, r in zip(cands, routings):
            extra = _routing_hint(r)
            lines.append(f"  {c.key}  {c.title}  · {c.why}{extra}")
        listing = "\n".join(lines)
        task_line = f"Sub-task: {self._task}\n" if self._task else ""
        ctx = ""
        if self._intent_context:
            ctx += f"Query intent: {self._intent_context}\n"
        if self._shared_context:
            ctx += f"Context from other documents:\n{self._shared_context}\n"

        system = (
            "You are a precise document navigator. You are given a question and a ranked "
            "list of candidate sections from ONE document, pre-selected by the engine's "
            "routing index. Decide which candidates' CONTENT directly answers the question. "
            "Prefer the fewest sections that fully answer it. If a candidate is a broad or "
            "container section whose children likely hold the answer, put it in `expand` "
            "instead of `targets`. Only ever use IDs from the provided list."
        )
        user = (
            f"Question: {self._query}\n"
            f"{task_line}{ctx}"
            f"Document: {doc_name}\n\n"
            f"Candidate sections (id  title  · why):\n{listing}\n\n"
            "Return targets (ids to read now), expand (container ids to drill into, optional), "
            "and done (true if the targets fully answer the question)."
        )
        try:
            return await self._llm.complete_structured(system, user, ScoutPick)
        except Exception as e:  # noqa: BLE001 — picker is best-effort; we have a fallback
            logger.warning("Scout pick failed: %s", e)
            return None

    # -- read (0 LLM) ------------------------------------------------------

    async def _read(
        self,
        doc: NavigableDocument,
        cand: _Cand,
        evidence: list[Evidence],
        trace: list[TraceStep],
        collected: set[str],
        round_no: int,
    ) -> None:
        if cand.key in collected:
            return
        content = await _safe(doc.cat(cand.nid), "")
        if not content:
            return
        collected.add(cand.key)
        path = await self._path_for(doc, cand)
        evidence.append(Evidence(source_path=path, node_title=cand.title, content=content))
        trace.append(TraceStep(
            action=f"cat {cand.key} ({cand.why})",
            observation=content[:200],
            round=round_no,
        ))

    async def _path_for(self, doc: NavigableDocument, cand: _Cand) -> str:
        anc = await _safe(doc.ancestors(cand.nid), []) or []
        parts = [getattr(a, "title", "") for a in anc]
        parts = [p for p in parts if p]
        if parts:
            return "/".join(parts)
        return cand.title

    # -- beam: parallel fetch + single decide ------------------------------

    async def _read_many(
        self,
        doc: NavigableDocument,
        cands: list[_Cand],
        evidence: list[Evidence],
        trace: list[TraceStep],
        collected: set[str],
        round_no: int,
    ) -> None:
        """Read several nodes concurrently (parallel across documents; 0 LLM)."""
        await asyncio.gather(*(
            self._read(doc, c, evidence, trace, collected, round_no) for c in cands
        ))

    async def _beam_expand(
        self,
        doc: NavigableDocument,
        expand_ids: list[str],
        by_key: dict[str, _Cand],
        pool: dict[str, _Cand],
    ) -> list[_Cand]:
        """Drill every requested container at once; return the new child candidates."""
        new: list[_Cand] = []
        for eid in expand_ids:
            cand = by_key.get(str(eid))
            if cand is None:
                continue
            before = set(pool.keys())
            await self._expand(doc, cand, pool)
            for key in pool.keys() - before:
                new.append(pool[key])
            if len(new) >= MAX_CANDIDATES:
                break
        return new[:MAX_CANDIDATES]

    async def _beam_decide(
        self, doc: NavigableDocument, doc_name: str, children: list[_Cand],
    ) -> list[str]:
        """Preview all children in parallel, then ONE LLM call picks what to read fully."""
        previews = await asyncio.gather(*(
            _safe(doc.head(c.nid, 6), "") for c in children
        ))
        listing = "\n".join(
            f"  {c.key}  {c.title}\n      {_clamp_preview(p)}"
            for c, p in zip(children, previews)
        )
        system = (
            "From the candidate subsections and their content previews, choose the IDs whose "
            "FULL content answers the question. Prefer the fewest that fully answer it. "
            "Only use IDs from the list."
        )
        user = (
            f"Question: {self._query}\n\n"
            f"Candidate subsections (id  title + preview):\n{listing}\n\n"
            "Return the IDs to read in full."
        )
        try:
            r = await self._llm.complete_structured(system, user, _BeamPick)
            return [str(t) for t in r.targets]
        except Exception as e:  # noqa: BLE001
            logger.warning("Beam decide failed: %s", e)
            return []


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

async def _safe(awaitable, default):
    """Await ``awaitable``; return ``default`` on any error (acceleration data is optional)."""
    try:
        return await awaitable
    except Exception:
        return default


def _clamp(title: str) -> str:
    title = title.strip().replace("\n", " ")
    return title if len(title) <= TITLE_CLAMP else title[:TITLE_CLAMP] + "…"


def _clamp_preview(text: str) -> str:
    text = " ".join((text or "").split())
    return text if len(text) <= 120 else text[:120] + "…"


def _routing_hint(routing: object | None) -> str:
    """Render a candidate's compile-time routing signal (questions / summary) for the picker."""
    if routing is None:
        return ""
    questions = list(getattr(routing, "questions", None) or [])
    if questions:
        return "  ⟨answers: " + "; ".join(q for q in questions[:2] if q) + "⟩"
    summary = (getattr(routing, "summary", "") or "").strip()
    if summary:
        return "  ⟨" + _clamp_preview(summary) + "⟩"
    return ""


def _empty_output(doc_name: str) -> WorkerOutput:
    return WorkerOutput(evidence=[], metrics=WorkerMetrics(), doc_name=doc_name, trace_steps=[])
