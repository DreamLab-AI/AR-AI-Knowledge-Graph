# ADR-120 — Authenticate `/api/ontology-agent/propose`; bind agent_id to the verified did:nostr; rate-limit

**Status:** Accepted — retroactive record 2026-07-22. Split out of the ADR-112
Decision register (§5, row ADR-120) as a documentation-only closure of a decision
already shipped. Code: `src/handlers/ontology_agent_handler.rs` (`propose` `:182`,
`configure_ontology_agent_routes` `:345`), guarded by `tests/rec1_route_guard.rs`.
**Date:** 2026-06-14 (decided under ADR-112, P0) · recorded 2026-07-22
**Decision-type:** Security (P0)
**Relates:** ADR-112 (keystone §1, §2.4), ADR-118 (`/load` hardening — sibling
write-hole close), ADR-125 (did:nostr Multikey), PRD-020, PRD-023 WP-12 (REC-1)

---

## 1. Context

The keystone's Context (ADR-112 §1) identified the governed-write anchor
`POST /api/ontology-agent/propose` as **wholly unauthenticated** — the entire
governance story (Whelk gate → ACSP forum → human merge) rested on a forge/flood
hole: anyone could POST a proposal and self-assert any `agent_id`/`user_id` in
the body. ADR-112 §2.4 scheduled the fix as P0. This is the split-out closure
record for the shipped fix.

## 2. Decision

Gate `/propose` and bind identity to a verified key:

- **Auth gate** — only `/propose` is wrapped
  `RequireAuth::authenticated()` inside its own `web::scope("/propose")`
  (`:355-361`); the sibling read routes (`/discover`, `/read`, `/query`,
  `/traverse`, `/validate`, `/status`) stay open, matching the read-pervasive /
  write-governed split of ADR-118.
- **Identity binding** — `agent_id`/`user_id` are **NOT trusted from the request
  body**; the handler overrides them with the verified `did:nostr` pubkey from the
  NIP-98 / session auth (`req.agent_context.agent_id = auth.pubkey`, `:190`),
  discarding whatever the body self-asserted. A forged `agent_id` cannot survive
  (ADR-112 §7 item 3).
- **Rate limit** — `RateLimit::per_minute(20)` is wrapped **outermost** so its
  `extract_identifier` reads the auth extension and keys the limit on
  `user:{pubkey}` rather than source IP (`:353-361`; in actix the last `.wrap()`
  is the outermost layer, so `RateLimit` is listed before `RequireAuth`).

## 3. Consequences

**Positive** — the governed-write loop no longer rests on an anonymous door;
proposals carry a verified provenance and are flood-bounded per identity. Closes
the P0 hole the keystone's adversarial review surfaced. With ADR-118 both write
surfaces (`/load` axiom ingest and `/propose`) are gated.

**Negative** — an agent must present a NIP-98 / session credential to propose;
intended — proposing is a governed, attributable write, not an anonymous one.

**Neutral** — the read routes remain anonymous by design; the asymmetry is the
point (read pervasive, write governed).

## 4. Verification
`tests/rec1_route_guard.rs` (REC-1a, `CANARY-VC-REC1-ROUTE`):
`ontology_agent_propose_rejects_unauthenticated_ingest` runs the real
`NostrService` verification path against a session-less service, so an
unauthenticated request is rejected by the gate itself; a companion assertion
holds the read side anonymous. One-shot regression canary against a refactor
silently dropping the gate. Reconciles ADR-112 §7 item 3.
