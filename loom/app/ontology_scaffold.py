#!/usr/bin/env python3
"""Ontology context scaffold for local LLM benchmarking (PRD-020 recipe).

Recipe: link -> seed -> expand -> serialise -> budget-clamp.

Given a prompt, this module links prompt terms to classes in a prebuilt
scaffold-index.json (see SCHEMA below), seeds the best-matching classes,
expands one hop through the taxonomy (sup/isup) and typed relations,
serialises a compact ``[ONTOLOGY CONTEXT]`` text block, and clamps it to a
token budget (approx tokens = chars / 4).

STDLIB ONLY. No third-party imports. Python 3.10+.

Bench usage
-----------
    from ontology_scaffold import scaffold_messages
    messages = scaffold_messages(messages)  # engage scaffold

``scaffold_messages`` takes an OpenAI-style chat ``messages`` list, builds a
scaffold from the LAST user message, and returns a new list with the scaffold
merged into the existing system message (or a new system message inserted at
position 0). If nothing in the ontology matches, the messages are returned
unchanged — the caller falls back to the raw prompt, so it is always safe to
call unconditionally in an A/B loop.

Index location: env ``ONTOLOGY_INDEX``, default
``~/githubs/llm-server/ontology/data/scaffold-index.json``.

CLI
---
    python3 ontology_scaffold.py "some prompt" [--budget 1500]
    python3 ontology_scaffold.py --stats
    python3 ontology_scaffold.py --selftest   # runs against an inline fixture

SCHEMA (scaffold-index.json, version 1)
---------------------------------------
    {
      "version": 1,
      "generated": "<ISO8601>",
      "counts": {"classes": <int>},
      "classes": {
        "<slug>": {
          "t": "<Title>", "d": "<definition <=400 chars>",
          "dom": "<domain|''>", "q": <float|null>, "m": "<maturity|''>",
          "sup": ["<parent slug>", ...], "isup": ["<ancestor slug>", ...],
          "rel": {"hasPart": [...], "requires": [...], ...},   # empty lists omitted
          "bl": ["<backlink slug>", ...]                        # max 20
        }
      }
    }

Slugs are kebab-case; ref IRIs look like ``urn:ngm:class:<slug>`` and the slug
is the segment after the last ``:``.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
from typing import Any, Iterable, Optional

__all__ = ["ScaffoldIndex", "scaffold", "scaffold_messages", "slugify", "get_index"]

DEFAULT_INDEX_PATH = "~/githubs/llm-server/ontology/data/scaffold-index.json"
ENV_VAR = "ONTOLOGY_INDEX"

# --- tuning knobs -----------------------------------------------------------
MIN_SEED_SCORE = 2.0          # below this, class is not a seed
EXACT_TITLE_WEIGHT = 8.0      # per word of an exactly-matched title n-gram
OVERLAP_WEIGHT = 2.0          # full title-word coverage earns this in total
SUBSTRING_WEIGHT = 0.75       # prompt term is a substring of the slug
SUBSTRING_MIN_LEN = 5         # shorter terms are too noisy for substring match
MAX_NGRAM = 4                 # longest phrase tried for exact title match
ISUP_CAP = 5                  # inferred ancestors listed per seed
REL_CAP = 3                   # relation targets listed per relation type
NEIGHBOUR_DEFS = 2            # 1-hop neighbour definitions per seed (hops>=1)
NEIGHBOUR_DEF_CHARS = 220     # neighbour one-liner definition truncation

HEADER = "[ONTOLOGY CONTEXT]"
FOOTER = "[END ONTOLOGY CONTEXT]"

SYSTEM_PREAMBLE = (
    "The following ontology context was retrieved from a curated knowledge "
    "graph. Where it is relevant to the user's request, treat it as ground "
    "truth for definitions and relationships between the concepts it covers. "
    "Where it is not relevant, ignore it and answer normally."
)

# Deterministic relation ordering for serialisation and neighbour picking.
REL_ORDER = (
    "hasPart", "requires", "enables", "dependsOn", "implements", "uses",
    "partOf", "relatedTo", "bridgesTo", "supports", "standardizedBy",
    "contrastsWith",
)

_STOPWORDS = frozenset(
    "a an the of and or to in on for with is are was were be been what how "
    "why when where which who whom me my mine i you your we our it its this "
    "that these those about tell explain describe does do did can could "
    "would should will vs versus between using use please give show".split()
)

_WORD_RE = re.compile(r"[a-z0-9]+")
_SLUG_RE = re.compile(r"[^a-z0-9]+")


def slugify(s: str) -> str:
    """Kebab-case slug, identical to the index build rule."""
    return _SLUG_RE.sub("-", s.lower()).strip("-")


def _ref_to_slug(ref: str) -> str:
    """Map a slug or an ``urn:ngm:class:<slug>`` IRI to its slug."""
    if ":" in ref:
        ref = ref.rsplit(":", 1)[-1]
    return slugify(ref) if not re.fullmatch(r"[a-z0-9-]+", ref) else ref


def _est_tokens(text: str) -> int:
    """Cheap token estimate: chars / 4, rounded up."""
    return (len(text) + 3) // 4


def _truncate(text: str, limit: int) -> str:
    if len(text) <= limit:
        return text
    cut = text[:limit]
    # Prefer a sentence boundary, then a word boundary.
    dot = cut.rfind(". ")
    if dot >= limit // 2:
        return cut[: dot + 1]
    space = cut.rfind(" ")
    if space >= limit // 2:
        cut = cut[:space]
    return cut.rstrip() + "…"


class ScaffoldIndex:
    """In-memory scaffold index with an inverted title-word index.

    Loading is O(index size); after load, :meth:`match` over ~8k classes runs
    well under 50 ms because scoring only touches classes surfaced by the
    inverted index plus one linear slug-substring pass per (long) term.
    """

    def __init__(self, data: dict[str, Any]) -> None:
        if data.get("version") != 1:
            raise ValueError(
                f"unsupported scaffold-index version: {data.get('version')!r}"
            )
        self.generated: str = data.get("generated", "")
        self.classes: dict[str, dict[str, Any]] = data.get("classes", {})
        # slugified-title -> slug (exact title lookup)
        self._by_title: dict[str, str] = {}
        # title word -> set of slugs (inverted index)
        self._inverted: dict[str, set[str]] = {}
        # slug -> number of title words (for overlap normalisation)
        self._title_len: dict[str, int] = {}
        self._slugs: list[str] = list(self.classes.keys())
        for slug, entry in self.classes.items():
            title = entry.get("t") or slug
            self._by_title.setdefault(slugify(title), slug)
            words = [w for w in _WORD_RE.findall(title.lower())]
            self._title_len[slug] = max(len(words), 1)
            for w in words:
                self._inverted.setdefault(w, set()).add(slug)

    # -- loading -------------------------------------------------------------

    @classmethod
    def load(cls, path: Optional[str] = None) -> "ScaffoldIndex":
        """Load from ``path``, else $ONTOLOGY_INDEX, else the default path."""
        p = path or os.environ.get(ENV_VAR) or DEFAULT_INDEX_PATH
        p = os.path.expanduser(p)
        with open(p, "r", encoding="utf-8") as fh:
            return cls(json.load(fh))

    # -- linking -------------------------------------------------------------

    def match(self, prompt: str, max_seeds: int = 4) -> list[tuple[str, float]]:
        """Score classes against the prompt; return top seeds above threshold.

        Scoring: exact title n-gram match (high) + title-word overlap +
        slug-substring bonus. Returns ``[(slug, score), ...]`` sorted by
        score desc, quality desc, slug asc.
        """
        raw_words = _WORD_RE.findall(prompt.lower())
        if not raw_words:
            return []
        terms = [w for w in raw_words if w not in _STOPWORDS and len(w) >= 2]
        scores: dict[str, float] = {}

        # 1. Exact title / slug match on n-grams of the raw word sequence.
        n_words = len(raw_words)
        for n in range(min(MAX_NGRAM, n_words), 0, -1):
            for i in range(n_words - n + 1):
                gram = raw_words[i : i + n]
                if n == 1 and (gram[0] in _STOPWORDS or len(gram[0]) < 2):
                    continue
                gs = "-".join(gram)
                slug = self._by_title.get(gs) or (gs if gs in self.classes else None)
                if slug is not None:
                    scores[slug] = scores.get(slug, 0.0) + EXACT_TITLE_WEIGHT * n

        # 2. Title-word overlap via the inverted index.
        for w in set(terms):
            for slug in self._inverted.get(w, ()):
                scores[slug] = scores.get(slug, 0.0) + (
                    OVERLAP_WEIGHT / self._title_len[slug]
                )

        # 3. Slug-substring bonus for longer terms.
        for w in set(t for t in terms if len(t) >= SUBSTRING_MIN_LEN):
            for slug in self._slugs:
                if w in slug:
                    scores[slug] = scores.get(slug, 0.0) + SUBSTRING_WEIGHT

        seeds = [(s, sc) for s, sc in scores.items() if sc >= MIN_SEED_SCORE]
        seeds.sort(
            key=lambda kv: (
                -kv[1],
                -(self.classes[kv[0]].get("q") or 0.0),
                kv[0],
            )
        )
        return seeds[:max_seeds]

    # -- helpers -------------------------------------------------------------

    def title_of(self, slug: str) -> str:
        e = self.classes.get(slug)
        return (e.get("t") or slug) if e else slug

    def stats(self) -> dict[str, Any]:
        domains: dict[str, int] = {}
        rel_edges = 0
        for e in self.classes.values():
            d = e.get("dom") or "(none)"
            domains[d] = domains.get(d, 0) + 1
            rel_edges += sum(len(v) for v in e.get("rel", {}).values())
        return {
            "classes": len(self.classes),
            "generated": self.generated,
            "title_words_indexed": len(self._inverted),
            "relation_edges": rel_edges,
            "top_domains": sorted(
                domains.items(), key=lambda kv: -kv[1]
            )[:10],
        }


# --- module-level singleton -------------------------------------------------

_INDEX: Optional[ScaffoldIndex] = None


def get_index(path: Optional[str] = None) -> ScaffoldIndex:
    """Lazily load and cache the scaffold index."""
    global _INDEX
    if _INDEX is None or path is not None:
        _INDEX = ScaffoldIndex.load(path)
    return _INDEX


# --- optional prose index (prose-enriched mode) ------------------------------
#
# data/prose-index.json carries the prose layer the structural index truncates:
# per-slug {"dfull": full definition (only when the structural "d" was cut),
# "cl": "Current Landscape" research prose}. OPTIONAL by design — a missing
# file or missing slug degrades to structural-only, silently.

DEFAULT_PROSE_PATH = "~/githubs/llm-server/ontology/data/prose-index.json"
PROSE_ENV_VAR = "ONTOLOGY_PROSE_INDEX"
PROSE_SEEDS = 2            # only the top seeds get prose (budget discipline)
PROSE_CL_CHARS = 1200      # landscape prose used per seed
PROSE_DEF_CHARS = 900      # full-definition chars used per seed

_PROSE: Optional[dict] = None


def get_prose(path: Optional[str] = None) -> dict:
    """Lazily load the prose index; returns {} when absent (fail-open)."""
    global _PROSE
    if _PROSE is None or path is not None:
        p = os.path.expanduser(
            path or os.environ.get(PROSE_ENV_VAR) or DEFAULT_PROSE_PATH
        )
        try:
            with open(p, "r", encoding="utf-8") as f:
                _PROSE = json.load(f).get("pages", {})
        except (OSError, ValueError):
            _PROSE = {}
    return _PROSE


# --- expand + serialise -----------------------------------------------------

def _rel_items(entry: dict[str, Any]) -> Iterable[tuple[str, list[str]]]:
    rel = entry.get("rel") or {}
    for rt in REL_ORDER:
        if rel.get(rt):
            yield rt, rel[rt]
    for rt in sorted(k for k in rel if k not in REL_ORDER):
        if rel[rt]:
            yield rt, rel[rt]


def _section_for(
    idx: ScaffoldIndex,
    slug: str,
    seeds: set[str],
    hops: int,
    prose_entry: Optional[dict] = None,
) -> str:
    e = idx.classes[slug]
    lines: list[str] = []

    meta = [p for p in (e.get("dom"), f"maturity: {e['m']}" if e.get("m") else "") if p]
    head = f"## {idx.title_of(slug)}"
    if meta:
        head += f" ({', '.join(meta)})"
    lines.append(head)

    # Prose-enriched: prefer the untruncated definition when available.
    dfull = (prose_entry or {}).get("dfull")
    if dfull:
        lines.append(_truncate(dfull.strip(), PROSE_DEF_CHARS))
    elif e.get("d"):
        lines.append(e["d"].strip())

    parents = [idx.title_of(_ref_to_slug(r)) for r in e.get("sup", [])]
    ancestors = [idx.title_of(_ref_to_slug(r)) for r in e.get("isup", [])[:ISUP_CAP]]
    isa_bits = []
    if parents:
        isa_bits.append("is-a: " + ", ".join(parents))
    if ancestors:
        isa_bits.append("ancestors: " + ", ".join(ancestors))
    if isa_bits:
        lines.append("; ".join(isa_bits))

    rel_bits = []
    neighbour_order: list[str] = []
    for rt, targets in _rel_items(e):
        tslugs = [_ref_to_slug(t) for t in targets[:REL_CAP]]
        rel_bits.append(f"{rt}: " + ", ".join(idx.title_of(t) for t in tslugs))
        neighbour_order.extend(tslugs)
    if rel_bits:
        lines.append("relations: " + "; ".join(rel_bits))

    # 1-hop neighbour definitions for the top relation targets.
    if hops >= 1:
        added: set[str] = set()
        for n in neighbour_order:
            if len(added) >= NEIGHBOUR_DEFS:
                break
            if n in added or n in seeds or n not in idx.classes:
                continue
            nd = idx.classes[n].get("d")
            if not nd:
                continue
            lines.append(
                f"- {idx.title_of(n)}: {_truncate(nd.strip(), NEIGHBOUR_DEF_CHARS)}"
            )
            added.add(n)

    # Prose-enriched: append the research-dated Current Landscape prose last so
    # the budget clamp (which trims whole sections from the end) still keeps
    # the structural facts when tight.
    cl = (prose_entry or {}).get("cl")
    if cl:
        lines.append(f"landscape: {_truncate(cl.strip(), PROSE_CL_CHARS)}")

    return "\n".join(lines)


def _clamp(sections: list[str], budget_tokens: int) -> str:
    """Trim whole sections from the end until the block fits the budget."""
    kept = list(sections)
    while kept:
        text = HEADER + "\n" + "\n\n".join(kept) + "\n" + FOOTER
        if _est_tokens(text) <= budget_tokens:
            return text
        kept.pop()
    return ""


# --- public API -------------------------------------------------------------

def scaffold(
    prompt: str,
    budget_tokens: int = 1500,
    max_seeds: int = 4,
    hops: int = 1,
    index: Optional[ScaffoldIndex] = None,
    prose: bool = False,
    prose_index: Optional[dict] = None,
) -> str:
    """Build an ontology context block for ``prompt``.

    ``prose=True`` enriches the top ``PROSE_SEEDS`` seed sections with the
    prose index (full definitions + Current Landscape research prose) when
    available — degrading silently to structural-only otherwise.

    Returns ``''`` when no class scores above the seed threshold — the caller
    should then fall back to the raw prompt.
    """
    idx = index if index is not None else get_index()
    seeds = idx.match(prompt, max_seeds=max_seeds)
    if not seeds:
        return ""
    prose_data: dict = {}
    if prose:
        prose_data = prose_index if prose_index is not None else get_prose()
    seed_slugs = {s for s, _ in seeds}
    sections = [
        _section_for(
            idx, s, seed_slugs, hops,
            prose_entry=prose_data.get(s) if i < PROSE_SEEDS else None,
        )
        for i, (s, _) in enumerate(seeds)
    ]
    return _clamp(sections, budget_tokens)


def _message_text(content: Any) -> str:
    """Extract plain text from an OpenAI message content (str or parts list)."""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return " ".join(
            p.get("text", "")
            for p in content
            if isinstance(p, dict) and p.get("type") == "text"
        )
    return ""


def scaffold_messages(
    messages: list,
    budget_tokens: int = 1500,
    max_seeds: int = 4,
    hops: int = 1,
    index: Optional[ScaffoldIndex] = None,
    prose: bool = False,
    prose_index: Optional[dict] = None,
) -> list:
    """Scaffold an OpenAI chat ``messages`` list from its LAST user message.

    Returns a NEW list (input is not mutated). The scaffold block plus a short
    instruction is merged into the first existing system message, or inserted
    as a new system message at position 0. If the scaffold is empty the
    original messages are returned unchanged (as a shallow copy), so this is
    always safe to call.
    """
    out = [dict(m) for m in messages]
    last_user = next(
        (m for m in reversed(out) if m.get("role") == "user"), None
    )
    if last_user is None:
        return out
    block = scaffold(
        _message_text(last_user.get("content")),
        budget_tokens=budget_tokens,
        max_seeds=max_seeds,
        hops=hops,
        index=index,
        prose=prose,
        prose_index=prose_index,
    )
    if not block:
        return out

    injection = SYSTEM_PREAMBLE + "\n\n" + block
    sys_msg = next((m for m in out if m.get("role") == "system"), None)
    if sys_msg is not None and isinstance(sys_msg.get("content"), str):
        sys_msg["content"] = sys_msg["content"].rstrip() + "\n\n" + injection
    else:
        out.insert(0, {"role": "system", "content": injection})
    return out


# --- self-test fixture ------------------------------------------------------

_FIXTURE: dict[str, Any] = {
    "version": 1,
    "generated": "2026-08-09T00:00:00Z",
    "counts": {"classes": 7},
    "classes": {
        "knowledge-graph": {
            "t": "Knowledge Graph",
            "d": "A knowledge graph represents entities and their relationships "
                 "as a typed graph with formal semantics, enabling reasoning "
                 "and structured retrieval.",
            "dom": "ai", "q": 0.9, "m": "mature",
            "sup": ["graph"], "isup": ["data-structure"],
            "rel": {"uses": ["urn:ngm:class:graph-database"],
                    "hasPart": ["ontology"]},
            "bl": ["semantic-web"],
        },
        "graph": {
            "t": "Graph",
            "d": "A graph is a set of vertices connected by edges.",
            "dom": "mathematics", "q": 0.8, "m": "mature",
            "sup": ["data-structure"], "isup": [], "bl": [],
        },
        "data-structure": {
            "t": "Data Structure",
            "d": "An organisation of data for efficient access and modification.",
            "dom": "computer-science", "q": 0.8, "m": "mature",
            "sup": [], "isup": [], "bl": [],
        },
        "graph-database": {
            "t": "Graph Database",
            "d": "A database optimised for storing and querying "
                 "graph-structured data with native traversal.",
            "dom": "data", "q": 0.7, "m": "mature",
            "sup": ["data-structure"], "isup": [],
            "rel": {"contrastsWith": ["vector-database"]}, "bl": [],
        },
        "ontology": {
            "t": "Ontology",
            "d": "A formal, explicit specification of a shared "
                 "conceptualisation of a domain.",
            "dom": "ai", "q": 0.85, "m": "mature",
            "sup": [], "isup": [],
            "rel": {"relatedTo": ["knowledge-graph"]}, "bl": [],
        },
        "vector-database": {
            "t": "Vector Database",
            "d": "A database optimised for similarity search over "
                 "high-dimensional embedding vectors.",
            "dom": "data", "q": 0.75, "m": "emerging",
            "sup": ["data-structure"], "isup": [], "bl": [],
        },
        "semantic-web": {
            "t": "Semantic Web",
            "d": "An extension of the web in which data is given "
                 "well-defined, machine-readable meaning.",
            "dom": "ai", "q": 0.6, "m": "established",
            "sup": [], "isup": [], "bl": [],
        },
    },
}


def _selftest() -> int:
    """Embedded self-test against the inline fixture. Returns exit code."""
    failures = 0

    def check(name: str, cond: bool, detail: str = "") -> None:
        nonlocal failures
        status = "ok" if cond else "FAIL"
        print(f"  [{status}] {name}" + (f" — {detail}" if detail and not cond else ""))
        if not cond:
            failures += 1

    print("selftest: ontology_scaffold")
    idx = ScaffoldIndex(_FIXTURE)

    # slugify contract
    check("slugify kebab-case", slugify("Knowledge  Graph!") == "knowledge-graph")
    check("iri ref -> slug", _ref_to_slug("urn:ngm:class:graph-database") == "graph-database")

    # link + seed + expand + serialise
    prompt = "Explain how a knowledge graph uses a graph database"
    block = scaffold(prompt, index=idx)
    check("block has wrapper", block.startswith(HEADER) and block.endswith(FOOTER))
    check("seed section present", "## Knowledge Graph (ai, maturity: mature)" in block)
    check("is-a line present", "is-a: Graph; ancestors: Data Structure" in block, block)
    check("relations line present", "uses: Graph Database" in block and "hasPart: Ontology" in block)
    # graph-database is itself a seed here, so the neighbour defs come from
    # the non-seed 1-hop targets: Ontology (from knowledge-graph) and
    # Vector Database (from graph-database).
    check("1-hop neighbour def present (Ontology)",
          "- Ontology: A formal, explicit specification" in block, block)
    check("1-hop neighbour def present (Vector Database)",
          "- Vector Database: A database optimised for similarity search" in block, block)
    check("seed not repeated as neighbour def", "- Graph Database:" not in block)

    # hops=0 suppresses neighbour definitions
    block0 = scaffold(prompt, hops=0, index=idx)
    check("hops=0 has no neighbour defs",
          "- Ontology:" not in block0 and "- Vector Database:" not in block0)

    # empty scaffold on irrelevant prompt
    check("irrelevant prompt -> ''", scaffold("best sourdough starter recipe", index=idx) == "")
    check("empty prompt -> ''", scaffold("", index=idx) == "")

    # budget clamp: trims whole sections, respects estimate
    big = scaffold(prompt, budget_tokens=1500, index=idx)
    small = scaffold(prompt, budget_tokens=60, index=idx)
    check("clamp shrinks output", small == "" or len(small) < len(big))
    check("clamp respects budget", small == "" or _est_tokens(small) <= 60)
    tiny = scaffold(prompt, budget_tokens=1, index=idx)
    check("impossible budget -> ''", tiny == "")

    # scaffold_messages: inserts system message, does not mutate input
    msgs = [{"role": "user", "content": "what is a knowledge graph?"}]
    out = scaffold_messages(msgs, index=idx)
    check("system message inserted at 0",
          out[0]["role"] == "system" and HEADER in out[0]["content"])
    check("instruction preamble present", SYSTEM_PREAMBLE.split(".")[0] in out[0]["content"])
    check("input not mutated", len(msgs) == 1 and msgs[0]["role"] == "user")

    # scaffold_messages: merges into existing system message
    msgs2 = [
        {"role": "system", "content": "You are a benchmark model."},
        {"role": "user", "content": "compare a graph database and a vector database"},
    ]
    out2 = scaffold_messages(msgs2, index=idx)
    check("merged into existing system",
          len(out2) == 2 and out2[0]["content"].startswith("You are a benchmark model.")
          and HEADER in out2[0]["content"])

    # scaffold_messages: list-of-parts content, scaffolds from LAST user msg
    msgs3 = [
        {"role": "user", "content": "unrelated earlier turn about weather"},
        {"role": "assistant", "content": "sure"},
        {"role": "user", "content": [{"type": "text", "text": "define ontology"}]},
    ]
    out3 = scaffold_messages(msgs3, index=idx)
    check("parts content + last user used",
          out3[0]["role"] == "system" and "## Ontology" in out3[0]["content"])

    # scaffold_messages: irrelevant prompt returns messages unchanged
    out4 = scaffold_messages([{"role": "user", "content": "sourdough hydration"}], index=idx)
    check("no-match messages unchanged", len(out4) == 1 and out4[0]["role"] == "user")

    # performance: 8k synthetic classes, match must run < 50 ms after load
    import random
    rng = random.Random(42)
    words = ("neural network graph vector agent protocol quantum edge cloud "
             "model data semantic spatial audio render mesh token stream "
             "policy ledger cipher fabric lattice kernel").split()
    big_classes: dict[str, Any] = {}
    for i in range(8000):
        title = " ".join(rng.sample(words, rng.randint(1, 3))).title() + f" {i}"
        s = slugify(title)
        big_classes[s] = {"t": title, "d": "Synthetic definition " * 5,
                          "dom": "bench", "q": rng.random(), "m": "draft",
                          "sup": [], "isup": [], "bl": []}
    big_classes.update(_FIXTURE["classes"])
    t0 = time.perf_counter()
    big_idx = ScaffoldIndex({"version": 1, "generated": "", "classes": big_classes})
    load_ms = (time.perf_counter() - t0) * 1000
    t0 = time.perf_counter()
    _ = scaffold("how does a knowledge graph relate to a neural network model", index=big_idx)
    match_ms = (time.perf_counter() - t0) * 1000
    print(f"  [info] 8k-class index: load {load_ms:.1f} ms, scaffold {match_ms:.2f} ms")
    check("8k-class scaffold < 50 ms", match_ms < 50.0, f"{match_ms:.2f} ms")

    print(f"selftest: {'PASS' if failures == 0 else f'{failures} FAILURE(S)'}")
    return 0 if failures == 0 else 1


# --- CLI --------------------------------------------------------------------

def main(argv: Optional[list[str]] = None) -> int:
    ap = argparse.ArgumentParser(
        description="Ontology context scaffold (PRD-020: link/seed/expand/serialise/clamp)"
    )
    ap.add_argument("prompt", nargs="?", help="prompt to scaffold")
    ap.add_argument("--budget", type=int, default=1500, help="token budget (default 1500)")
    ap.add_argument("--seeds", type=int, default=4, help="max seed classes (default 4)")
    ap.add_argument("--hops", type=int, default=1, help="neighbour expansion hops (0 or 1)")
    ap.add_argument("--index", default=None, help="path to scaffold-index.json "
                    f"(default: ${ENV_VAR} or {DEFAULT_INDEX_PATH})")
    ap.add_argument("--prose", action="store_true",
                    help="prose-enriched mode: full definitions + Current Landscape "
                         "from the prose index (optional; degrades to structural)")
    ap.add_argument("--prose-index", default=None,
                    help=f"prose index path (default: ${PROSE_ENV_VAR} or {DEFAULT_PROSE_PATH})")
    ap.add_argument("--stats", action="store_true", help="print index stats and exit")
    ap.add_argument("--selftest", action="store_true",
                    help="run embedded self-test against an inline fixture (no file needed)")
    args = ap.parse_args(argv)

    if args.selftest:
        return _selftest()

    def _load_or_die() -> ScaffoldIndex:
        try:
            return get_index(args.index)
        except (OSError, ValueError, json.JSONDecodeError) as exc:
            p = args.index or os.environ.get(ENV_VAR) or DEFAULT_INDEX_PATH
            print(f"error: cannot load scaffold index {p!r}: {exc}", file=sys.stderr)
            print(f"hint: set ${ENV_VAR} or pass --index; "
                  "run --selftest to verify the module without an index.",
                  file=sys.stderr)
            raise SystemExit(2)

    if args.stats:
        t0 = time.perf_counter()
        idx = _load_or_die()
        load_ms = (time.perf_counter() - t0) * 1000
        st = idx.stats()
        print(f"index      : {os.path.expanduser(args.index or os.environ.get(ENV_VAR) or DEFAULT_INDEX_PATH)}")
        print(f"generated  : {st['generated']}")
        print(f"classes    : {st['classes']}")
        print(f"title words: {st['title_words_indexed']}")
        print(f"rel edges  : {st['relation_edges']}")
        print(f"load time  : {load_ms:.1f} ms")
        print("top domains:")
        for dom, n in st["top_domains"]:
            print(f"  {dom}: {n}")
        return 0

    if not args.prompt:
        ap.error("a prompt is required unless --stats or --selftest is given")

    idx = _load_or_die()
    prose_data = get_prose(args.prose_index) if args.prose else None
    block = scaffold(args.prompt, budget_tokens=args.budget,
                     max_seeds=args.seeds, hops=args.hops, index=idx,
                     prose=args.prose, prose_index=prose_data)
    if not block:
        print("(empty scaffold — no ontology match; caller should use the raw prompt)",
              file=sys.stderr)
        return 0
    print(block)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
