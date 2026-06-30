"""Adaptive route cache — turn repeated/near-repeated questions into 0-LLM lookups.

A successful, *verified* Scout route (the set of node ids that answered a query)
is recorded against a signature of the query: ``intent + normalized keyword set``.
A later query on the same document whose keyword set overlaps strongly (Jaccard)
with a high-confidence stored route reuses those nodes directly — skipping the
LLM pick entirely.

The cache is self-improving (it grows from real traffic) and persistent (a JSON
file alongside the user's cache dir). It is deterministic and uses no model.

Disable with ``VECTORLESS_ROUTE_CACHE=0``; relocate with ``VECTORLESS_CACHE_DIR``.
"""

from __future__ import annotations

import asyncio
import json
import logging
import os
import tempfile
from pathlib import Path

from vectorless.ask.utils import extract_keywords

logger = logging.getLogger(__name__)

# Only reuse routes that overlap strongly and were recorded with high confidence.
LOOKUP_JACCARD = 0.8
LOOKUP_MIN_CONFIDENCE = 0.7
MAX_ENTRIES_PER_DOC = 200


def _norm_keywords(query: str) -> list[str]:
    """Lowercased, de-duplicated, sorted keyword set for a query (order-independent)."""
    return sorted({k.lower() for k in extract_keywords(query) if k})


def _jaccard(a: set[str], b: set[str]) -> float:
    if not a and not b:
        return 1.0
    if not a or not b:
        return 0.0
    return len(a & b) / len(a | b)


class RouteCache:
    """Persistent, per-document route cache keyed by (intent, keyword set)."""

    def __init__(self, path: str | os.PathLike) -> None:
        self._path = Path(path)
        self._data: dict[str, list[dict]] = {}
        self._lock = asyncio.Lock()
        self._load()

    @classmethod
    def default(cls) -> "RouteCache | None":
        """Build the default on-disk cache, or None if disabled by env."""
        if os.getenv("VECTORLESS_ROUTE_CACHE", "1").lower() in ("0", "false", "no", "off"):
            return None
        base = os.getenv("VECTORLESS_CACHE_DIR") or os.path.join(
            os.path.expanduser("~"), ".cache", "vectorless"
        )
        try:
            return cls(os.path.join(base, "route_cache.json"))
        except Exception as e:  # noqa: BLE001
            logger.warning("route cache init failed: %s", e)
            return None

    # -- read -------------------------------------------------------------

    def lookup(self, doc_id: str, query: str, intent: str) -> list[str] | None:
        """Return node ids of the best matching high-confidence route, or None."""
        entries = self._data.get(doc_id) or []
        if not entries:
            return None
        kw = set(_norm_keywords(query))
        if not kw:
            return None
        best: dict | None = None
        best_j = 0.0
        for e in entries:
            if e.get("intent") != intent:
                continue
            if float(e.get("confidence", 0.0)) < LOOKUP_MIN_CONFIDENCE:
                continue
            j = _jaccard(kw, set(e.get("keywords", [])))
            if j > best_j:
                best_j, best = j, e
        if best is not None and best_j >= LOOKUP_JACCARD:
            node_ids = [str(n) for n in best.get("node_ids", []) if n]
            return node_ids or None
        return None

    # -- write ------------------------------------------------------------

    async def record(
        self,
        doc_id: str,
        query: str,
        intent: str,
        node_ids: list[str],
        confidence: float,
    ) -> None:
        """Upsert a route. Merges into an existing identical (intent, keywords) entry."""
        if not node_ids:
            return
        kw = _norm_keywords(query)
        if not kw:
            return
        async with self._lock:
            entries = self._data.setdefault(doc_id, [])
            kw_set = set(kw)
            for e in entries:
                if e.get("intent") == intent and set(e.get("keywords", [])) == kw_set:
                    if confidence >= float(e.get("confidence", 0.0)):
                        e["node_ids"] = list(node_ids)
                        e["confidence"] = confidence
                    e["hits"] = int(e.get("hits", 0)) + 1
                    break
            else:
                entries.append({
                    "intent": intent,
                    "keywords": kw,
                    "node_ids": list(node_ids),
                    "confidence": confidence,
                    "hits": 1,
                })
            if len(entries) > MAX_ENTRIES_PER_DOC:
                entries.sort(key=lambda x: int(x.get("hits", 0)))
                del entries[: len(entries) - MAX_ENTRIES_PER_DOC]
            self._save()

    # -- persistence ------------------------------------------------------

    def _load(self) -> None:
        try:
            self._data = json.loads(self._path.read_text(encoding="utf-8"))
            if not isinstance(self._data, dict):
                self._data = {}
        except FileNotFoundError:
            self._data = {}
        except Exception as e:  # noqa: BLE001
            logger.warning("route cache load failed (%s) — starting empty", e)
            self._data = {}

    def _save(self) -> None:
        try:
            self._path.parent.mkdir(parents=True, exist_ok=True)
            fd, tmp = tempfile.mkstemp(dir=str(self._path.parent), suffix=".tmp")
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                json.dump(self._data, f)
            os.replace(tmp, self._path)
        except Exception as e:  # noqa: BLE001
            logger.warning("route cache save failed: %s", e)
