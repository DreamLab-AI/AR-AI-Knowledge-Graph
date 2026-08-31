# ADR-118 — Read-pervasive / write-untouched: harden `/ontology/load`, resolve the duplicate scope

**Status:** Accepted — retroactive record 2026-07-22. Split out of the ADR-112
Decision register (§5, row ADR-118) to document code that already ships:
`src/handlers/api_handler/ontology/mod.rs:1361-1423` (the `/ontology` scope with
`RequireAuth::power_user().mutations_only()`), guarded by
`tests/rec1_route_guard.rs`.
**Date:** 2026-06-14 (decided under ADR-112) · recorded 2026-07-22
**Decision-type:** Security
**Relates:** ADR-112 (keystone §2.4), ADR-117 (SPARQL clamp — same WS-0 sweep),
ADR-120 (`/propose` auth — the sibling write-hole close), PRD-020

---

## 1. Context

ADR-112 §2.4 fixed the mutation model: the retrieval brain is **read-only**, the
sole write path is the governed `ontology_propose → Whelk EL gate → GitHub PR /
ACSP forum → human merge` loop, and there must be **no ungoverned
`/api/ontology/load`** that bypasses the Whelk consistency gate. The keystone's
own Context (§1) recorded two live holes: axiom ingest (`/load`, `/load-axioms`)
was gated only `authenticated()` rather than `power_user`, and a duplicate
`/ontology` scope muddied which routes were mutating. This is the split-out record
of the `/load` hardening decision.

## 2. Decision

In `api_handler/ontology/mod.rs::config`:
- Wrap the entire `/ontology` scope with **`RequireAuth::power_user().mutations_only()`**
  — every state-changing op (POST/PUT/DELETE), including `/load` and `/load-axioms`
  axiom ingest, is gated at **power_user** (was `authenticated()`, WS-0 hardening,
  `:1361`, `:1392-1394`).
- `mutations_only()` **bypasses safe GET**, so read-only inspection
  (`/graph`, `/classes`, `/inferred`, `/metrics`, `/validate` GET, …) stays
  **public** — the read-pervasive half of ADR-112 §2.4.
- Resolve the duplicate-scope ambiguity: `/validate` and `/axioms` are each
  served on **both** methods with the method-appropriate gate (POST mutating →
  power_user; GET read → public), and the read-only `validate_ontology` is aliased
  (`validate_ontology_ro`) to avoid clashing with the POST `validate_graph`.
- Axiom ingest continues to funnel through the Whelk EL consistency gate; there is
  no direct-load bypass. `AGENTBOX_ONTOLOGY_DIRECT_LOAD` stays off (ADR-112 §2.4).

## 3. Consequences

**Positive** — the "read pervasive, write governed" invariant is enforced at the
router: anonymous reads stay cheap and open, but no unauthenticated or merely-authed
caller can inject axioms. Pairs with ADR-120 (`/propose` auth) to close both write
holes the keystone Context flagged.

**Negative** — a tool that previously reached `/load` with a plain authenticated
token now needs a power_user credential; intended — axiom ingest is a privileged,
governed operation.

**Neutral** — the split of mutating vs read routes onto the same paths (via
`mutations_only`) means the route table lists each dual-method path twice; a
documentation cost only.

## 4. Verification
`tests/rec1_route_guard.rs` (REC-1a/REC-1b, `CANARY-VC-REC1-ROUTE`): asserts the
ontology INGEST routes stay behind the auth gate and the read side stays
anonymous, using a real `NostrService` verification path so an unauthenticated
request is rejected by the gate itself, not a "service missing" shortcut. This is
a one-shot regression canary against a later refactor silently dropping the gate.
