# ADR-133 — Final-Mile Sprint: Tick-Tock Alternation Between Swarm and Operator

**Status:** Accepted — 2026-07-22
**Decision type:** Process / delivery mechanics
**Relates:** ADR-131 (reconciliation baseline), ADR-132 (Neo4j removal record), PRD-024 (the sprint's scope), PRD-019/PRD-023 (the gap-close discipline this generalises), agentbox audit 2026-07-15 (projection principle)

---

## Context

The 2026-07-22 audit closed the drift question: the code is right, the docs now say so, and the residue is enumerated in `docs/TODO-unified.md` under a six-state unblock taxonomy. What remains cannot be closed by either party alone:

- The swarm can write code, file evidence, and stage configuration, but **cannot observe live traffic, wear a headset, mint posture decisions, or authorise pushes/rebuilds** — and the gap-close honesty rule (P2-REC-9's tier-overclaim-is-a-breach) forbids it from pretending otherwise.
- The operator can decide and observe, but operator time is the scarcest resource in the system; unbounded "please review everything" requests waste it.

Previous sprints interleaved these implicitly, which produced the `pending-live-session` pile-up: eight canaries code-proven and stalled for want of a single reachable endpoint and one driven session.

## Decision

1. **The sprint alternates strictly: tick (swarm) → tock (operator) → tick …** as scheduled in PRD-024 §4. Ticks are autonomous and hours-long; tocks are bounded menus of enumerated decisions and observations, sized minutes-to-one-session.

2. **Tier-promotion authority is split by evidence type.** A tick may promote a maturity tier only on evidence a machine can produce (compiles, tests, greps, receipts). Any tier requiring live observation (`integrated → released`, canary fires, on-device validation) promotes only in the tick *following* the tock that produced the observation, citing it. Violating this is a canary-discipline breach per the REC-9 precedent.

3. **Every tock is pre-briefed.** The tick before a decision tock produces a one-page brief per decision (context, options, recommendation, falsification of the recommendation). The operator never has to rediscover context to decide.

4. **Blocked items reclassify, never stall.** If a tock cannot happen (hardware absent, counterparty missing), affected items move to the correct TODO-unified state lane (`external`, `data-floor`, re-dated `posture`) and the alternation continues with the remaining menu.

5. **The register is the interface.** `docs/TODO-unified.md` is the single work register for both repos; ticks drain it, tocks re-date it, and the sprint is over when §1–§3 are empty (PRD-024 §5). Split tracking (VisionClaw ladder vs agentbox backlog) is retired as a working interface; `agentbox/docs/developer/backlog.md` remains as agentbox-detail annexe with a pointer banner.

## Consequences

- Operator load becomes predictable: four tocks (≈30 min + one session + four decisions + one hardware session), rather than continuous review.
- The `pending-live-session` backlog drains in two tocks because the keystone (Tock 0's `visionclaw-server:4000`) is sequenced first, unblocking canary registration before any evidence-filing tick runs.
- The alternation leaves an audit trail by construction: every tier promotion cites the tock observation it rests on, extending the falsification culture from artefacts to *process*.
- Cost: genuine serialisation. A tick that finishes early waits for its tock. Accepted — the alternative (swarm self-certifying live behaviour) is the exact failure mode ADR-119 existed to prevent.
- If the operator abandons the sprint mid-alternation, the tree remains consistent: every tick ends with the full verification gate green (PRD-024 §5 last criterion), so any tock boundary is a safe stopping point.
