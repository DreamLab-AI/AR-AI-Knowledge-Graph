---
id: ADR-2014
title: GitHub `public:: true` gates KG inclusion; `owl:class::` bypasses it
date: 2026-08-31
decision_status: accepted
implementation_status: complete
activation_status: live
supersedes: []
superseded_by: []
verified_commit: e0f8cd896
owner: jjohare
review_trigger: a source of authoritative data whose pages carry neither `public:: true` nor `owl:class::`, or a move to per-page ACLs at ingest
repo: visionclaw
domain: DATA-authority-erasure
lineage: legacy ADR-050 (Pod-backed :KGNode visibility), ADR-051 (publish/unpublish saga; GitHub `public:: true` as arbiter, re-checked server-side)
---

# ADR-2014 — GitHub `public:: true` gates KG inclusion; `owl:class::` bypasses it

## Context

The working knowledge graph must surface only *published* authored pages, yet
formal ontology data must ingest wherever it lives in the repo. Two failure
modes were possible: leaking private working pages, or dropping authoritative
ontology pages that happen to sit outside an ontology directory. Anchoring the
gate on a source directory would get both wrong. Prior state (ADR-050/051) made
`public:: true` the publish arbiter, re-checked server-side.

## Decision

A plain markdown page (no `owl:class::`) becomes a KG node **only** if it
carries `public:: true`. A page whose first parsed node has an `owl_class_iri`
is treated as authoritative formal data and ingests **unconditionally**,
independent of publish tagging or directory. The gate anchors on the parsed
owl:class, not on the file path. This forecloses directory-based inclusion
rules and any implicit "everything in dir X is public" shortcut.

## Consequences

- Private working pages are excluded by default; absence of the marker means
  private (`logseq_page_is_public` returns false on absence).
- An ontology page misauthored without `owl:class::` will be gated as a plain
  page and silently dropped unless it also carries `public:: true`.
- No per-page ACL granularity: the gate is binary. Finer visibility (owner
  scope, groups) is out of scope and would need a new mechanism.
- `linked_page` wikilink stubs are dropped regardless; only authored nodes
  contribute, so a public page's private wikilink targets never materialise.

## Verification

Re-checked at `e0f8cd896`: `github_sync_service.rs` computes
`is_ontology` from `n.owl_class_iri.is_some()`, then the gate
`if !is_ontology && !logseq_page_is_public(content) { return; }`.
`logseq_page_is_public` case-insensitively matches a `public::` line and
returns false on absence. `file_service.rs` `is_public_file` enforces the same
`public-access:: true` / legacy `public:: true` rule on the file-service path.
