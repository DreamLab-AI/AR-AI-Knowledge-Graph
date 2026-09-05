---
id: ADR-2093
title: The service agent key fails closed and is compared in constant time
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: any new route that authenticates with VISIONCLAW_AGENT_KEY, or a change to the dev-auth build profile
repo: visionclaw
domain: SECURITY-profiles
---

# ADR-2093 — The service agent key fails closed and is compared in constant time

## Context
- Two routes authenticate unattended service-to-service callers with the shared
  `VISIONCLAW_AGENT_KEY` credential. Both got it wrong in the same two ways.
- `image_gen_handler::agent_key()` read the variable and **substituted the literal
  `"changeme-agent-key"` when it was unset**, so an unconfigured deployment accepted a
  publicly-known key on `POST /api/image-gen/agent-submit`. Raised by vc-core.
- `enrichment_proposals_handler::agent_key()` carried an identical fallback — found while fixing the
  first — on `POST /api/enrichment-proposals/:id/decide`, the **governed decision route** the agentbox
  broker-bridge calls. That is the more serious of the two: it gates KG write-back.
- Both compared with `provided != agent_key()`, a short-circuiting `PartialEq<str>` whose runtime
  depends on the length of the matching prefix, leaking the key byte-by-byte to a timing attacker.
- `liveness_harness_handler` already had the correct posture for the same threat model and the same
  credential — fail-closed, constant-time, dev bypass behind a build cfg. The two routes above
  diverged from an in-tree pattern rather than from an unwritten rule.
- PHASE2 decision policy 1: the estate posture is fail-closed, so a code default that fails open is
  fixed, with any dev relaxation gated behind the existing dev profile.

## Decision
A request is authorised **only** when a non-empty `VISIONCLAW_AGENT_KEY` is configured *and* the
presented `X-Agent-Key` matches it exactly. An unset or empty key is never substituted with a
default — it fails closed, so an unconfigured deployment authenticates nobody rather than everybody.

Comparison is constant time: a dependency-free length-check plus XOR-accumulate fold over the byte
slices, matching `liveness_harness_handler::constant_time_eq` (`subtle` and `constant_time_eq` are
transitive-only deps here, so no new direct dependency is taken for four lines of code).

The credential check is a pure function of `(expected, provided)`, split out from the `HttpRequest`
so the fail-closed semantics are unit-testable without a request or a build-cfg dance. On
`image-gen`, the dev bypass lives behind `#[cfg(any(debug_assertions, feature = "dev-auth"))]`,
exactly as `liveness_harness_handler` does it; the release build has no bypass codepath. The
enrichment decision route has **no** bypass in any profile — it is a governed write path.

## Consequences
- A deployment that never set `VISIONCLAW_AGENT_KEY` and relied on the default will now get 401 on
  both routes. That is the point, and it is a deliberate behaviour change: the previous behaviour was
  "any caller who has read the source can drive these endpoints".
- Timing no longer discloses the key prefix.
- Three routes now share one posture and one implementation shape. A fourth would be a good reason to
  lift `check_agent_key`/`constant_time_eq` into a shared module rather than copy it a third time —
  recorded as follow-on work, not done here to keep the security fix small and reviewable.
- `docs/SECURITY-profiles.md` is owned by the estate lead; this ADR is routed to them so the
  fail-closed credential posture can be stated there alongside the other profile rules.

## Verification
Ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`; must be re-run
at the landing commit.

```
$ cargo test -p visionclaw-server --lib agent_key
test handlers::enrichment_proposals_handler::agent_key_tests::only_exact_match_authorises ... ok
test handlers::enrichment_proposals_handler::agent_key_tests::unset_or_empty_key_fails_closed ... ok
test handlers::image_gen_handler::agent_key_tests::constant_time_eq_matches_equality_semantics ... ok
test handlers::image_gen_handler::agent_key_tests::empty_key_fails_closed ... ok
test handlers::image_gen_handler::agent_key_tests::exact_match_authorises_and_mismatch_does_not ... ok
test handlers::image_gen_handler::agent_key_tests::missing_header_fails_closed ... ok
test handlers::image_gen_handler::agent_key_tests::unset_key_fails_closed ... ok
test result: ok. 7 passed; 0 failed

$ cargo check -p visionclaw-server --lib            # exit 0
$ cargo check -p visionclaw-server --lib --features dev-auth
0 errors                                            # the dev-bypass cfg arm also compiles

$ grep -rn "changeme-agent-key" src/ --include=*.rs
# only the ADR-2093 comment and the regression assertion that the literal is now REJECTED
```

The tests assert the specific regression: `check_agent_key(None, Some("changeme-agent-key"))` is
`false`, i.e. the old default is now rejected rather than accepted.

Not verified: no live request was issued against either route in this environment; the evidence is
the pure-function tests plus the compile of both cfg arms.
