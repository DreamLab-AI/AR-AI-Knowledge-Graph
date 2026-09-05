---
id: ADR-2077
title: Delete the dead browser-client surfaces — interactionApi, message:graph, closeAll, empty feature dirs
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a feature proposal that would re-introduce a client-side interaction API or a graph-message event on the cross-service bus, which must then be designed against the live drag/pin path rather than restored from history
repo: visionclaw
domain: XR-client
lineage: applies the estate rule that dead code is deleted, never ported or stubbed; the surviving drag/pin path is the one drawn in VC-32.10
---

# ADR-2077 — Delete the dead browser-client surfaces

## Context

Four client surfaces had no reachable caller. `client/src/services/interactionApi.ts` (376
lines) exported an `InteractionApi` singleton whose only occurrence anywhere in `client/src`
was its own definition — the live drag/pin path is
`features/graph/hooks/useGraphEventHandlers.ts` sending `nodeDragStart` / `nodeDragUpdate` /
`nodeDragEnd` over the graph socket. `WebSocketEventBus` declared a `'message:graph'` event
that nothing ever emitted. `WebSocketRegistry.closeAll()` had zero callers.
`client/src/features/contributor-studio/` and `client/src/features/workspace/` contained no
source files at all — only an empty `types/` and an empty `components/` directory; the real
workspace logic lives in `client/src/api/workspaceApi.ts` and `client/src/hooks/useWorkspaces.ts`.
Exposed by diagrams VC-30.5, VC-32.10, VC-34.16 and VC-34.17.

## Decision

All four are deleted rather than stubbed, commented out or marked deprecated:

- `client/src/services/interactionApi.ts` is removed entirely. Node drag and pin are sent by
  `useGraphEventHandlers.ts` behind an `isReady()` guard, and that is the only interaction
  path. A future client-side interaction API is a new design against the live socket, not a
  restoration of this file.
- The `'message:graph'` member of `WebSocketEventBus`'s event union and its payload-map entry
  are removed. Every other event (`connection:*`, `message:voice`, `message:bots`,
  `message:pod`, `registry:*`) is live and untouched.
- `WebSocketRegistry.closeAll()` is removed. `register`, `unregister`, `get`, `getEntry`,
  `getAll`, `size` and `readyStateLabel` remain.
- The `contributor-studio/` and `workspace/` feature directories are removed. `workspaceApi.ts`
  and `useWorkspaces.ts` are untouched and remain the workspace implementation.

## Consequences

- `client/src/features` now contains 15 directories, all of which hold source; the feature
  tree no longer advertises two features that do not exist.
- `WebSocketEventBus`'s event union is exhaustive over events that are actually emitted, so a
  `switch` over it can be checked meaningfully.
- Nothing can close every registered socket in one call. No caller wanted that, and the
  per-socket `unregister` path remains.
- `client/src/services/api/README.md:131` still lists `interactionApi.ts` in a file inventory;
  that line is corrected as part of this change's documentation pass.

## Verification

Verification ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`
and must be re-run at the landing commit. Each deletion was re-proved dead immediately before
removal, not only from the Phase 1 report.

- `grep -rn "interactionApi\|InteractionApi" client/src/` before deletion → only the file's own
  definition plus one prose line in `client/src/services/api/README.md`; no importer.
- `grep -rn "emit('message:graph'" client/src/` → no output.
- `grep -rn "closeAll" client/src/` → only the definition in `WebSocketRegistry.ts`; no caller.
- `find client/src/features/contributor-studio client/src/features/workspace -type f` → no output.
- After deletion: `cd client && ./node_modules/.bin/tsc --noEmit` → exit 0, no output.
- `cd client && npm test` (`vitest run`) → `Test Files 69 passed (69)`, `Tests 773 passed (773)`.
- `find client/src/features/contributor-studio client/src/features/workspace` → both
  "No such file or directory".
