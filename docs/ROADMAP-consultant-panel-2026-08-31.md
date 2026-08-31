# ADR ecosystem roadmap — five-model consultant panel synthesis

**Date:** 2026-08-31
**Corpus:** 59 operative ADR-2xxx records (visionclaw 35, agentbox 24) + governance files, assembled into a single 231 KB metadata-rich blob (per-record provenance: last commit, ledger head, `sha256/12`).
**Panel:** GLM 5.3 (z.ai), Gemini 3.1 Pro (native Google API), DeepSeek v4-flash, codex/GPT-5.x (CLI), Claude Fable (repo-verified seat). Each ran the same adversarial brief independently; this document integrates their findings.
**Convention:** `VC-20xx` = visionclaw record, `AB-20xx` = agentbox record (IDs collide across repos — see G-DEFECT-2). Findings tagged with the number of panellists that raised them independently (5/5 = unanimous). Claims marked **VERIFIED** were checked against the live repos this session with `file:line`.

---

## How to read consensus

The five seats saw the same corpus but only the Fable seat could read the code. Where a blob-only seat inferred a hazard and Fable independently confirmed it against source, the finding is load-bearing. Where all five converged from the text alone, the corpus itself is self-contradictory — no code needed. The strongest signals below are both: unanimous *and* code-verified.

---

## P0 — ship-blocking, do first

### P0-1 · The staleness CI gate is armed on zero records (5/5, VERIFIED)
Every PREAMBLE/README/ADR-2001 advertises that stale `verified_commit` + `verified_paths` claims fail CI. **VERIFIED:** 0 of 59 records carry `verified_paths`; `adr-index-gen.js` skips any record without it (`if (!Array.isArray(paths) || paths.length === 0) continue`). Every `verified_commit` is a 9-char abbreviation, though the TEMPLATE says only a full 40-char SHA arms checking. **The corpus's central falsifiability promise is currently marketing.** Worse (DeepSeek R1, VERIFIED): both repos' ADR-2001 share `verified_commit: 73540faa0` — a SHA cannot identify a commit across two independent repos, so the field is not even repo-bound.
- **Action:** Backfill `verified_paths` + full 40-char `verified_commit` on the ~20 code-bearing records (security records first); make CI reject short SHAs and empty paths on any `complete` record; add a negative fixture that mutates a verified path and asserts the build fails. Until this lands, nothing else in governance is enforceable.
- **Effort:** M · **Affected:** both ADR-2001, both TEMPLATE/PREAMBLE, all records, `scripts/adr-index-gen.js`

### P0-2 · Release security collapses under the `dev-auth` build flag (Fable+codex+DeepSeek+GLM, VERIFIED)
**VERIFIED** (`src/main.rs:169`): `enforce_release_env_hygiene()` is `#[cfg(any(debug_assertions, feature = "dev-auth"))] fn …() {}` — a no-op stub whenever `dev-auth` is compiled in. VC-2012's dev-session-token fence is likewise `dev-auth`-gated. VC-2008 shows the dev image builds `cargo build --release --features gpu,dev-auth` — a *release* binary with the hygiene abort stubbed **and** the bypass fence's first gate open. VC-2026/VC-2012 present themselves as the release backstop; that backstop is conditionally absent on a flag no record governs or CI-checks.
- **Action:** Mint a record (or extend VC-2026) requiring the production image to build without `dev-auth`, with a CI/image assertion that the shipped binary carries neither the stub nor the fence. Alternatively, decouple the hygiene abort from the feature flag so it runs on any non-debug build.
- **Effort:** M · **Affected:** VC-2008, VC-2012, VC-2026

### P0-3 · A HIGH-severity tokenless plane is live in production (5/5)
AB-2002 is `accepted/complete/**staged**`: "until the next image rebuild the running box remains `--auth none`", and the accepted residual is that all consumers share uid 1000 so any same-user process reads the token file. AB-2009 (`complete/**live**`) simultaneously claims "the daemon now requires the token regardless" citing `flake.nix:1977`. Two "verified" records contradict each other about the same daemon (DeepSeek C2, GLM 1.3); the honest reading is that the AoE session-control surface is tokenless on the running box *today*, pending a rebuild with **no recorded deadline**.
- **Action:** Date-bound the image rebuild; on activation flip AB-2002 to `live` with a deployment receipt and correct whichever of AB-2002/AB-2009 is false. This is the largest live exposure by the corpus's own severity rating.
- **Effort:** S · **Affected:** AB-2002, AB-2009

