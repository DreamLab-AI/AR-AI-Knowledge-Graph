---
id: ADR-2031
title: "Withdrawn — number reserved, no decision recorded"
date: 2026-08-31
decision_status: rejected
implementation_status: none
activation_status: inactive
supersedes: []
superseded_by: []
verified_commit:
verified_paths: []
owner: jjohare
review_trigger: none — tombstone is terminal
repo: visionclaw
---

# ADR-2031 — Withdrawn — number reserved, no decision recorded

## Context
The number ADR-2031 was minted, then withdrawn before any decision was recorded; this tombstone exists so readers do not assume a record was lost.

## Decision
ADR-2031 records no decision; it is a reserved, withdrawn number.

## Consequences
The ADR sequence stays contiguous and auditable — no silent hole between ADR-2030 and ADR-2032.

## Verification
N/A (tombstone).

## Closeout extension — 2026-09-04

CP-01/09. Owner remains jjohare. Rejected/none/inactive is retained: this is a terminal numbering tombstone, not an unimplemented feature. Closeout disposition is preserve without reuse. Keep incoming links resolvable and exclude the record from feature delivery or activation counts; any later design receives its own decision record. No code verification or activation is applicable. This extension records corpus disposition, not a new architecture decision.

## Acceptance progress — 2026-09-05

**Nothing to implement, by design.** This record is a terminal numbering tombstone,
not an unimplemented feature, so it carries no acceptance condition and no code
was written against it. Confirmed during the 2026-09-05 pass:

- The number remains reserved and unreused. No new decision in this pass claimed
  ADR-2031; the work landed on ADR-2018, 2019, 2020, 2024, 2028, 2029, 2030, 2033,
  2034 and 2035, with 2032/2036 receiving a documentation-only receipt
  specification.
- The record is excluded from feature delivery and activation counts. Where this
  pass reports "ADRs advanced", 2031 is deliberately not among them.
- Incoming links stay resolvable: the file is untouched apart from this section.

**Tests run.** None applicable — no code verification or activation exists for a
tombstone.

**Governed paths changed.** None.

**Open.** Nothing. Rejected/none/inactive is retained, and the closeout disposition
"preserve without reuse" holds. Any later design covering this ground receives its
own decision record and its own number.
