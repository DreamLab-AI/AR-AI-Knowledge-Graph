# ADR-033: Git-as-bead-provenance for VisionClaw governance events

## Status

Proposed

## Date

2026-05-16

> **Correction (gap-close REC-2, branch `gap-close/2026-07`, 2026-07-08).** This
> ADR references "ADR-041 (BrokerActor)" as the canonical publisher of governance
> decisions. That `BrokerActor` never merged to `main` (ADR-130 Decision 2
> supersedes its transport + Neo4j adapter). On `main` the publisher is the
> ADR-110 ACSP producer (`ElevationActor`) plus the enrichment-decide handler,
> both sitting over the storage-agnostic broker kernel now ported to
> `src/domain/broker/`. Read "`BrokerActor`" below as "the broker governance
> publisher" — the git-bead provenance design is unchanged by which actor writes
> the `BrokerDecisionMade` event.

## Context

Three converging workstreams make a git-backed audit trail for governance
events compelling — and cheap:

1. **ADR-034 (Nostr-signed bead provenance)** establishes that every
   governance event in VisionClaw — BrokerDecision, AgentAdmission,
   PolicyChange — is published as a Nostr event signed by the actor's
   pod-resident key. Provenance today is the **signature chain**.

2. **ADR-041 (broker governance publisher)** is the canonical publisher of
   those signed governance decisions to the server pod under
   `events/governance/`. On `main` this is the ADR-110 ACSP producer +
   enrichment-decide handler over the ported `src/domain/broker/` kernel (ADR-130
   Decision 2), not the unmerged `crashbug` `BrokerActor`. Currently those writes
   go through the JSS sidecar via HTTP PUT; after ADR-032 M3 they will go through
   embedded `solid-pod-rs` directly.

