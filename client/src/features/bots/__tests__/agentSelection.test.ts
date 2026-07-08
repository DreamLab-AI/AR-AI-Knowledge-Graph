// D2 (PRD-023 WP-3): node-selection → agent-id resolver.

import { describe, it, expect } from 'vitest';
import { isAgentNodeDetail, resolveSelectedAgentId } from '../agentSelection';
import type { BotsAgent } from '../types/BotsTypes';

const DID = `did:nostr:${'a'.repeat(64)}`;

const agents = [
  { id: 'task-1', name: 'Alpha', type: 'coder', did_nostr: DID },
  { id: 'task-2', name: 'Beta', type: 'researcher' },
] as unknown as BotsAgent[];

describe('isAgentNodeDetail', () => {
  it('is true for a node carrying agent_type metadata', () => {
    expect(isAgentNodeDetail({ metadata: { agent_type: 'coder' } })).toBe(true);
  });

  it('is true when node_type is agent', () => {
    expect(isAgentNodeDetail({ metadata: { node_type: 'agent' } })).toBe(true);
  });

  it('is false for a plain knowledge-graph node', () => {
    expect(isAgentNodeDetail({ nodeId: '42', metadata: { domain: 'ecology' } })).toBe(false);
  });

  it('is false for null', () => {
    expect(isAgentNodeDetail(null)).toBe(false);
  });
});

describe('resolveSelectedAgentId', () => {
  it('resolves by did:nostr on metadata', () => {
    expect(resolveSelectedAgentId({ metadata: { did_nostr: DID } }, agents)).toBe('task-1');
  });

  it('resolves by explicit metadata_id', () => {
    expect(resolveSelectedAgentId({ nodeId: '1000', metadata: { metadata_id: 'task-2' } }, agents)).toBe('task-2');
  });

  it('resolves when the node id is itself an agent id (agent node)', () => {
    // Rule 3 now requires an agent node: the selection carries agent metadata,
    // so a nodeId===agent.id match opens the steering surface.
    expect(
      resolveSelectedAgentId({ nodeId: 'task-1', metadata: { node_type: 'agent' } }, agents),
    ).toBe('task-1');
  });

  it('does NOT resolve a bare nodeId collision on a non-agent node', () => {
    // Rule-3 guard (D2 gap-close): a knowledge-graph node whose id coincidentally
    // equals an agent id, but carrying no agent_type/node_type=agent, must not
    // open the steering surface. Before the isAgentNodeDetail guard this returned
    // 'task-1' and steered a document node.
    expect(resolveSelectedAgentId({ nodeId: 'task-1' }, agents)).toBeNull();
    expect(
      resolveSelectedAgentId({ nodeId: 'task-1', metadata: { node_type: 'document' } }, agents),
    ).toBeNull();
  });

  it('resolves an agent node by name', () => {
    expect(resolveSelectedAgentId({ label: 'Beta', metadata: { agent_type: 'researcher', name: 'Beta' } }, agents)).toBe('task-2');
  });

  it('does not resolve a non-agent node whose label collides with an agent name', () => {
    // No agent_type / node_type=agent, and neither id nor did matches → null,
    // so a document titled "Alpha" never opens the steering panel.
    expect(resolveSelectedAgentId({ nodeId: '999', label: 'Alpha', metadata: { domain: 'docs' } }, agents)).toBeNull();
  });

  it('returns null when there are no agents', () => {
    expect(resolveSelectedAgentId({ nodeId: 'task-1' }, [])).toBeNull();
  });
});
