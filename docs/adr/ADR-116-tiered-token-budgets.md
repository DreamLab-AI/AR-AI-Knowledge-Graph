# ADR-116 — Model-tier token budgets with a hard, lower-only ceiling

**Status:** Accepted — retroactive record 2026-07-22. Split out of the ADR-112
Decision register (§5, row ADR-116) to document code that already ships:
`agentbox/mcp/servers/lib/ontology-budget.js` (`TIERS` frozen table `:14-21`,
`resolveBudget`/`clampToBudget`).
**Date:** 2026-06-14 (decided under ADR-112) · recorded 2026-07-22
**Decision-type:** Architecture
**Relates:** ADR-112 (keystone §2.5), ADR-026 (3-tier model routing), ADR-115
(the serialised width this governor clamps), ADR-117 (the server-side clamp this
one pairs with), PRD-020

---

## 1. Context

ADR-112 §2.5 required the overflow guarantee to be **structural** — one budget
governor every channel routes through — rather than per-caller discretion. The
adversarial review of the keystone specifically flagged a "discretionary budget"
hole and a 93k-token `full:true` page-body leak. This is the split-out record of
the concrete budget model.

## 2. Decision

`ontology-budget.js` is a pure, dependency-free, synchronous governor. Every
channel routes its serialised subgraph through `clampToBudget()` before it can
reach a model context.

**Per-tier defaults** (frozen `TIERS`, aligned to ADR-026 routing):

| Tier | maxTokens | depth | mode | allowFull |
|---|---|---|---|---|
| booster | 80 | 0 | menu | false |
| haiku | 500 | 0 | menu | false |
| sonnet | 2000 | 1 | expand | true |
| opus | 6000 | 2 | expand | true |

Default tier `sonnet`; unknown tier → default.

**Invariants:**
- **The cap is a hard ceiling; an override may only LOWER it, never raise it**
  (`resolveBudget`) — this closes the discretionary-budget finding.
- **`full:true` page bodies are forbidden below `sonnet`**, and where allowed are
  chunked to ≤ the tier budget — closes the 93k-token leak.
- Token estimate is `ceil(len/4)`, rounding **up** so the governor errs toward
  under-filling, never over.
- Truncation is explicit: the payload carries a `# … [truncated: token budget
  reached]` marker rather than a silent cut.
- The PUSH hot path clamps **locally** (`clampBreadcrumb`) — it must not trust a
  network response for its ≤80-token budget (ADR-112 §2.5).

## 3. Consequences

**Positive** — overflow is guaranteed at one chokepoint, not per caller; the
ceiling cannot be argued upward at a call site. Pairs with ADR-115 (smaller
pre-clamp width) and ADR-117 (server-side row/byte clamp) so the budget holds
end to end.

**Negative** — the `ceil(len/4)` heuristic is an approximation, not a tokeniser;
it deliberately over-estimates, so a payload may be clamped slightly earlier than
a true token count would require. Accepted as the safe direction.

**Neutral** — booster/haiku are menu-only (no expand, no full body); deep prose
is an opus/sonnet privilege, matching the recall economics of ADR-112 §3.
