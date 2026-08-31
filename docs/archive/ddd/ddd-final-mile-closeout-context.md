# DDD: Final-Mile Closeout Bounded Context

**Status:** Living document
**Date:** 2026-07-22
**Scope:** The cross-repo closeout coordination context — how residue items move through the six unblock states to closure via tick-tock alternation
**Governed by:** [PRD-024 Final-Mile Closeout](../prd/PRD-024-final-mile-closeout.md), [ADR-133 Final-Mile Sprint](../adr/ADR-133-final-mile-sprint.md)
**Conformant to:** Gap-Close (canon) closure protocol (falsification, receipt, canary), Ecosystem Alignment maturity vocabulary (ADR-002), [DDD Gap-Close VisionClaw](ddd-gap-close-visionclaw-context.md)

---

## 1. Bounded Context

This context owns the *coordination* of closure, not the closures themselves. Its domain is the residue register (`TODO-unified.md`) and the alternation protocol that drains it: which state an item is in, whose move it is, what evidence a transition requires, and when the sprint is over. It deliberately does not own any subsystem's domain model — the ontology spine, the wire protocol, the sovereign mesh, and the XR surface each keep their own contexts; this context holds only a `ResidueItem` projection of each with its unblock state.

It conforms to the Gap-Close canon: falsification-before-claim, receipts, canaries, and the maturity tiers arrive from upstream unchanged. Its one original contribution is the **unblock-state taxonomy** as a first-class domain concept — the audit corpus's five-state discipline (technical gap / posture / external / data-floor / live-pending) promoted to six explicit states with `ops-action`, each with a distinct transition trigger and a distinct authorised actor.

## 2. Context Map

| Context | Relationship | Notes |
|---|---|---|
| **Gap-Close (canon)** | Conformist (upstream) | Closure protocol and maturity tiers consumed verbatim; this context adds no parallel closure rules |
| **Gap-Close VisionClaw** (PRD-023/ADR-130) | Customer/Supplier (upstream) | Supplies the `LivenessHarness` and surface-side canaries; this context schedules when they fire |
| **agentbox sovereign mesh** | Customer/Supplier (upstream) | Supplies envelope canaries (COM/REC items), posture flags, and the 2026-07-15 audit ledger; this context sequences their unblocking |
| **Ontology spine** (ADR-112 family) | Customer/Supplier (upstream) | Supplies ADR-117/119 live-fire falsifications as `live-session` items |
| **Operator** | Partnership | The tock half of the alternation — the only actor that can emit `PostureDecided`, `LiveObservationRecorded`, `RebuildAuthorised` |
| **Agent swarm** | Partnership | The tick half — the only actor that can emit `TickCompleted` with a green verification gate |

## 3. Aggregates

- **`ResidueItem`** (root) — identity: register key (K-1, C-3, L-5, T-2, D-1, E-4). Holds: unblock state, owning repo(s), evidence pointers, blocked-by edges. Invariant: exactly one state at a time; state changes only via the transitions in §6.
- **`Tick`** — an autonomous swarm work unit. Holds: menu (the ResidueItems it may touch), verification-gate result, evidence filed. Invariant: may not close a `live-session` item; ends green or reports partial honestly.
- **`Tock`** — a bounded operator unit. Holds: enumerated decision menu with pre-briefs, observations produced. Invariant: never open-ended; every menu entry resolves to a decision, an observation, or an explicit re-deferral.
- **`SprintCloseout`** — the terminal aggregate; satisfied when the PRD-024 §6 falsification block runs clean.

## 4. Domain Events

| Event | Emitted by | Consumed by |
|---|---|---|
| `KeystoneReachable` | Tock 0 (health probe green on `visionclaw-server:4000`) | Tick 1 (canary registration sweep) |
| `CanaryRegistered` / `CanaryFired` | Tick / live session | Next tick (evidence filing, tier promotion) |
| `PostureDecided { surface, verdict, date }` | Tock | Next tick (execution or re-dating) |
| `LiveObservationRecorded` | Tock (driven session, headset, phone) | Next tick — the **only** licence to promote observation-gated tiers |
| `TierPromoted { item, from, to, evidence }` | Tick | Register; gap-close evidence file |
| `DataFloorCleared { pattern_count }` | Async lane check | Tock nod → flag flip |
| `ItemReclassified { from_state, to_state, reason }` | Either | Register — the no-stall rule (ADR-133 §4) |
| `VerificationGateGreen` | Tick end | Tock boundary (safe stopping point) |

## 5. Invariants

1. **One state per item** — an item claiming to be both `code-gap` and `live-session` is mislabelled, and mislabelling is a defect (REC-9 rule).
2. **Observation-gated tiers promote only on a cited `LiveObservationRecorded`** — the swarm cannot self-certify liveness (ADR-133 §2).
3. **Every tick ends with the verification gate run and its true result recorded** — a red gate ends the tick as `partial`, never silently.
4. **Frozen items (§7 of the register) are immutable in this context** — thawing requires a new ADR upstream, not a register edit.
5. **Deciding "stay closed" is a closure** — a posture tock that re-defers must re-date; undated deferrals are drift by definition.

## 6. State Transitions (ubiquitous language)

```
code-gap      --tick implements-->                  closed (evidence: code + tests)
ops-action    --tock/authorised tick performs-->    closed (evidence: probe/receipt)
live-session  --tock observes--> tick files-->      closed (evidence: canary receipt)
posture       --tock decides-->                     closed | re-dated
data-floor    --clock + tock nod-->                 closed (flag flipped, observed)
external      --upstream change-->                  reclassified to one of the above
any           --blocker discovered-->               reclassified (never stalls the alternation)
```

## 7. Ownership Summary

| Artefact | Owner |
|---|---|
| `docs/TODO-unified.md` | This context (single write-point: ticks drain, tocks re-date) |
| Tock decision briefs | Producing tick |
| Gap-close evidence files | The closing tick, citing the enabling tock |
| PRD-024 acceptance run | Tick 4 (`SprintCloseout`) |
| Subsystem domain models | **Not this context** — upstream contexts, unchanged |

## 8. Open Issues

1. Whether `DataFloorCleared` should carry its own falsification before `feed_routing` flips (PRD-024 open question 3).
2. Whether Tock 3 (hardware) items reclassify to `external (hardware access)` after N weeks unattended, and who sets N.
3. Whether this context outlives the sprint as the permanent residue-coordination model, or dissolves back into the canon Gap-Close context at `SprintCloseout` (recommendation: dissolve; the taxonomy survives in the register's rules).
