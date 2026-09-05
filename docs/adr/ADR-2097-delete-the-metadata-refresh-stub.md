---
id: ADR-2097
title: Delete the MetadataActor refresh stub rather than implement it
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b0bc275f6501aae7751b85a72ce15fe1e730e7e8
verified_paths: []
owner: jjohare
review_trigger: A requirement for MetadataActor to read metadata.json directly, or any second writer to the metadata store.
repo: visionclaw
domain: DATA-authority-erasure
lineage: diagram docs/diagrams/visionclaw/25-insight-kpi-nlq-semantics.md:482
---

# ADR-2097 — Delete the MetadataActor refresh stub rather than implement it

## Context
`MetadataActor::refresh_metadata` logged `"Metadata refresh requested"` and
returned `Ok(())`. It was reachable only through
`Handler<RefreshMetadata>`, and `RefreshMetadata` had **no senders anywhere in
the workspace** — only its definition in
`crates/visionclaw-actors/src/messages/graph_messages.rs` and three re-export
lists.

It also could not be implemented honestly. `MetadataStore` is a
`HashMap<String, Metadata>` type alias; the actor is constructed with
`MetadataStore::new()` and holds no path, repository, or store handle. The file
on disk is owned by `FileService` (`METADATA_PATH`), which rebuilds the store
and pushes it in via `UpdateMetadata`. Giving the actor its own loader would
give `metadata.json` a second owner and let the two copies diverge.

## Decision
`RefreshMetadata`, its handler, and `MetadataActor::refresh_metadata` are
**deleted** — from the message definition and all three re-export lists as well
as the actor. Dead code is deleted, not stubbed.

`MetadataActor` is documented as what it is: a cache cell with exactly one write
path (`UpdateMetadata`), fed by the handlers that own `metadata.json`. It is not
a loader and must not acquire a source of its own; a future reload requirement
belongs on `FileService`, which already owns the file.

## Consequences
- One fewer message in the actor protocol, and one fewer handler that returns
  success without doing anything — the shape that makes a subsystem look
  implemented when it is not.
- Single-writer ownership of `metadata.json` is now stated where a reader will
  find it, so the next person who wants a refresh is pointed at `FileService`
  rather than at re-adding this stub.
- If an external refresh trigger is ever needed, it must be added deliberately
  with a real source, not resurrected.

## Verification
`cargo check --workspace --all-targets` exit 0 — proving no sender existed, since
removing the message from the shared crate and its re-exports breaks nothing.
`cargo test -p visionclaw-server --lib -- metadata_actor` — 2 new tests pass
(`update_replaces_the_whole_store`, `update_to_empty_leaves_no_residue`) pinning
`UpdateMetadata` as the sole write path.
Verification ran on the uncommitted working tree above commit
`b0bc275f6501aae7751b85a72ce15fe1e730e7e8`.
