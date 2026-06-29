# Audit: VisionClaw solid-pod-rs integration

**Date:** 2026-05-16
**Auditor:** visionclaw-auditor (team `git-pods-mesh`, task #5)
**Branch at audit:** `radical-rollback` (NOT pushed — `main` workflow rule)
**Scope:** AUDIT + dep-pin + ADR-033 proposal only. No refactor. No actor-code edits.

---

## 1. Scope and constraints

- Task #5 is an **audit + proposal** pass.
- User previously instructed "leave VisionClaw" and has now reversed. Treat carefully:
  no edits beyond the safe dep-pin in `Cargo.toml` lines 169–200.
- Forbidden edits: `broker_actor.rs`, `server_nostr_actor.rs`, any in-flight actor code.
- No cutover to embedded `solid-pod-rs` (that is ADR-032 M3+).
- No feature-flag changes beyond what the dep-pin itself requires.

## 2. ADR-032 milestone status

| Milestone | Description | Status (pre-audit) | Status (post-audit) |
|---|---|---|---|
| M1 | Land `solid-pod-rs` / `-nostr` / `-idp` as `optional`, gated by `solid-pod-embed` feature, dep-graph-only | ✅ done (commit `8ec41f75b`) | ✅ unchanged |
| M2 | Pin to a stable rev / crates.io release | 🟡 placeholder pin (`branch = "main"`) | ✅ **completed — `version = "0.4.0-alpha.11"`** |
| M3 | Wire consumers (NIP-98 verify, did:nostr resolver, fs-backend) behind the feature flag, retire JSS sidecar | ⏳ pending | ⏳ pending (next ADR-032 pass) |
| M4 | Default `solid-pod-embed` on; remove `solid_proxy_handler.rs` | ⏳ pending | ⏳ pending |
| M5 | Remove JSS sidecar from Docker compose | ⏳ pending | ⏳ pending |

## 3. Dependency resolution model

### Pre-audit (`Cargo.toml` lines 198–200)

```toml
solid-pod-rs       = { git = "https://github.com/DreamLab-AI/solid-pod-rs", branch = "main", default-features = false, optional = true }
solid-pod-rs-nostr = { git = "https://github.com/DreamLab-AI/solid-pod-rs", branch = "main", default-features = false, optional = true }
solid-pod-rs-idp   = { git = "https://github.com/DreamLab-AI/solid-pod-rs", branch = "main", default-features = false, optional = true }
```

- `branch = "main"` resolved to commit `d8a1c81a` (`v0.4.0-alpha.11`) per `cargo tree`.
- A floating-branch pin is non-reproducible: any new push to upstream `main` mutates
  the resolved SHA on the next `cargo update`.
- All three deps are `optional = true` and gated behind the `solid-pod-embed`
  Cargo feature. No consumer in `src/` references them yet.

### Post-audit

```toml
solid-pod-rs       = { version = "0.4.0-alpha.11", default-features = false, optional = true }
solid-pod-rs-nostr = { version = "0.4.0-alpha.11", default-features = false, optional = true }
solid-pod-rs-idp   = { version = "0.4.0-alpha.11", default-features = false, optional = true }
```

- Now pulled from crates.io (publish landed upstream — see workspace commits `f8a696e`,
  `874524f`, `b4f0434` on `main`).
- Resolution is fully reproducible from `Cargo.lock`.
- `default-features = false` retained — alpha.11 still lacks several JSS-Phase-1
  feature surfaces (`key-provisioning`, `nip05-endpoint`, etc.). M3 will switch
  individual consumers' feature flags on as they get wired.
- `optional = true` retained — `solid-pod-embed` feature flag remains the only
  way to pull these crates into the compile unit.

## 4. Feature surface (alpha.11)

Intended ADR-032 surface (per Cargo.toml comment block at line 192):

```
fs-backend, nip98-schnorr, did-nostr, nip05-endpoint,
schnorr-verifier, pod-provisioning, key-provisioning
```

At alpha.11 several of these features do not yet exist upstream
(notably `key-provisioning`). M3 must verify and enable per-consumer.

Sibling crates **NOT** added (per ADR-032 §Deferred):

- `solid-pod-rs-server` — reference HTTP route templates only.
- `solid-pod-rs-activitypub` — only needed if VisionClaw federates via AP.
- `solid-pod-rs-git` — see **ADR-033** (proposed in this pass).
- `solid-pod-rs-didkey` — out of scope (VisionClaw is Nostr-first).

## 5. Consumer-code audit (`src/`)

```bash
grep -rn "solid_pod_rs\|solid-pod-rs" src/
# → 0 hits
```

No call sites. The dep-graph entries compile in isolation; no symbols are
imported into the binary. **Conclusion:** the dep-pin is risk-free for the
M1 surface — there is no consumer to break.

JSS sidecar consumers (still active, untouched in this audit):

- `src/handlers/solid_proxy_handler.rs` — reverse proxy to JSS over HTTP.
- `src/handlers/image_gen_handler.rs` — secondary JSS env reference.
- `src/handlers/mod.rs` — module wiring.
- `src/services/nostr_bridge.rs` — bridges Nostr identity into JSS.

These are M3+M4 refactor targets and are **explicitly out of scope** for this audit.

## 6. Resolved versions at audit time (`cargo tree` snapshot)

```
solid-pod-rs       v0.4.0-alpha.11 (was: git+...?branch=main#d8a1c81a)
solid-pod-rs-idp   v0.4.0-alpha.11 (was: git+...?branch=main#d8a1c81a)
solid-pod-rs-nostr v0.4.0-alpha.11 (was: git+...?branch=main#d8a1c81a)
```

`cargo update -p solid-pod-rs -p solid-pod-rs-nostr -p solid-pod-rs-idp` ran
clean: 3 packages relocated from the git source to the crates.io source.
264 unchanged transitive deps.

## 7. Build verification

- `cargo metadata --format-version 1 --features solid-pod-embed` — **passes**
  (full dependency graph resolves; 3.3 MB JSON output emitted cleanly).
- `cargo check --workspace` — **fails in this container**, but the failure is
  a pre-existing environment limitation: the `cust v0.3.2` build script
  panics with `Could not find a cuda installation` because this audit
  container lacks a CUDA toolkit (CUDA-bearing builds run in tmux tab 6 on
  the GPU host per `workspace/CLAUDE.md`). The failure is **unrelated to
  the dep-pin** — it reproduces identically on the pre-audit `Cargo.toml`.
  Dep-resolution itself is clean. The pin is therefore **kept**.

## 8. Decision

- ✅ Pin landed in `Cargo.toml` lines 198–200.
- ✅ Comment block above the pin rewritten to reflect M1+M2 completion and
  to point to ADR-033 for the `solid-pod-rs-git` sibling crate.
- ⏳ M3 (consumer wiring) is the next ADR-032 pass — not done here.
- ⏳ ADR-033 (git bead provenance) lands as **Proposed** in this pass.

## 9. Follow-ups

1. ADR-033 acceptance gate: blocked on solid-pod-rs alpha.12 (task #1: git auto-init
   at pod provisioning) and agentbox wiring (task #2).
2. M3 wiring sequence:
   a. Stand up `solid-pod-rs` `FsBackend` behind a feature flag, parallel to JSS.
   b. Route NIP-98 verification through `solid-pod-rs-nostr` first (smallest blast radius).
   c. Migrate did:nostr resolver next.
   d. Migrate pod-provisioning (`solid-pod-rs-idp`) last — depends on JSS Phase 1 features.
3. JSS sidecar removal is gated on M3 + M4 sign-off and is **explicitly deferred**
   from this audit.

## 10. Related ADRs

- ADR-032: Embed solid-pod-rs as Rust library — this audit advances it from M1 to M2.
- ADR-033 (proposed in this pass): git-as-bead-provenance.
- ADR-034: Nostr-signed bead provenance — git becomes a storage / audit layer beneath this.
- ADR-041: BrokerActor signed governance decisions — the events that get git-committed.
- Upstream ADR-087: CF-Workers-portable cores — irrelevant for VisionClaw (native runtime).
- Upstream ADR-088: WAC Turtle serializer — orthogonal but lives in the same alpha.11.
