---
title: How-To Guides
description: Task-oriented recipes for deploying, developing, operating, and extending VisionClaw — grouped into Core, Features, Integration, and Operations.
---

# How-To Guides

> [VisionClaw Docs](../README.md) · How-To Guides

Practical, task-focused recipes. Each guide assumes VisionClaw is already running; if it is not, start with [Installation](../tutorials/installation.md) and [Deploy VisionClaw](deployment.md). For concepts and rationale see [Explanation](../explanation/system-overview.md); for exhaustive lookups see [Reference](../reference/README.md).

---

## Core

Day-to-day tasks for deploying, building, and driving the system.

| Guide | Task |
|-------|------|
| [Deploy VisionClaw](deployment.md) | Launch the Docker stack with `./scripts/launch.sh`, build natively with the GPU feature, install CUDA 13.1, and verify the service URLs. |
| [Development Guide](development.md) | Set up the Rust/React toolchain locally, navigate the project structure, run the test workflow, and add new features. |
| [Agent Orchestration](agent-orchestration.md) | Deploy, configure, and coordinate the multi-agent AI system via the Docker multi-container setup, MCP tools, and the Control Center's agent status surface. |
| [REST API Usage](rest-api-usage.md) | Integrate against the REST API — authentication, common workflows, error handling, pagination, and pairing with the WebSocket stream. |
| [Performance Profiling](performance-profiling.md) | Identify and diagnose bottlenecks across GPU physics, WebSocket throughput, the render pipeline, and Oxigraph queries. |
| [Quest 3 Setup](xr-quest3-setup.md) | Side-load the Godot 4 + OpenXR XR APK onto a Meta Quest 3, enable developer mode, pair LiveKit voice, and verify multi-user presence. |
| [Navigation Guide](navigation-guide.md) | Drive the 3D interface — camera movement, controls, and spatial navigation. |
| [Use the Broker Inbox](use-broker-inbox.md) | Review and action governed proposals through the Judgment Broker Inbox. |
| [ComfyUI SAM3D Setup](comfyui-sam3d-setup.md) | Stand up the ComfyUI SAM3D Docker service for 3D asset generation. |

---

## Features

How to use specific client and graph capabilities.

| Guide | Task |
|-------|------|
| [Auth & User Settings](features/auth-user-settings.md) | Configure server-side authentication middleware and per-user settings lookup. |
| [Command Palette](features/command-palette.md) | Use and extend the command palette — fuzzy search, keyboard navigation, and custom command registration. |
| [Discovery & Similarity Search](features/discovery-search.md) | Find semantically similar concepts, detect ontology gaps, and explore related nodes via content and structural analysis. |
| [Filtering Nodes](features/filtering-nodes.md) | Filter visible graph nodes by quality and authority scores held in node metadata. |
| [Hierarchy Integration](features/hierarchy-integration.md) | Wire the class-hierarchy tree visualisation into the graph canvas. |
| [Intelligent Pathfinding](features/intelligent-pathfinding.md) | Run semantic pathfinding that weights traversal by query relevance and graph semantics, not just hop count. |
| [Local File Sync Strategy](features/local-file-sync-strategy.md) | Apply the two-pass parser, visibility classification, and Pod-first graph-commit saga. |
| [System Health Monitoring](features/monitoring.md) | Use the HealthDashboard to track component health, physics status, and the MCP relay. |
| [Natural Language Queries](features/natural-language-queries.md) | Translate plain-English questions into graph queries via LLM-powered semantic understanding. |
| [Nostr Authentication](features/nostr-auth.md) | Enforce NIP-07/NIP-98 browser-extension authentication before application access. |
| [Onboarding](features/onboarding.md) | Start, skip, and restart the welcome tour, and understand how tour state persists. |
| [Ontology Parser](features/ontology-parser.md) | Configure OWL 2 parsing and the Logseq Markdown conventions it reads. |
| [Stress Majorisation](features/stress-majorization-guide.md) | Configure and wire the stress-majorisation layout optimisation. |
| [Voice Integration](features/voice-integration.md) | Configure the STT/TTS voice pipeline. |
| [Voice Routing](features/voice-routing.md) | Route multi-user voice through the LiveKit SFU with push-to-talk and spatial audio for agents. |
| [Workspace Management](features/workspace.md) | Create, organise, and restore named graph configurations with the Workspace Manager. |

