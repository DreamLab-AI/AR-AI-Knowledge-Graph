---
id: ADR-2099
title: Delete the unreachable framed-header agent-action decode branch
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b0bc275f6501aae7751b85a72ce15fe1e730e7e8
verified_paths: []
owner: jjohare
review_trigger: Any change to the framed-header layout in `BinaryWebSocketProtocol.createMessage`/`parseHeader` (in particular moving the type byte off offset 0), or a server change that wraps `AgentAction` in the 6-byte framed header.
repo: visionclaw
---

# ADR-2099 — Delete the unreachable framed-header agent-action decode branch

## Context
The browser client carried two decode paths for the `AGENT_ACTION` (0x23) frame.
`processBinaryData` peels the bare tag and returns
(`client/src/store/websocket/binaryProtocol.ts`), and a second
`case MessageType.AGENT_ACTION` in the type switch called a
`handleAgentAction(data, header)` that stripped `MESSAGE_HEADER_SIZE` via
`parseHeader`/`extractPayload`. The second path was unreachable: the framed
header writes its type byte at offset 0 (`createMessage`,
`BinaryWebSocketProtocol.ts:84`) and `parseHeader` reads it back from offset 0
(`:105`), so `header.type === AGENT_ACTION` implies `firstByte === 0x23` — a
case the early return has already consumed. The file's own comment described the
dead branch as the live alternative, inverting the truth. Estate diagram
`docs/diagrams/estate/02-agent-events-agentbox-to-visionclaw.md:272` recorded it
as DOC-DRIFT.

## Decision
The bare-tag decoder is the **only** agent-action decode path, and there is no
fallback. `handleAgentAction` and its switch case are deleted rather than kept
"in case the server frames it later": a second path that cannot execute is not a
fallback, it is drift that reads as behaviour. The wire layout is
`[0x23][count:u16][(len:u16)(event 15B+payload)]…` (`encode_agent_actions`,
`src/utils/binary_protocol.rs:1233`); exactly one byte is peeled, never
`MESSAGE_HEADER_SIZE`.

The decoder's fail-closed posture for unknown versions (ADR-2078) is unchanged
and is now covered by test: an unrecognised lead byte, and an unsupported framed
header version, must never yield agent actions.

Should the server ever frame 0x23, the change is to the tag branch itself, with a
new record — not to reinstate a shadowed switch case.

## Consequences
- One decode path for 0x23; the "which branch runs?" question disappears, and the
  comment now states the reachability argument rather than contradicting it.
- The invariant is executable. `binaryProtocolAgentAction.test.ts` asserts
  `parseHeader`/`extractPayload` are never reached for a 0x23 frame at any size,
  so reintroducing a framed path fails CI instead of lying dormant.
- Cost: if the server adopts framed agent actions, this must be reopened
  deliberately. That is the intended trade — the `review_trigger` above names the
  exact event.
- No behavioural change ships: the deleted code could not execute.

## Verification
Verified on the working tree at `b0bc275f6501aae7751b85a72ce15fe1e730e7e8` (the
tree carried other agents' uncommitted edits; the paths this record touches were
inspected directly).

- Unreachability established by reading `createMessage` (`:84`, type at offset 0)
  against `parseHeader` (`:105`, type from offset 0) — not by inspection of the
  call graph alone.
- `npx tsc --noEmit` → exit 0.
- `eslint src --ext ts,tsx` → 0 errors (25 before this sprint's lint pass).
- `vitest run` → 73 files, 809 tests passed (789 before; +8 from
  `src/store/websocket/__tests__/binaryProtocolAgentAction.test.ts`, +12 from
  ADR-2100's suite). No regressions.
