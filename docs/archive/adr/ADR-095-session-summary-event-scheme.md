# ADR-095 — Session-as-Summary Event Scheme (kind-30840)

| Field | Value |
|-------|-------|
| Status | Deferred — frozen by closeout 2026-07-03 (was Proposed 2026-06-02). kind-30840 session-summary digest is live in agentbox; the remaining scheme work is deferred. |
| Drives | PRD-017 §5.4 (F15-F19) |
| Companion ADRs | ADR-093 (substrate), ADR-096 (pod boundary), ADR-097 (topology) |
| Affected repos | `agentbox` (summary generator, dual writer) |
| Evidence | `docs/integration-research/nostr-mobile-bridge/07-nip-substrate.md`, `01-telegram-bridge-teardown.md`, `05-solid-pod-interaction.md` |

## Context

The user wants to "manage sessions via summaries generated about the work done." CTM does NOT generate session summaries — it only forwards Claude Code's own `transcript_summary`/`last_assistant_message` from `Stop` hooks (research 01 §5, `hook.rs:360-393`). The raw material (transcript JSONL, per-turn summary, tool records) exists but no summary artifact is produced. This must be built.

A summary needs two homes: a Nostr event the phone can read in any client (live, browsable feed), and a durable user-owned record (the pod, per ADR-096). The Nostr representation must be addressable so re-publishing updates the same logical session record rather than appending duplicates.

## Decision

### D1 — Session summaries are kind-30840 addressable events

Each session is represented by one **kind-30840** event (NIP-33 parameterised-replaceable; `30000-39999` range → `d`-tag dedup, research 03 §2). The `d` tag is the session id, so re-publishing (status transitions active→complete) replaces in place. Schema:

```jsonc
{
  "kind": 30840,
  "pubkey": "<agent_pubkey>",
  "tags": [
    ["d", "<session_id>"],
    ["p", "<admin_pubkey>"], ["p", "<phone_pubkey>"],
    ["agent", "<agent_pubkey>"],
    ["relay", "<wss://relay_url>"],
    ["start", "<unix_ts>"], ["end", "<unix_ts>"],
    ["status", "active|complete|failed"],
    ["t", "session-summary"],
    ["alt", "Agent session summary"]
  ],
  "content": "{\"title\":...,\"summary\":...,\"tool_calls\":<int>,\"tokens_used\":<int>,\"outcome\":...}"
}
```

The `content` is a JSON object (not encrypted in Phase 1 — the summary is intended to be operator-readable in any client; if confidentiality is required later, wrap in NIP-44). The `p` tags address both operator and phone so the phone's client surfaces it.

### D2 — Dual write: relay event AND pod resource

On session end (and at optional checkpoints), the agent MUST both:
1. publish the kind-30840 event to the relay (D1), and
2. write a JSON-LD summary to the operator's pod at `/sessions/<iso-date>-<session-id>.jsonld` (ADR-096).

The relay event is the live/browsable view AND a durable record — the CF relay is a Cloudflare Worker + Durable Object (`nostr-bbs-relay-worker/src/relay_do/`), so kind-30840 persists transactionally there. The pod resource is therefore not the *only* durable copy; its distinct value is **self-sovereign ownership and export** — the operator owns and controls the pod, whereas the relay (however durable) is operator-controlled but not the user's sovereign store. They carry the same logical content; the pod resource additionally carries `owner_did`, `action_urn`, and resource URNs (research 05 §7).

### D3 — Summary generation source

Phase 1 generates `content` from Claude Code's own `transcript_summary` (`Stop` hook field) — present today, zero token cost. A richer dedicated LLM summarisation pass over the transcript/tool record is an OPTIONAL upgrade (research 01 §5 notes both the raw material and CTM's optional Haiku summariser pattern, `summarizer.rs:144`). The choice is a PRD-017 open question; D3 fixes the Phase 1 default as `transcript_summary`.

### D4 — kind allocation reservation

kind-30840 is reserved for session summaries across the ecosystem. It sits outside the forum's used ranges (kind-0/3/1/4/7/40-42/1059/1984/30910-30916/31400-31405 per research 03 §1) and the ACSP governance band (31400-31405). Phase 3 forum interop must not reuse 30840 for another purpose.

## Consequences

**Positive:**
- Delivers the "manage via summaries" capability CTM lacks, in a form any Nostr client can render.
- Addressable (`d`-tag) means status transitions update one record — no duplicate session entries in the phone feed.
- Dual write reconciles "browsable now" with "durable + sovereign" (ADR-096).

**Negative / risks:**
- Dual write is two operations that can partially fail; the agent must handle "event published, pod write failed" (and vice versa) without losing the summary. Idempotency via `d`-tag (relay) and deterministic path (pod) makes retry safe.
- Unencrypted `content` (D1) means any relay subscriber with the filter can read summary text. Acceptable for Phase 1 on the private agentbox relay (whitelist-gated writes; summaries are operator-addressed); revisit before CF-relay federation if summaries are sensitive.