---

## Integration

Connecting VisionClaw to external services and the Solid data plane.

| Guide | Task |
|-------|------|
| [ComfyUI Service Integration](integration/comfyui-service-integration.md) | Run ComfyUI as a supervised service on port 8188 with the API bridge. |
| [Git over HTTP](integration/git-over-http.md) | Enable the git smart-HTTP backend on a Solid pod, then clone and push pod containers as git remotes with WAC authorisation and NIP-98 signed push. |
| [Solid Pod Integration](integration/solid-integration.md) | Wire decentralised user data, graph views, ontology governance, agent memory, and Type Index discovery via JSS. |
| [Solid Pod Creation](integration/solid-pod-creation.md) | Provision and manage per-user Solid Pods. |

---

## Operations

Running and maintaining VisionClaw in production. See the [Operations index](operations/README.md) for the key metrics overview.

| Guide | Task |
|-------|------|
| [Configuration](operations/configuration.md) | Set environment variables, runtime settings, and YAML configuration. |
| [Maintenance](operations/maintenance.md) | Run routine maintenance, backups, and graph-store (Oxigraph) housekeeping. |
| [Security](operations/security.md) | Apply authentication hardening, secrets management, and SSRF mitigations. |
| [Telemetry & Logging](operations/telemetry-logging.md) | Set up structured logging, metrics, and observability. |
| [Troubleshooting](operations/troubleshooting.md) | Resolve common errors with diagnostic commands and known-issue notes. |
| [Metrics Reference](operations/metrics.md) | Look up exposed counters, gauges, and histograms. |
| [Pipeline Admin API](operations/pipeline-admin-api.md) | Manage the semantic intelligence pipeline lifecycle through admin REST endpoints. |
| [Pipeline Operator Runbook](operations/pipeline-operator-runbook.md) | Follow the on-call playbook for monitoring, common issues, and recovery. |
| [Power-User Pod Bootstrap](operations/power-user-bootstrap.md) | Bootstrap a power-user Solid Pod for advanced workflows. |
| [Server Nostr Identity](operations/server-nostr-identity.md) | Manage the server's Nostr identity and key operations. |
| [Forum Runbook](operations/forum-runbook.md) | Operate the nostr-rust-forum (nostr-bbs-rs) service. |
| [agentbox Runbook](operations/agentbox-runbook.md) | Operate the agentbox subsystem. |
| [solid-pod-rs Runbook](operations/solid-pod-rs-runbook.md) | Operate the solid-pod-rs Solid server. |
| [Corpus Regeneration Scaling](operations/corpus-regen-scaling-runbook.md) | Scale corpus regeneration (R3) under load. |
| [Bridge Audit Drift](operations/bridge-audit-drift-runbook.md) | Detect and remediate bridge audit drift (R1). |

---

## See also

- [Tutorials](../tutorials/README.md) — start here if you are learning VisionClaw from scratch.
- [Reference](../reference/README.md) — exhaustive API, protocol, and configuration lookups.
- [System Overview](../explanation/system-overview.md) · [Deployment Topology](../explanation/deployment-topology.md) · [Security Model](../explanation/security-model.md) — the concepts behind these tasks.
- Governing ADRs: [ADR-080 — Forum-Kit Deployment Topology Patterns](../adr/ADR-080-forum-kit-deployment-topology-patterns.md) · [ADR-011 — Auth Enforcement](../adr/ADR-011-auth-enforcement.md)
