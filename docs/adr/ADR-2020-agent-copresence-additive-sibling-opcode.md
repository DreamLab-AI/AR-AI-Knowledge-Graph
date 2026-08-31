---
id: ADR-2020
title: Agent co-presence is an additive sibling opcode, not an extension of the pose frame
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: staged
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: a new social/co-presence field, or pressure to pack social state into the 0x43 pose transform_mask
repo: visionclaw
domain: PROTOCOL-registry
lineage: "Distils ADR-102 (server-initiated presence handshake) + ADR-130 D4 (copresence AgentAvatar/Gaze/Proxemics layer)."
---

# ADR-2020 — Agent co-presence is an additive sibling opcode, not an extension of the pose frame

## Context

The avatar-pose frame (opcode `0x43`) carries skeletal transforms behind a
`transform_mask`. Co-presence work (ADR-130 D4: AgentAvatar/Gaze/Proxemics)
adds social state — activity, gaze, attention — that changes on a different
cadence from skeleton pose and for a different population (agents, not just
human avatars). Packing it into the existing `0x43` mask would couple two
independently-evolving concerns into one codec and one mask budget, so every
social-schema change would risk the pose parser (ADR-102 handshake path).

## Decision

Social/co-presence state rides a **separate additive opcode `0x44`** with its
own mask-gated codec, declared the additive sibling of `0x43`. Skeleton pose
and social state evolve independently: a change to gaze/attention encoding
touches only the `0x44` codec and its field mask, never the `0x43` pose frame,
and vice versa. This forecloses overloading the pose `transform_mask` with
non-skeletal state and forecloses one codec's schema churn destabilising the
other.

## Consequences

- The two concerns version independently; the pose parser is insulated from
  social-schema evolution and each keeps its own field mask budget.
- Once wired, a client tracking a full avatar will consume two frames (pose +
  presence) and must correlate them by node id — the deliberate cost of
  decoupling.
- `0x44` is a registry allocation on the presence socket (per ADR-2019),
  disjoint from the graph tag space; adding further social opcodes follows the
  same additive-sibling pattern rather than mask extension.

## Verification

At e0f8cd896: `crates/visionclaw-xr-presence/src/agent_presence.rs` defines
`pub const OPCODE_AGENT_PRESENCE: u8 = 0x44`, documented "Additive sibling of
0x43 (`wire::OPCODE_AVATAR_POSE`)", with a per-field mask codec and an
opcode-guarded decode (`if bytes[0] != OPCODE_AGENT_PRESENCE { ... }`). The
codec is implemented and fuzzed (`encode_agent_presence`/`decode_agent_presence`
covered by unit, property, and fuzz tests) but **not yet wired into any live
path**: `src/handlers/presence_handler.rs` and `src/actors/presence_actor.rs`
decode only `0x43` (`wire::decode`, opcode-checked), and
`xr-client/tests/unit/test_graph_agents.gd` names "the live 0x44
agent-presence wire" a pending integration point. Hence `activation_status:
staged`, not live. Sibling pose opcode `OPCODE_AVATAR_POSE = 0x43` lives in
`crates/visionclaw-xr-presence/src/wire.rs`.
