# Maintainers

VisionClaw is maintained by a small group with commit access, working in the open.
Decisions are recorded in issues and PRs.

## Current maintainers

| Maintainer | GitHub | Focus |
|---|---|---|
| John O'Hare | [@jjohare](https://github.com/jjohare) | Project lead; GPU physics, ontology governance, XR, agent mesh |
| Melvin Carvalho | [@melvincarvalho](https://github.com/melvincarvalho) | Upstream IP; JSS Solid protocol, DID:Nostr, Web Ledgers, identity standards |

## Upstream

VisionClaw's sovereign data layer is backed by [solid-pod-rs](https://github.com/DreamLab-AI/solid-pod-rs),
a Rust port of Melvin Carvalho's [JavaScriptSolidServer (JSS)](https://github.com/JavaScriptSolidServer/JavaScriptSolidServer).
Agent identities use `did:nostr` and Solid Pods for cryptographic data provenance. Protocol-level
decisions and spec alignment defer to the upstream JSS repository.

## Process

Maintainers follow the same workflow as other contributors (issue → branch → PR → review → merge).

## Becoming a maintainer

By invitation of an existing maintainer, after demonstrated substantive
contribution. No formal vote; existing maintainers make the call and
update this file.

## Security

Security disclosures: use [GitHub private security advisories](https://github.com/DreamLab-AI/VisionClaw/security/advisories/new).
