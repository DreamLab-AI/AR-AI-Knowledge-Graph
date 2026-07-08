// D2 steering surface (PRD-023 WP-3): pure resolver mapping a graph node
// selection to a steerable agent.
//
// Renderer-free and DOM-free so the "which agent did the operator select" logic
// is unit-testable without React or a live graph. The stateful consumer that
// mounts `AgentDetailPanel` behind the resolved selection is `AgentOpsSurface`.

import type { BotsAgent } from './types/BotsTypes';

/**
 * The `visionclaw:node-selected` event detail shape (subset). Agent nodes carry
 * `agent_type` in their metadata (see `convert_agents_to_nodes` server-side) and
 * may additionally expose their `did:nostr` (COM-14) and their agent/metadata id.
 */
export interface NodeSelectedDetail {
  nodeId?: string;
  label?: string;
  metadata?: Record<string, unknown> | null;
}

/** Read a string field from a loose metadata bag (snake_case or camelCase). */
function metaStr(meta: Record<string, unknown> | null | undefined, ...keys: string[]): string | undefined {
  if (!meta) return undefined;
  for (const k of keys) {
    const v = meta[k];
    if (typeof v === 'string' && v.length > 0) return v;
  }
  return undefined;
}

/**
 * True iff the selected node is an agent node — it carries an `agent_type`, or
 * its node type is explicitly `agent`. A non-agent knowledge-graph node returns
 * false so the steering surface never opens on a document/topic node.
 */
export function isAgentNodeDetail(detail: NodeSelectedDetail | null | undefined): boolean {
  if (!detail) return false;
  const meta = detail.metadata ?? {};
  if (metaStr(meta, 'agent_type', 'agentType')) return true;
  const nodeType = metaStr(meta, 'node_type', 'nodeType', 'type');
  return nodeType === 'agent';
}

/**
 * Resolve the selected node to a known agent's id (the value `AgentDetailPanel`
 * keys on), or `null` when the selection is not a steerable agent.
 *
 * Match order, most specific first:
 *  1. a `did:nostr` on the node metadata matches an agent's `did_nostr`;
 *  2. the node's own metadata/agent id matches an agent's `id`;
 *  3. the selected `nodeId` is itself an agent id;
 *  4. an agent node whose display `name` matches an agent's `name`.
 */
export function resolveSelectedAgentId(
  detail: NodeSelectedDetail | null | undefined,
  agents: BotsAgent[] | null | undefined,
): string | null {
  if (!detail || !agents || agents.length === 0) return null;
  const meta = detail.metadata ?? {};

  // 1) did:nostr on metadata → the agent keyed by that DID.
  const did = metaStr(meta, 'did_nostr', 'didNostr');
  if (did) {
    const byDid = agents.find((a) => a.did_nostr === did);
    if (byDid) return byDid.id;
  }

  // 2) explicit agent/metadata id on the node.
  const metaId = metaStr(meta, 'metadata_id', 'metadataId', 'agent_id', 'agentId', 'id');
  if (metaId) {
    const byMetaId = agents.find((a) => a.id === metaId);
    if (byMetaId) return byMetaId.id;
  }

  // 3) the selected node id is itself an agent id.
  if (detail.nodeId) {
    const byNodeId = agents.find((a) => a.id === detail.nodeId);
    if (byNodeId) return byNodeId.id;
  }

  // 4) name match, but only for an actual agent node (avoids matching an
  //    unrelated document whose label happens to equal an agent name).
  if (isAgentNodeDetail(detail)) {
    const name = metaStr(meta, 'name') ?? detail.label;
    if (name) {
      const byName = agents.find((a) => a.name === name);
      if (byName) return byName.id;
    }
  }

  return null;
}