### P0-4 · Deployment ships demo-open with no boot-time profile guard (5/5, VERIFIED)
VC-2027 admits profiles are "documented prose, not machine-selected — nothing at boot asserts the running env matches a named profile", and the shipped `docker-compose.unified.yml` realises **demo-open** with `RBAC_PUBLIC_READS:-1`. VC-2003 records `RBAC_PUBLIC_READS=1` + `PUBKEY_VISIBILITY_FILTER=0` as an illegal full-disclosure combination with no runtime rejection. VC-2010's shipped `editor` default means "new authenticated pubkeys can write the graph immediately". These cannot coexist with VC-2026's "fail-closed" claim: a production deploy is one env var away from anonymous full disclosure, and first-contact write access is the default.
- **Action:** Implement the boot-time profile selector VC-2027 defers; abort on the VC-2003 illegal combination; make a machine-selected locked profile the production default; reserve demo-open for an explicit dev selector. (Note: the RBAC_DEFAULT_ROLE=viewer lever for the locked profile already landed this session in `8e78a9d19` — the missing piece is the *boot assertion*, not the lever.)
- **Effort:** M · **Affected:** VC-2003, VC-2010, VC-2026, VC-2027

### P0-5 · The "single identity boundary" is perforated by nine unauthenticated LAN doors (5/5)
AB-2009: "exactly one NIP-98-verifying door — the `:9096` nip98-proxy … forecloses any second identity ingress." AB-2013 sanctions ten non-loopback publishes that never touch it: voice `8443/8444`, browsercontainer `5903/8931/**9222**`, gui-tools `5905/9876/9877`, xr-runtime `5904`. Raw CDP on `9222` is unauthenticated remote browser control (file read via `file://`, credential theft, pivot); VNC and MCP-SSE are control planes. AB-2013 treats "on the sanctioned list with a citation" as equivalent to "safe" — but no citable ADR exists for the cockpit, CDP, or MCP-SSE doors. Policy data lives in a shell script, not the ledger.
- **Action:** Threat-model each sanctioned door (raw CDP `9222` first); bind to loopback unless remote access is essential; where essential, require authenticated TLS + explicit authorisation and mint a governing record per door. Narrow AB-2009 to "single *identity* door, N sanctioned *non-identity* doors each with a stated trust model".
- **Effort:** M–L · **Affected:** AB-2009, AB-2013 (+ new per-door records)

### P0-6 · Session-mirror egress to a third-party cloud relay is ungoverned (Fable, unique)
The estate mirrors every session turn to an external Cloudflare-Worker Nostr relay (NIP-59 gift-wrapped self-DMs, kind-30840 digests — per workspace `CLAUDE.md`). AB-2012 governs relay *ingress* (allowlist-only); **nothing governs this egress of full transcripts off-box**. For an estate whose entire thesis is sovereignty and email privacy, an ungoverned exfiltration channel sits outside the compliance surface entirely.
- **Action:** Mint a record deciding what content leaves the box, the encryption/authority model, whether transcripts are redacted before wrapping, and the off-switch fail-mode. Give it a `review_trigger`. **UNVERIFIED** — hook code not yet inspected; verify before ratifying.
- **Effort:** M · **Affected:** AB-2012 (+ new)

---

## P1 — high value, next

### P1-1 · Provenance is "tamper-evident" but not durable, and cannot be erased (5/5)
VC-2016 guarantees append-only `GRAPH_PROVENANCE` ("DELETE/DROP/CLEAR never issued"; right-to-be-forgotten "cannot be satisfied today"). VC-2017 admits Oxigraph has no PITR and no backup; recovery is GitHub re-sync, which reconstructs `:assert` but **not** provenance. So the indelible audit log is silently *lost* on any volume failure, and simultaneously legally un-erasable while it survives. DeepSeek adds the deeper conflict (C1): if `:assert` rebuilds from GitHub, GitHub is the write-master and VC-2004's "canonical Oxigraph store" claim is false.
- **Action:** Either extend backup to the Oxigraph store (or dual-write provenance to a backed-up SQLite DB), or re-scope VC-2016 to "best-effort, non-durable"; ratify the redaction/crypto-shred mechanism VC-2016 defers; decide estate-wide erasure propagation across GitHub, Oxigraph, SQLite, RuVector, transcripts, backups.
- **Effort:** L · **Affected:** VC-2004, VC-2016, VC-2017, AB-2014

