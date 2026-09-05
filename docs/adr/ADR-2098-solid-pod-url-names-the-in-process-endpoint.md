---
id: ADR-2098
title: Point SOLID_POD_URL at the in-process pod endpoint, not the removed JSS sidecar
date: 2026-09-05
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: b0bc275f6501aae7751b85a72ce15fe1e730e7e8
verified_paths: []
owner: jjohare
review_trigger: A change to the /solid scope prefix, SYSTEM_NETWORK_PORT's default, or re-introduction of an external pod host.
repo: visionclaw
domain: BASELINE-architecture
lineage: completes ADR-032 M3 (embed solid-pod-rs, JSS sidecar removed); relates to ADR-2100 (one JSS notification client); diagram docs/diagrams/estate/09-build-deploy-and-ci-estate.md:461
---

# ADR-2098 — Point SOLID_POD_URL at the in-process pod endpoint, not the removed JSS sidecar

## Context
ADR-032 M3 embedded `solid-pod-rs` as a library and removed the
`solidproject/community-server` service from `docker-compose.unified.yml`. The
pod has since been served in-process under the `/solid` scope on the server's
own listener (`src/handlers/solid_proxy_handler.rs`, `SYSTEM_NETWORK_PORT`,
default 4000).

Two configuration defaults still named the departed sidecar:
`.github/workflows/ontology-publish.yml:31` defaulted `SOLID_POD_URL` to
`http://jss:3030`, and `env.example` set it to `http://visionclaw-jss:3030`.
Neither host resolves. The ontology-publish job's deploy step therefore could
only fail DNS unless a repository variable overrode it, and the failure looked
like a network fault rather than a stale default.

The same drift hid a second fact: the workflow POSTs to
`$SOLID_POD_URL/.notifications` to trigger a broadcast. On the embedded pod that
path is a **GET WebSocket upgrade** (solid-0.1), not a POST trigger; the
sidecar's POST-to-broadcast endpoint left with the sidecar.

## Decision
Both defaults name the endpoint the code actually serves:
`http://localhost:4000/solid`. `vars.SOLID_POD_URL` remains the override for a
deployment where the server is reachable elsewhere; what changes is that the
fallback is a real endpoint rather than a dead service name.

The notification POST stays, annotated as a **best-effort no-op on the embedded
pod** — kept only for a deployment still fronted by JSS. Subscribers to the
embedded pod are notified by the pod itself on the LDP writes that precede it.
The absence is explicit rather than silent.

## Consequences
- A CI run without `vars.SOLID_POD_URL` set now targets a plausible endpoint;
  if it fails, it fails because the server is unreachable, not because the
  default is fiction.
- Anyone reading `env.example` learns the pod is in-process, which is the fact
  ADR-032 M3 established and the config contradicted.
- The `.notifications` POST is documented as expected-to-fail rather than
  appearing as an unexplained `|| echo` tolerance.
- Remaining `jss:3030` references in `docs/how-to/`, `docs/explanation/` and
  `archive/` are **not** changed here; they are prose about the pre-cutover
  design and are out of this record's scope.

## Verification
`grep -n SOLID_POD_URL .github/workflows/ontology-publish.yml env.example`
confirms both defaults now read `http://localhost:4000/solid`. The served path
is `web::scope("/solid")` in `configure_routes`
(`src/handlers/solid_proxy_handler.rs`), mounted from `src/main.rs` on the
listener bound to `SYSTEM_NETWORK_PORT` (default 4000, `src/main.rs:801-805`);
`/.notifications` there is registered `web::get()` only.
Verification ran on the uncommitted working tree above commit
`b0bc275f6501aae7751b85a72ce15fe1e730e7e8`.
