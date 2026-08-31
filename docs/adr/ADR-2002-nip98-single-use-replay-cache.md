---
id: ADR-2002
title: NIP-98 replay protection is a two-layer scheme with a single-use cache
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e78e958fa
owner: jjohare
review_trigger: horizontal scaling of the backend (replicas/load balancer), or any change to TOKEN_MAX_AGE_SECONDS
repo: visionclaw
---

# ADR-2002 — NIP-98 replay protection is a two-layer scheme with a single-use cache

## Context

NIP-98 validation enforced only a ±60 s freshness window plus method/URL binding.
A captured signed mutation could be replayed freely for its whole validity window;
the XR client's "single-use NIP-98" comment was aspirational. Both 2026-08-31
external reviews flagged this; the sense-check swarm confirmed no event-id cache
existed anywhere in `src/`.

## Decision

Replay resistance is the freshness window **plus** a process-wide single-use
event-id cache (`src/utils/nip98.rs`). The id is claimed atomically under one
lock only **after** full validation, so failed signatures/URL/method/freshness
checks cannot burn a legitimate id. Cache bookkeeping uses monotonic `Instant`
(TTL 2× the window); a hard cap of 100 000 live entries **fails closed** via
`ReplayCacheFull` — we never evict a live entry, because eviction would hand a
flooder a replay primitive. The cache is **process-local by design**: replay
protection does not span replicas.

## Consequences

- Every NIP-98 call site inherits replay protection automatically because all
  validation funnels through `validate_nip98_token()`.
- Under a sustained valid-signature flood the server rejects new auth
  (availability cost) rather than growing memory or permitting replay.
- Horizontal scaling requires shared state (e.g. Redis) or sticky routing
  before the single-process invariant can be relaxed — see the review trigger.
- `docs/SECURITY-profiles.md` invariant 4 records the scheme; weakening either
  layer re-opens replay-within-60s.

## Verification

`cargo test --lib nip98` → 26 passed at `e78e958fa`, including barrier-race
atomicity (16 threads, exactly one winner), cap-boundary fail-closed on an
isolated map, expired-entry reclaim, and non-burning of ids on failed
signature/URL/method/freshness. Two codex adversarial rounds (round 1 BLOCK →
fixed; round 2 concerns closed by the cap-boundary test + prune-clamp fix).