### P1-2 · The cross-repo federation contract is governed from one side only (5/5)
VC-2023 pins the content address "byte-identical to the agentbox `sha12()` contract"; VC-2025 maps inbound `urn:agentbox:*` kinds; VC-2022 converges with AB-2011. **No agentbox ADR** owns `sha12()`, the `urn:agentbox` grammar, or the closed kind-map (VERIFIED: agentbox ledger is ADR-2001–2024, none covers content addressing/URN minting). Each repo's CI validates only its own tree, so an agentbox helper change passes agentbox CI and silently breaks the visionclaw federation join. AB-2011 concedes the convergence was "parallel, not deduped".
- **Action:** Mint a shared cross-repo contract record (or reciprocal agentbox ADRs with typed cross-repo links) owning truncation length, hex casing, and the closed inbound kind-map; add cross-repo CI fixtures asserting sha12 byte-parity and hex-canonical identity, run in both repos.
- **Effort:** M · **Affected:** VC-2022, VC-2023, VC-2025, AB-2011

### P1-3 · The legacy session-bearer realm is a replayable, unrevocable credential (5/5)
VC-2009: "a captured session header pair is replayable until expiry", the branch is "unconditional and non-cfg-gated", expiry slides on `last_seen`, validation is "plain-equality". No logout, revocation, or token binding exists anywhere in the corpus. Combined with the demo-open Editor default (P0-4), a stolen bearer yields write-graph access for the token's sliding lifetime. This also falsifies VC-2013's "sole secp256k1 realm" wording (see P2-1). Gemini adds the revocation gap: a role demotion (VC-2010) does not invalidate an active bearer.
- **Action:** Add revocation/logout + token binding to the bearer realm, or convert the React client to per-request NIP-98 signing (VC-2009's own exit path) and drop the UUID fallback. Decide how a role revocation kills a live session.
- **Effort:** M · **Affected:** VC-2002, VC-2009, VC-2010, VC-2011

### P1-4 · The NIP-98 replay cache is a keyless self-DoS primitive (GLM+DeepSeek+Gemini)
VC-2002: a hard cap of 100 000 live entries "fails closed via `ReplayCacheFull` — we never evict a live entry"; "under a sustained valid-signature flood the server rejects new auth." Any attacker self-mints one keypair and floods unique valid events to fill the cache, locking out **all** legitimate authentication for the TTL. No per-pubkey rate limit or cache-priority policy exists. Gemini adds the restart-replay angle: the cache is process-local `Instant`-based, so crashing/waiting out the process resets the 60 s window.
- **Action:** Add per-pubkey admission/rate-limiting ahead of `ReplayCacheFull`; consider a durable or replica-coordinated replay store (also closes the missing agentbox-side replay cache, DeepSeek G1/H9).
- **Effort:** M · **Affected:** VC-2002, AB-2009

### P1-5 · Key custody, rotation, and break-glass are entirely ungoverned (Fable+codex+DeepSeek)
Five load-bearing secrets are implied and unowned: the visionclaw bridge key (VC-2013), the "currently shared" visionclaw-server publisher key with key-split explicitly pending (AB-2012), the break-glass bearer (AB-2009/AB-2010 — the only credential surviving verifier failure, and the least-governed), the dream-dispatch SSH credential to `john@10.10.10.1` (AB-2024), and `backup-secrets.sh` (VC-2017). AB-2012's relay allowlist is "baked at nix build", so publisher revocation needs a full rebuild.
- **Action:** One record (or one per class) deciding custody, rotation, revocation, and audit for each secret. Prioritise the shared publisher key split (compromise window = one build-deploy cycle) and break-glass issuance/expiry/scope.
- **Effort:** M · **Affected:** AB-2009, AB-2010, AB-2012, AB-2024, VC-2013, VC-2017

### P1-6 · The `did:nostr:local` fail-open identity can reach durable storage (Fable+codex+DeepSeek+GLM)
AB-2011 residual: on a degraded boot the entrypoint "keeps its historic `did:nostr:local` placeholder fallback rather than aborting — a non-canonical identity that must be caught before it reaches storage." The record names the hazard but records **no catch point** and no storage-reject invariant — directly contradicting AB-2009's fail-closed posture and AB-2011's own hex-canonical mandate. Multiple boxes could share the `local` identity (authority confusion).
- **Action:** Abort boot on failed key derivation, or add a storage-layer reject for non-canonical identities with a test. Small, self-contained.
- **Effort:** S · **Affected:** AB-2009, AB-2011

### P1-7 · The Loom façade is a plaintext, unauthenticated LAN model door (Gemini+DeepSeek+Fable)
AB-2023: consumers call `http://192.168.2.132:8084/v1`, no TLS/auth/authenticity recorded, retrieval "falling back transparently to VisionClaw" with unspecified credentials. Any LAN peer that reaches `:8084` can poison ontology retrieval or burn model budget, bypassing the single identity boundary. **UNVERIFIED** whether the port is network-exposed beyond expected consumers.
- **Action:** Verify the exposure; bind to loopback/expected-consumers or front with auth; record the fallback path's credential model.
- **Effort:** M · **Affected:** AB-2009, AB-2023

---

## P2 — correctness and hygiene

### P2-1 · Records marked `complete` while shipping known defects (5/5, VERIFIED)
- VC-2024 (`complete`) admits release builds "silently truncate an over-range ID to its low 26 bits" — the historical V1 corruption mode. **VERIFIED** (`src/utils/binary_protocol.rs:117-136`): the 2^26 ceiling is guarded by `debug_assert!` only, then `(id & NODE_ID_MASK) | FLAG` unconditionally. Dormant below 2^26 nodes, but shipped as "complete".
- VC-2035 (`complete`) documents a stale in-code doc-comment ("still asserts `hierarchical` is EXCLUDED — contradicted by the accept at `:586`") it chose to ship rather than fix.
- **Action:** Promote the node-ID `debug_assert` to a release-mode reject/log guard; fix the VC-2035 doc-comment; downgrade any record to `partial` where a defect ships. Harden the VC-2030 PTX splice to fail on unexpected headers (GLM/DeepSeek R10).
- **Effort:** S · **Affected:** VC-2024, VC-2030, VC-2035

### P2-2 · `implementation_status: complete` vs `activation_status: live` has no coherence gate (codex+Fable+GLM)
Six records pair `partial` implementation with `live` activation (VC-2005, VC-2029, VC-2034, AB-2023, AB-2024; VC-2017 the material one — no Oxigraph backup, yet live). AB-2019 records `implementation: none` for a freeze demonstrably in force. The three-axis enum can represent "complete + admitted-open-defect" and "none + enforced" — states that should be illegal. No roll-up shows cumulative live-but-incomplete risk.
- **Action:** Define the status lattice so incoherent triples are unrepresentable; add a governing-doc roll-up of live-but-incomplete records.
- **Effort:** S · **Affected:** VC-2001, VC-2005/2017/2029/2034, AB-2019/2023/2024

### P2-3 · Supersession graph is empty; IDs collide across repos (5/5, VERIFIED)
**VERIFIED:** all 59 records carry `supersedes: []` / `superseded_by: []`, so the "asymmetric supersession edges" CI check governs an empty graph. Legacy supersession lives only in free-text `lineage:` (which flattens genuine supersession, partial supersession, distillation, and mere reference into one un-typed, un-queryable string). Both repos number from ADR-2001, so `VC-2022` (did:nostr) and `AB-2022` (governed ontology writes) collide; supersession fields have no repo qualifier, so a cross-repo link is inexpressible.
- **Action:** Prefix IDs per repo (or add a `repo` field to supersession edges); type the lineage relation (supersedes / supersedes-in-part / distils / references); populate the typed edges so the reciprocity check has something to bite on.
- **Effort:** L (population is editorial) · **Affected:** both ADR-2001, all records

### P2-4 · Ledger hygiene: missing ADR-2031, split records, disconnected living docs (5/5, VERIFIED)
- **VERIFIED:** no `ADR-2031*` file; the visionclaw index jumps 2030→2032 with no tombstone (the schema has `rejected` for exactly this).
- Granularity: VC-2012 bundles dev-bypass fencing + report-mode acks; the RBAC/security posture is smeared across VC-2003/2009/2010/2011/2026/2027 — no single record states the enforceable whole (candidate: one SECURITY-flags record with profiles as a table).
- The living-doc *Invariants* are declared "the compliance surface" yet sit outside the ledger with no frontmatter, staleness contract, or CI — the strongest-authority layer has the weakest verification (inverts VC-2001's falsifiability story).
- **Action:** Tombstone 2031; split the bundled records; bring living-doc invariants under a staleness contract.
- **Effort:** M · **Affected:** VC-2031, VC-2012, both PREAMBLE, security-domain records

### P2-5 · Two divergent CI/tooling copies for one shared schema (Fable+DeepSeek)
The two repos are policed by different workflows (`docs-ci.yml` vs `invariants.yml`) with apparently different rule sets; each ships (or references) its own `adr-index-gen.js`, and agentbox's README/PREAMBLE disagree on the regenerate path (`agentbox/docs/adr` vs `docs/adr`). AB-2001's verification text was copy-pasted from VC-2001 and may be false in-repo. **UNVERIFIED** — agentbox script copy not diffed this session.
- **Action:** Treat `adr-index-gen.js` as a shared vendored artefact with a parity check; reconcile the two workflows' rule sets; adapt AB-2001's verification prose to its own repo.
- **Effort:** M · **Affected:** both ADR-2001, both PREAMBLE, both CI workflows

---

## Cross-cutting observations

- **The corpus is honest but over-marketed.** Nearly every P0 is something the records *disclose* in their own Consequences/Verification — the failure is that the ledger's advertised enforcement (staleness CI, supersession reciprocity, fail-closed posture) does not actually bite, so disclosed hazards never convert into caught regressions. Fix the enforcement (P0-1, P2-3) and the ledger starts policing itself.
- **The ADR corpus is not self-contained.** Four of five seats independently noted that the normative *Invariants* live in living docs outside the reviewed corpus, so "operative decision pack" overstates completeness. Any future review should include the living docs.
- **Single-owner concentration.** Every record names `jjohare`; no domain/security co-owners, and most `review_trigger`s are semantic ("a proposal", "next touch") and cannot fire in CI. The ledger can age with no accountable review event.
- **What already landed this session** (do not re-open): `RBAC_DEFAULT_ROLE=viewer` locked-profile lever (`8e78a9d19`), the whitelist-structural ports gate rewrite, the `?auth=` NIP-98 WS carrier fix. P0-4 and P0-5 build on these rather than replacing them.

---

## Ranked action list (single view)

| # | Action | Sev | Effort | Consensus | Primary ADRs |
|---|--------|-----|--------|-----------|--------------|
| 1 | Arm the staleness gate (verified_paths + full SHA + CI) | P0 | M | 5/5 ✓ | ADR-2001 ×2, all |
| 2 | Govern production build-flag (no `dev-auth`) | P0 | M | 4/5 ✓ | VC-2008/2012/2026 |
| 3 | Close/deadline the staged AoE token gap | P0 | S | 5/5 | AB-2002/2009 |
| 4 | Boot-time profile selector + illegal-combo abort | P0 | M | 5/5 ✓ | VC-2003/2010/2026/2027 |
| 5 | Reconcile single-boundary vs nine sanctioned doors | P0 | M–L | 5/5 | AB-2009/2013 |
| 6 | Govern session-mirror cloud egress | P0 | M | 1/5 (unique) | AB-2012 +new |
| 7 | Provenance durability + estate erasure | P1 | L | 5/5 | VC-2004/2016/2017 |
| 8 | Cross-repo federation contract + CI | P1 | M | 5/5 ✓ | VC-2022/2023/2025, AB-2011 |
| 9 | Sunset/harden replayable session bearers | P1 | M | 5/5 | VC-2002/2009/2010/2011 |
| 10 | Replay-cache DoS mitigation | P1 | M | 3/5 | VC-2002, AB-2009 |
| 11 | Key custody / rotation / break-glass records | P1 | M | 3/5 | AB-2009/2010/2012/2024, VC-2013/2017 |
| 12 | Kill `did:nostr:local` fail-open | P1 | S | 4/5 | AB-2009/2011 |
| 13 | Audit/authenticate the Loom `:8084` door | P1 | M | 3/5 | AB-2009/2023 |
| 14 | Fix `complete`-with-defect records (node-ID guard, PTX, doc-comment) | P2 | S | 5/5 ✓ | VC-2024/2030/2035 |
| 15 | Status-axis coherence lattice + live-incomplete roll-up | P2 | S | 3/5 | VC-2001, six records |
| 16 | Type + populate supersession; namespace IDs per repo | P2 | L | 5/5 ✓ | ADR-2001 ×2, all |
| 17 | Tombstone 2031; split bundled records; govern living-doc invariants | P2 | M | 5/5 ✓ | VC-2031/2012, PREAMBLE |
| 18 | Shared generator/CI parity across repos | P2 | M | 2/5 | ADR-2001 ×2, CI |

✓ = at least one claim in the row independently code-verified this session.
