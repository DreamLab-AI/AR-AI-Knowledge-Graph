---
title: Pipeline Admin API Guide
description: REST endpoints for managing the semantic intelligence pipeline
category: how-to
tags:
  - api
  - tutorial
  - backend
updated-date: 2026-01-29
difficulty-level: intermediate
---


# Pipeline Admin API Guide

> **OBSOLETE / REMOVED (ADR-2004)**: The pipeline-admin routes and the
> `pipeline_admin_handler` / `PipelineAdminState` types described here were
> **deleted** during the Neo4j → Oxigraph persistence migration (the handlers were
> SQLite-pipeline-specific). See the comment at `src/main.rs` ("Pipeline admin routes
> removed … in Oxigraph migration, ADR-2004"). This document is retained only as a record
> of the former API and should not be followed for the current system. Recommend
> archiving or deleting once a replacement admin surface (if any) is documented.

**Status**: REMOVED — superseded by ADR-2004
**Last Updated**: January 29, 2026 (content no longer reflects the codebase)

---

## Overview

The Ontology Pipeline Admin API provides REST endpoints for managing the semantic intelligence pipeline: triggering reasoning, monitoring status, and controlling execution flow.

---

## Quick Start

### Trigger Pipeline
```bash
curl -X POST http://localhost:4000/api/admin/pipeline/trigger \
  -H "Content-Type: application/json" \
  -d '{"force": true}'
```

### Check Status
```bash
curl http://localhost:4000/api/admin/pipeline/status
```

### View Metrics
```bash
curl http://localhost:4000/api/admin/pipeline/metrics
```

---

## REST Endpoints

### POST /api/admin/pipeline/trigger
**Description**: Manually trigger ontology reasoning pipeline

**Request**:
```json
{
  "force": true
}
```

**Response**:
```json
{
  "status": "triggered",
  "correlation-id": "abc123",
  "timestamp": "2025-11-03T18:00:00Z"
}
```

---

### GET /api/admin/pipeline/status
**Description**: Get current pipeline execution status

**Response**:
```json
{
  "status": "running",
  "current-stage": "reasoning",
  "progress-percent": 45,
  "started-at": "2025-11-03T17:59:00Z",
  "queue-sizes": {
    "reasoning": 3,
    "constraints": 0,
    "gpu-upload": 0
  }
}
```

---

### POST /api/admin/pipeline/pause
**Description**: Pause pipeline execution

**Request**:
```json
{
  "reason": "Maintenance"
}
```

**Response**:
```json
{
  "status": "paused",
  "reason": "Maintenance",
  "timestamp": "2025-11-03T18:00:00Z"
}
```

---

### POST /api/admin/pipeline/resume
**Description**: Resume paused pipeline

**Response**:
```json
{
  "status": "running",
  "timestamp": "2025-11-03T18:05:00Z"
}
```

---

### GET /api/admin/pipeline/metrics
**Description**: Get pipeline performance metrics

**Response**:
```json
{
  "metrics": {
    "reasoning-latency-ms": 45.2,
    "constraint-gen-latency-ms": 12.3,
    "gpu-upload-latency-ms": 8.7,
    "total-pipeline-latency-ms": 66.2,
    "error-rate": 0.01,
    "cache-hit-rate": 0.85
  },
  "window": "last-1000-executions"
}
```

---

### GET /api/admin/pipeline/events/:correlation-id
**Description**: Query event log for specific execution

**Response**:
```json
{
  "correlation-id": "abc123",
  "events": [
    {
      "type": "OntologyModified",
      "timestamp": "2025-11-03T17:59:00Z",
      "data": {"file-count": 5}
    },
    {
      "type": "ReasoningComplete",
      "timestamp": "2025-11-03T17:59:45Z",
      "data": {"inference-count": 42}
    }
  ]
}
```

---

### POST /api/admin/pipeline/cache/clear
**Description**: Clear pipeline cache

**Response**:
```json
{
  "status": "cleared",
  "cache-entries-removed": 156
}
```

---

## Integration Checklist

### Step 1: Add Module to mod.rs

**File**: `src/handlers/mod.rs`

Add after line 42:
```rust
pub mod pipeline-admin-handler;
pub use pipeline-admin-handler::configure-routes as configure-pipeline-admin-routes;
```

### Step 2: Add Module to services/mod.rs

**File**: `src/services/mod.rs`

Add line:
```rust
pub mod pipeline-events;
```

### Step 3: Add AppState Fields

**File**: `src/app-state.rs`

Add imports:
```rust
use crate::services::pipeline-events::PipelineEventBus;
use crate::services::ontology-pipeline-service::{OntologyPipelineService, SemanticPhysicsConfig};
```

Add fields to AppState struct:
```rust
pub pipeline-event-bus: Arc<RwLock<PipelineEventBus>>,
pub pipeline-service: Arc<OntologyPipelineService>,
pub pipeline-paused: Arc<RwLock<bool>>,
pub pipeline-pause-reason: Arc<RwLock<Option<String>>>,
```

### Step 4: Initialize in AppState::new()

Add after actor initialization:
```rust
// Initialize pipeline infrastructure
info!("[AppState::new] Initializing pipeline event bus and orchestration service");
let pipeline-event-bus = Arc::new(RwLock::new(PipelineEventBus::new(10000)));

let pipeline-config = SemanticPhysicsConfig::default();
let mut pipeline-service = OntologyPipelineService::new(pipeline-config);

// Wire existing actors
if let Some(ref ontology-addr) = ontology-actor-addr {
    pipeline-service.set-ontology-actor(ontology-addr.clone());
}
pipeline-service.set-graph-actor(graph-service-addr.clone());
pipeline-service.set-graph-repository(knowledge-graph-repository.clone());

let pipeline-service = Arc::new(pipeline-service);
let pipeline-paused = Arc::new(RwLock::new(false));
let pipeline-pause-reason = Arc::new(RwLock::new(None));
```

### Step 5: Register Routes in main.rs

**File**: `src/main.rs`

Add import:
```rust
use visionclaw_server::handlers::configure-pipeline-admin-routes;
```

Create pipeline admin state:
```rust
let pipeline-admin-state = web::Data::new(
    visionclaw_server::handlers::pipeline-admin-handler::PipelineAdminState {
        pipeline-service: app-state-data.pipeline-service.clone(),
        event-bus: app-state-data.pipeline-event-bus.clone(),
        paused: app-state-data.pipeline-paused.clone(),
        pause-reason: app-state-data.pipeline-pause-reason.clone(),
    }
);
```

Add to app configuration:
```rust
.app-data(pipeline-admin-state.clone())
```

Register routes:
```rust
.configure(configure-pipeline-admin-routes)
```

---

## Pipeline Flow

### Automatic Execution

```
GitHub Sync → OWL Parse → Oxigraph → [TRIGGER]
                                            ↓
                                       Reasoning
                                            ↓
                                  Constraint Generation
                                            ↓
                                       GPU Upload
                                            ↓
                                    Client Visualization
```

### Event Bus

All pipeline stages emit events:
- `OntologyModifiedEvent`
- `ReasoningCompleteEvent`
- `ConstraintsGeneratedEvent`
- `GPUUploadCompleteEvent`
- `PositionsUpdatedEvent`
- `PipelineErrorEvent`

---

## Configuration

### SemanticPhysicsConfig

Default values:
```rust
SemanticPhysicsConfig {
    auto-trigger-reasoning: true,       // Auto-run on ontology change
    auto-generate-constraints: true,     // Auto-generate constraints
    constraint-strength: 1.0,            // Force multiplier
    use-gpu-constraints: true,           // Upload to GPU
    max-reasoning-depth: 10,             // Inference depth limit
    cache-inferences: true,              // Enable caching
}
```

---

## Actor Integration

### Required Actor Addresses

The pipeline service needs:
1. **OntologyActor** - OWL parsing and storage
2. **GraphServiceActor** - Graph data access ❌ DEPRECATED (Nov 2025) - Use unified-gpu-compute.rs
3. **ReasoningActor** - Inference execution (to be added)
4. **OntologyConstraintActor** - GPU constraint upload (to be added)

### Wiring Example

```rust
pipeline-service.set-ontology-actor(ontology-addr.clone());
pipeline-service.set-graph-actor(graph-addr.clone());
pipeline-service.set-graph-repository(repo.clone());
```

---

## Monitoring

### Log Output

```
[PipelineEventBus] Event published: OntologyModified (correlation: abc123)
[OntologyPipelineService] Triggering reasoning for 5 OWL files
[CustomReasoner] Generated 42 new inferences
[PipelineEventBus] Event published: ReasoningComplete (correlation: abc123)
```

### Metrics Collection

Metrics are tracked for:
- Reasoning latency (ms)
- Constraint generation latency (ms)
- GPU upload latency (ms)
- Error rates
- Cache hit rates

---

## Troubleshooting

### Issue: "Pipeline Service Not Initialized"
**Solution**: Ensure AppState fields are wired correctly.

### Issue: Queue Sizes Always Zero
**Solution**: Queues are placeholders until async task queues are implemented.

### Issue: Metrics Return Zeros
**Solution**: Metrics are placeholders until ReasoningActor integration is complete.

---

## Feature Engineering Pipeline — `/api/discovery/*`

Three admin endpoints control the feature engineering pipeline (ADR-072, PRD-009). These are long-running batch operations designed to run post-sync.

### Trigger Embedding Indexing

Encodes all ontology node text (preferred_term, definition, scope_note) into 384-dim MiniLM-L6-v2 vectors:

```bash
curl -X POST http://localhost:4000/api/discovery/index
```

Returns `nodes_processed`, `nodes_embedded`, `nodes_skipped`, `batches_sent`.

### Trigger KGE Training

Trains TransE embeddings (128-dim) on the full edge set. CPU-only, takes 30–90s depending on graph size:

```bash
curl -X POST http://localhost:4000/api/discovery/train
```

Returns `num_entities`, `num_relations`, `num_triples`, `final_loss`, `epochs_completed`, `duration_ms`.

### Trigger N-Hop Materialisation

Creates transitive 2-hop and 3-hop edges as weak springs. Requires `NHOP_MATERIALIZATION_ENABLED=true`:

```bash
curl -X POST http://localhost:4000/api/discovery/materialize
```

Returns `two_hop_edges_created`, `three_hop_edges_created`, `nodes_processed`, `duration_ms`.

### Recommended Execution Order

After a fresh ontology sync:

```bash
# 1. Index content embeddings
curl -X POST http://localhost:4000/api/discovery/index

# 2. Train topology embeddings
curl -X POST http://localhost:4000/api/discovery/train

# 3. Materialise transitive edges (if enabled)
curl -X POST http://localhost:4000/api/discovery/materialize
```

Each is idempotent — re-running overwrites previous results.

---

## Future Enhancements

### Planned Features
- [ ] Real-time metrics from ReasoningActor
- [ ] Queue depth tracking
- [ ] Performance profiling
- [ ] WebSocket event streaming
- [ ] Pipeline analytics dashboard
- [ ] Async job tracking for long-running feature engineering operations

---

## References

- 
- 
- 
- 
