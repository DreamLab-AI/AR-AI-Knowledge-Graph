# Resume Prompt — 2026-06-12 session hand-off

> Copy-paste everything below this line into a fresh Claude Code session in
> `/home/devuser/workspace/project` to pick up where we left off.

---

Resume the 2026-06-12 workstreams. First run
`mcp__claude-flow__memory_search({query: "2026-06-12 shipped", namespace: "patterns", limit: 10})`
and read `docs/adr/ADR-110-agentic-actors-acsp-control-surfaces.md` plus
agentbox `docs/reference/prd/PRD-015-consumer-broadcast-economy.md` and
`docs/reference/adr/ADR-032-402-scheme-grammar.md` for full context. State
of the world at hand-off, then the work queue:

## Where everything stands (all committed and pushed)

- **Graph pipeline**: node-collapse fixed (SPARQL upsert + path-keyed SHA1 +
  EntityKind classifier); stub purge done; twin renames applied at source;
  `elevatedFrom::` → `elevated_from` bridge edges; corpus ≈ 196 working
  pages + ~5.9k authored classes + ~9.8k owl_class frontier stubs (the
  elevation queue — accepted as legitimate population).
- **ACSP producer shipped (ADR-110)**: `src/services/acsp/{events,client}.rs`
  on nostr_sdk; `ElevationActor` (panel `vc-elevation`, cases `vc-elev-`,
  max 5 open) + voice-primary ranking via `elevation_voice.rs`;
  `VoiceInterfaceActor` routes spoken config to the settings assistant.
  Local Whisper in, Kokoro out.
- **XR**: native Godot 4 + gdext client expanded (V3 52-byte wire incl.
  analytics tail, instanced edges, community/centrality/anomaly rendering,
  server-authoritative drag, NIP-98 WS auth, LOD by centrality, backoff).
- **Realtime fixes**: reheat 0.997 + max(spring,repel) ratio; periodic full
  broadcast; worker tween loop restored; WS inbound watchdog; 2s subscribe
  rate-limit; SpaceMouse snap-to-level removed.
- **Docs**: ecosystem-wide mermaid truth-up landed in ALL FIVE repos
  (VisionClaw `429691bd1`, agentbox `99dc2a19`, forum `98cf18e`,
  solid-pod-rs `fa84a12`, website `e828259`) — every diagram verified
  against code, aspirational designs labelled, audit snapshots banner-stamped.
- **PRD-015 + ADR-032 (agentbox `82246b1f`, `4ced53fb`)**: consumer &
  broadcast economy surfaces, **Lightning-first** (operator decision: NWC/
  L402 native rail; NO native EVM/USDC — x402 detect-only, payable via C9
  delegation only). ADR-032 is the frozen 402 scheme grammar, status
  *proposed*, acceptance = fixture corpus green as merge gate.

## Work queue (rough priority order)

1. **PRD-015 Phase 1 implementation (agentbox)** — C1 `lib/pay402.js` pure
   classifier + `tests/contract/pay402/` captured-bytes fixture corpus
   (ADR-032 D2/D4 are the spec); C3 spend-policy middleware (fail-closed,
   `[payments.consumer]` keys + schema + validator); C2 native payer
   (NIP-98 ledger debit, idempotent single retry); C4 receipt/activity URNs
   via `lib/uris.js` on EVERY attempt; B2 additive `accepts[]` in
   `payment-gate.js` with byte-identical legacy regression; B1
   `/.well-known/x402.json` boot-time generator; C5 `skills/payment-router`
   skill; C6 estimate+hold. Exit: node-A-pays-node-B end to end, suites
   green in standalone + client modes. Then flip ADR-032 to accepted.
2. **Spend-approval ACSP case (PRD-015 Open Q1, resolved design)** —
   kind-31402 `abx-pay-` cases for above-threshold spends, kind-31403
   decisions; reuse the ElevationActor/AcspClient pattern (VisionClaw
   `src/actors/elevation_actor.rs` is the reference producer).
3. **Elevation loop operations (VisionClaw)** — register the ACSP panel
   pubkey in the forum relay `agent_registry`
   (`POST /api/governance/agents/register`); set `ELEVATION_ACTOR_ENABLED=1`,
   `FORUM_RELAY_URL`, `ACSP_PANEL_NOSTR_PRIVKEY` in container env; then
   watch the first real frontier cases land. Follow-ups: durable rejection
   skip-list (currently session-scoped); bridge/retire the
   enrichment-proposals REST path into ACSP case types.
4. **BC20 receipt/activity crossing (agentbox known gap)** —
   `bc20-provenance-bridge.js` `crossOutbound` has no production caller;
   wire it so spend/elevation provenance reaches the host graph
   (economy-loop.md §What remains; PRD-015 §8 names it).
5. **XR on-device validation** — Quest session against the dev stack
   (xr-runtime sidecar rebuild + headset run, PRD-019 #27); LiveKit Android
   AAR for voice media — also unlocks per-user/room voice attribution the
   VoiceDemandLedger already models (`speaker` arg).
6. **Security follow-ups (from the diagram/QE sweeps)** — unauthenticated
   `GET /api/graph/data` (interface-sequences FINDING-1); no settings
   broadcast to other connected sessions (FINDING-6, we are multi-client);
   solid-pod-rs `PodError::PayloadTooLarge` falls through to 500 instead of
   413 (`server lib.rs:230-240`, found 2026-06-12, not yet fixed); forum
   search-worker admin set still env-only vs D1 (cartography Gap 2).
7. **Doc debris (small)** — `docs/reference/rest-api.md` still documents
   ~30 broker/workflow REST endpoints that don't exist (decide: delete
   tables or implement); `docs/reference/protocols/protocol-matrix.md` +
   `glossary.md` still claim 0x42/36B wire (no mermaid, escaped the sweep);
   `docs/tutorials/installation.md` stale `docker ps` narrative;
   solid-pod-rs `docs/diagrams/rendered/*.png` stale vs corrected `.mmd`
   (mmdc broken in container — re-render elsewhere); agentbox should repin
   solid-pod-rs past `fa84a12` to pick up corrected upstream docs.
8. **CUDA kernel-count truth** — docs variously claim 37/39/92 kernels;
   code has 82 `__global__` declarations. Pick a blessed counting method
   and align the claims.

## Constraints to honour (non-negotiable)

- Lightning/L402/NWC for real money; never build a native EVM/USDC rail.
- RuVector MCP memory only; never file-based memory or raw SQL INSERT.
- Never SSH to the host; host builds go via tmux tab 6
  (`tmux send-keys -t 6 './scripts/launch.sh up dev' Enter`) — tab 6 is
  currently attached to the compose log tail, don't Ctrl-C it.
- agentbox repo: no host-project specifics (refer to "host project" by
  role); manifest-gate everything (Nix package set + supervisor block +
  schema + validator together).
- Push with `env -u GITHUB_TOKEN git push …` (env token lacks access; the
  gh-auth credential works). VisionClaw pushes to `dreamlab-github`.
- jjohare/logseq + personal-context-portfolio are PRIVATE.
