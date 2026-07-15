// P0 data hygiene (Lane A): the agent poll must map only genuine swarm agents,
// never the knowledge/ontology nodes returned when a server ignores graph_type.

import { describe, it, expect } from 'vitest';
import { transformAgentData, isGenuineAgentNode } from '../hooks/useAgentPolling';
import type { AgentSwarmData } from '../services/AgentPollingService';

type SwarmNode = AgentSwarmData['nodes'][number];

const agentNode = (over: Partial<SwarmNode> = {}): SwarmNode => ({
  id: 1,
  metadataId: 'task-1',
  label: 'Coder',
  metadata: { agent_type: 'coder', status: 'busy' },
  ...over,
});

const knowledgeNode = (id: number): SwarmNode => ({
  id,
  metadataId: `page-${id}`,
  label: 'Some Document',
  type: 'page',
  metadata: { workload: '0' },
});

describe('isGenuineAgentNode', () => {
  it('accepts a node carrying agent_type metadata', () => {
    expect(isGenuineAgentNode(agentNode())).toBe(true);
  });

  it('accepts a node whose type is agent or bot', () => {
    expect(isGenuineAgentNode({ id: 2, metadataId: 'x', label: 'x', type: 'agent' })).toBe(true);
    expect(isGenuineAgentNode({ id: 3, metadataId: 'y', label: 'y', type: 'bot' })).toBe(true);
  });

  it('rejects a knowledge-graph node', () => {
    expect(isGenuineAgentNode(knowledgeNode(100))).toBe(false);
  });
});

describe('transformAgentData', () => {
  it('maps only genuine agents, dropping knowledge/ontology nodes', () => {
    const data: AgentSwarmData = {
      nodes: [agentNode(), knowledgeNode(100), knowledgeNode(101)],
      edges: [],
    };
    const { agents } = transformAgentData(data);
    expect(agents).toHaveLength(1);
    expect(agents[0].id).toBe('task-1');
    expect(agents[0].type).toBe('coder');
    expect(agents[0].status).toBe('busy');
  });

  it('never fabricates specialist/active agents from documents', () => {
    const data: AgentSwarmData = {
      nodes: [knowledgeNode(100), knowledgeNode(101)],
      edges: [],
    };
    const { agents, edges } = transformAgentData(data);
    expect(agents).toEqual([]);
    expect(edges).toEqual([]);
  });

  it('keeps only edges whose endpoints are both agents', () => {
    const data: AgentSwarmData = {
      nodes: [
        agentNode({ id: 1, metadataId: 'a1' }),
        agentNode({ id: 2, metadataId: 'a2' }),
        knowledgeNode(100),
      ],
      edges: [
        { id: 'e1', source: 1, target: 2, weight: 0.5 },
        { id: 'e2', source: 1, target: 100, weight: 0.5 },
      ],
    };
    const { edges } = transformAgentData(data);
    expect(edges).toHaveLength(1);
    expect(edges[0].id).toBe('e1');
    expect(edges[0].source).toBe('a1');
    expect(edges[0].target).toBe('a2');
  });

  it('tolerates a missing nodes array', () => {
    const { agents, edges } = transformAgentData({} as AgentSwarmData);
    expect(agents).toEqual([]);
    expect(edges).toEqual([]);
  });
});