3. **JSS upstream + solid-pod-rs alpha.12 (in flight — see task #1)** add
   `git init` at pod provisioning, plus a `solid-pod-rs-git` sibling crate
   that exposes the smart-HTTP git protocol over the pod's filesystem.

If the VisionClaw server pod is `git init`'d, **every write to
`events/governance/` automatically becomes a git commit**. The git log
over that directory becomes a tamper-evident, hash-chained, byte-level
audit trail that **supplements** the Nostr signature chain. The signature
chain proves authorship and content; the git chain proves order and
byte-identity over time. Two independent provenance layers, both
cryptographic, both auditable with standard tooling (`nostr-verify`,
`git verify-commit`).

The cost is near-zero: git already exists, the pod already has a
filesystem, and the commits piggyback on writes VisionClaw is doing
anyway. The only gating dependencies are:

- `solid-pod-rs` alpha.12 (task #1).
- agentbox-side wiring of `[sovereign_mesh.git].auto_init = true`
  (task #2).
- ADR-032 M3 (VisionClaw stops going through the JSS sidecar).

## Decision

**Adopt git-as-storage for the VisionClaw server pod's
`events/governance/` directory** once the upstream prerequisites land
(alpha.12 + agentbox wiring + ADR-032 M3).

Concretely:

### D1 — Pod provisioning

When VisionClaw's server pod is provisioned (via `solid-pod-rs-idp`),
enable:

```toml
[sovereign_mesh.git]
auto_init = true
scope     = "events/governance/"
```

`auto_init = true` causes the pod root (or the configured scope, if
sub-tree git is supported by alpha.12) to be initialised as a git
working tree. Any other directory (`events/inbox/`, `events/outbox/`,
`data/`, `acl/`) is **outside** the working tree and is **not** tracked.

### D2 — Commit-on-write

For each `BrokerDecisionMade` event written to the pod by `BrokerActor`:

| Git field | Value |
|---|---|
| `author.name` | VisionClaw server WebID label (e.g. `visionclaw@dreamlab.ai`) |
| `author.email` | The Nostr npub or did:nostr URI |
| `author.date` | `created_at` from the Nostr event (UTC ISO-8601) |
| `committer` | Same as author |
| `committer.date` | Same as author (via `committer-date-is-author-date`) |
| `subject` | `governance decision {case_id}: {outcome}` |
| `body` | The signed Nostr event JSON (event id, sig, pubkey, tags, content) |

This guarantees:

- Author/committer date == Nostr `created_at`, so the git log reads in
  the same temporal order as the Nostr feed.
- The commit body is **the canonical signed Nostr event**, so
  `git log --format=%B` over `events/governance/` reproduces the full
  governance feed; no additional dump is needed.
- The git tree SHA hashes the file content, so any byte-level tamper
  is detectable independently of Nostr signature verification.

### D3 — Verification surface

- **Operator audit:** `git log --follow events/governance/ --format='%H %ad %s'`.
- **Tamper detection (no GPG):** `git fsck` + `git log --format=%T` (tree-SHA
  chain).
- **Tamper detection (GPG, future):** `git verify-commit <sha>` — requires
  signing commits with the pod's GPG/SSH key. **Deferred** to a follow-up
  ADR; the Nostr signature in the commit body is already cryptographic
  and sufficient for the v1 of this pattern.
- **Cross-check:** for any case, the operator can:
  1. Pull the Nostr event from a relay by `event_id`.
  2. Pull the matching git commit by message-grep.
  3. Compare bytes: they must be identical.

### D4 — Scope discipline (governance only)

Git tracks **only** `events/governance/`. Specifically excluded:

- `events/inbox/` — ephemeral mail, GC'd; tracking would balloon the repo.
- `events/outbox/` — likewise ephemeral.
- `data/` — user-content blobs; provenance is per-blob and is handled by
  Solid Notifications / WAC, not git.
- `acl/` — WAC ACLs are policy, not events.

A `.gitignore` at the pod root enforces this. Recommendation: do **not**
make the pod root the working tree if alpha.12's `scope` feature lands;
otherwise rely on `.gitignore`.

## Consequences

### Positive

- **Independent second provenance layer.** Nostr signatures prove
  authorship; git tree-SHAs prove order and bytes. Either can be
  audited without the other.
- **Zero new tooling for operators.** `git log`, `git blame`,
  `git diff`, `git fsck` all just work.
- **Byte-level tamper detection** with standard tools.
- **Future-compatible with GPG-signed commits** (`git verify-commit`)
  once we wire pod-resident signing keys (deferred).
- **Survives pod migration.** Cloning the pod via the
  `solid-pod-rs-git` smart-HTTP backend transports the full audit
  trail with byte-perfect history.

### Negative

- **Disk growth.** Every governance decision adds an object + a tree +
  a commit. Mitigation: scheduled `git gc --auto` cron at the pod
  level (handled by `solid-pod-rs-git`).
- **Commit ordering vs Nostr ordering.** Solved by setting
  `author.date = committer.date = nostr.created_at`, but writes that
  arrive out of order will produce a git log that is **temporally**
  monotonic but **structurally** linear (later commits referencing
  earlier author-dates). Acceptable.
- **Two provenance systems to keep in sync.** A write that succeeds
  on Nostr but fails on git (e.g. disk full) is a partial-success
  failure mode. Mitigation: BrokerActor must treat the git commit
  as part of the write transaction — fail the whole publish if the
  commit fails. This is a constraint on the M3 wiring code.

### Neutral

- **No wire-format change.** Nostr consumers, NIP-04 readers,
  NIP-98 verifiers see exactly the same events. Git is purely a
  server-side audit layer.
- **AGPL implications unchanged.** `solid-pod-rs-git` is AGPL-3.0,
  same as the rest of solid-pod-rs; ADR-032 already accepts that
  licence boundary.

## Open questions

- **Should agent pods (not just the server pod) adopt the same
  pattern?** Recommendation: defer to a per-agent ADR. Agent pods
  emit far more events; the disk-growth concern is more acute, and
  the audit value is lower (agent decisions are not policy).
- **Should we commit `events/governance/` deletes?** Current
  recommendation: governance events are append-only by policy.
  Reject deletes at the WAC layer; git therefore never sees a
  deletion. If a redaction is required, write a redaction event
  (a new commit) rather than mutating history.
- **GPG-signing the commits.** Deferred. The Nostr signature in
  the commit body is already cryptographic. GPG would add a
  second layer keyed to the pod (rather than the event author),
  which has value but adds key-management cost. Revisit when
  the pod-resident GPG/SSH-key workflow is wired (likely after
  JSS Phase 2).

## Implementation prerequisites

This ADR is **proposed only** and cannot be accepted until:

1. **Task #1** — `solid-pod-rs` alpha.12 ships with `git init` at
   pod provisioning and the `[sovereign_mesh.git]` config block.
2. **Task #2** — agentbox wires git auto-init and surfaces the
   git HTTP route through its pod provisioner.
3. **ADR-032 M3** — VisionClaw stops going through the JSS sidecar
   and writes through embedded `solid-pod-rs` directly. (Until M3,
   the writes go via HTTP through JSS, which does not expose the
   underlying filesystem in a way that lets us commit-on-write.)

When all three land, BrokerActor's `publish_governance_decision()`
gains a commit step. That code change is **NOT** in scope for this
ADR and **NOT** in scope for the current audit pass.

## Related decisions

- **ADR-032** — Embed solid-pod-rs as Rust library. M2 (registry pin)
  completed alongside this proposal in audit 2026-05-16; M3 (consumer
  wiring) is a prerequisite for ADR-033 acceptance.
- **ADR-034** — Nostr-signed bead provenance. This ADR adds a git
  audit layer **beneath** ADR-034; signature chain remains canonical
  provenance.
- **ADR-041** — BrokerActor. The publisher whose `publish_*` methods
  will gain a commit step when this ADR is accepted.
- **Upstream ADR-087 (solid-pod-rs)** — CF-Workers-portable cores.
  Orthogonal: VisionClaw runs native, so the Worker portability gap
  does not constrain this design.
- **Upstream ADR-088 (solid-pod-rs)** — WAC Turtle serializer. Also
  orthogonal; ACLs are deliberately excluded from the git scope (D4).
- **Task #3 (NRF)** — CF Workers git limitation ADR. VisionClaw is
  not Worker-bound, so the limitation does not propagate here.
