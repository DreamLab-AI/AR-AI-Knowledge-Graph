---
id: ADR-2076
title: The XR client authenticates the graph socket with NIP-98 only, never a query token
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b00c28a0d766c8cf46cd00b100dab60ef2dd74a4
verified_paths: []
owner: jjohare
review_trigger: a deployment that cannot supply XR_NOSTR_SECRET and needs a different graph-socket credential, which must then be designed as a header or post-upgrade frame rather than a query parameter
repo: visionclaw
domain: XR-client
lineage: removes the XR half of the `?token=` divergence recorded in BASELINE-architecture and XR-client; the surviving path is the NIP-98 authenticate frame of ADR-2032's transport
---

# ADR-2076 — The XR client authenticates the graph socket with NIP-98 only, never a query token

## Context

The Godot XR client appended its graph-socket credential to the URL:
`transport.rs` had `with_token(url, token)` producing `"{url}?token={token}"`, threaded
through `spawn_graph_stream`, `graph_pump` and the gdext `BinaryProtocolClient::connect_to_url`,
and fed from `XR_GRAPH_TOKEN` via `graph_scene.gd`. The socket's actual authentication was
already the NIP-98 `authenticate` text frame sent after subscribe, so the query token was a
redundant second credential on a weaker channel — URLs reach proxy logs, referrers and crash
dumps. `?token=` on `/wss` is a standing divergence from legacy ADR-011 recorded at
`docs/BASELINE-architecture.md:217` and in the XR-client divergence list. Exposed by
diagram VC-36.3.

## Decision

The XR graph socket connects to the plain URL and authenticates only with the NIP-98
`authenticate` frame minted by `signer.rs::nip98_authenticate_json` over that same URL.

`with_token` and its unit test are deleted. The `token` parameter is removed from
`spawn_graph_stream`, `graph_pump` and `BinaryProtocolClient::connect_to_url`, and the
`XR_GRAPH_TOKEN` environment variable, the `_graph_token` field and the `graph_token`
parameter of `connect_to_server` are removed from `graph_scene.gd`. A client with no
`XR_NOSTR_SECRET` still connects and streams positions anonymously; it simply cannot send
mutating messages, which is the pre-existing and intended behaviour.

## Consequences

- `XR_NOSTR_SECRET` is now the only graph-socket credential for the XR client, matching
  XR-client Invariant 6. `XR_GRAPH_TOKEN` is gone; any launch script setting it should drop it
  (it is silently ignored, not an error).
- Graph-socket credentials no longer appear in URLs. This closes the XR side of the `?token=`
  divergence; the server side still *accepts* the query form for other clients and remains
  recorded as an open divergence owned by the wire/core domains.
- The NIP-98 `u` tag is now signed over the same plain URL the socket connected to, removing
  a class of mismatch where the signed URL and the connected URL could differ by the token
  suffix.
- Read-only anonymous streaming is unchanged.

## Verification

Verification ran on the uncommitted working tree above `b00c28a0d766c8cf46cd00b100dab60ef2dd74a4`
and must be re-run at the landing commit. The Godot client is its own workspace and builds in
this container.

- `cargo check --manifest-path xr-client/rust/Cargo.toml --lib` → `Finished dev profile`
  (a cargo global-cache auto-clean permission warning is pre-existing environment noise and
  does not affect the check).
- `cargo test --manifest-path xr-client/rust/Cargo.toml --lib` →
  `test result: ok. 226 passed; 0 failed; 0 ignored`.
- `grep -rn "with_token\|XR_GRAPH_TOKEN\|_graph_token" xr-client/ crates/` → no output.
- Caller sweep before changing the gdext arity:
  `grep -rn "connect_to_url\|spawn_graph_stream\|graph_token\|connect_to_server" xr-client/ crates/`
  → no other call sites; no GUT test under `xr-client/tests/unit/` references these symbols.
- Note for the docs: `xr-client/README.md` and `docs/XR-client.md` describe the headless suite as
  "141 tests"; the suite now reports **226**. The count is corrected in the governing doc as part
  of this change.
