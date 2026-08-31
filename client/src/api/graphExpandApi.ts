// frontend/src/api/graphExpandApi.ts
// Graph2VR "predicate-count-first expansion" client (desktop migration).
//
// Two-step interaction over the live backend, mirroring the Rust handlers in
// src/handlers/api_handler/graph/mod.rs:
//   1. GET  /api/graph/node/{id}/relations  → predicate counts per direction
//   2. POST /api/graph/node/{id}/expand     → neighbours along ONE predicate
//
// Responses are camelCase (backend #[serde(rename_all = "camelCase")]). Node ids
// arrive as numeric u32; the additive-merge path String()-coerces them (a known
// id-type trap in this codebase — see CLAUDE.local memory).

import { unifiedApiClient } from '../services/api/UnifiedApiClient';
import { createLogger, createErrorMetadata } from '../utils/loggerConfig';

const logger = createLogger('graphExpandApi');

// ── Relations (GET) ────────────────────────────────────────────────────────

/** One predicate group incident to a node, in a single direction. */
export interface RelationCount {
  edgeType: string;
  label: string;
  count: number;
}

/** Response of GET /api/graph/node/{id}/relations. */
export interface RelationsResponse {
  outgoing: RelationCount[];
  incoming: RelationCount[];
}

// ── Expansion (POST) ───────────────────────────────────────────────────────

export type ExpandDirection = 'outgoing' | 'incoming';

/** Request body for POST /api/graph/node/{id}/expand. */
export interface ExpandRequest {
  edgeType: string;
  direction: ExpandDirection;
  limit?: number;
}

export interface ExpandNode {
  id: number;
  metadataId: string;
  label: string;
  nodeType?: string;
}

export interface ExpandEdge {
  source: number;
  target: number;
  edgeType: string;
  weight: number;
}

/** Response of POST /api/graph/node/{id}/expand. */
export interface ExpandResponse {
  nodes: ExpandNode[];
  edges: ExpandEdge[];
}

/**
 * Fetch predicate-count-first relation summary for a node. Throws on network /
 * non-2xx failure so the caller (context menu) can surface an honest error
 * rather than an empty-but-silent menu.
 */
export async function fetchNodeRelations(nodeId: string | number): Promise<RelationsResponse> {
  const url = `/graph/node/${encodeURIComponent(String(nodeId))}/relations`;
  const response = await unifiedApiClient.get(url, { timeout: 10000 });
  const data = (response.data?.data ?? response.data) as Partial<RelationsResponse>;
  return {
    outgoing: Array.isArray(data?.outgoing) ? data.outgoing : [],
    incoming: Array.isArray(data?.incoming) ? data.incoming : [],
  };
}

/**
 * Pull neighbours of a node along one predicate/direction. Limit defaults to 25
 * (the backend default) and is clamped server-side to [1, 500]. Throws on
 * failure.
 */
export async function expandNode(
  nodeId: string | number,
  request: ExpandRequest,
): Promise<ExpandResponse> {
  const url = `/graph/node/${encodeURIComponent(String(nodeId))}/expand`;
  const body: ExpandRequest = {
    edgeType: request.edgeType,
    direction: request.direction,
    limit: request.limit ?? 25,
  };
  try {
    const response = await unifiedApiClient.post(url, body, { timeout: 10000 });
    const data = (response.data?.data ?? response.data) as Partial<ExpandResponse>;
    return {
      nodes: Array.isArray(data?.nodes) ? data.nodes : [],
      edges: Array.isArray(data?.edges) ? data.edges : [],
    };
  } catch (error) {
    logger.error(`expandNode(${nodeId}, ${request.edgeType}/${request.direction}) failed:`, createErrorMetadata(error));
    throw error;
  }
}
