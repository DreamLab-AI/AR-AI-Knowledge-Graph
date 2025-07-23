import type { BotsAgent, BotsCommunication, BotsEdge, TokenUsage } from '../types/BotsTypes';

// Agent role categorization based on visualization requirements
export const AGENT_ROLES = {
  // Meta & Coordinating Roles (Gold/Yellow)
  meta: ['coordinator', 'analyst', 'architect', 'optimizer', 'monitor'],
  // Primary Agent Roles (Green)
  primary: ['coder', 'tester', 'researcher', 'reviewer', 'documenter', 'specialist']
} as const;

// Mock agent templates with realistic properties
export const AGENT_TEMPLATES: Record<string, Partial<BotsAgent>> = {
  coordinator: {
    type: 'coordinator',
    health: 95,
    cpuUsage: 45,
    memoryUsage: 38,
    workload: 0.6
  },
  analyst: {
    type: 'analyst',
    health: 92,
    cpuUsage: 55,
    memoryUsage: 42,
    workload: 0.7
  },
  architect: {
    type: 'architect',
    health: 94,
    cpuUsage: 35,
    memoryUsage: 45,
    workload: 0.5
  },
  optimizer: {
    type: 'optimizer',
    health: 90,
    cpuUsage: 65,
    memoryUsage: 55,
    workload: 0.8
  },
  monitor: {
    type: 'monitor',
    health: 98,
    cpuUsage: 25,
    memoryUsage: 30,
    workload: 0.3
  },
  coder: {
    type: 'coder',
    health: 88,
    cpuUsage: 75,
    memoryUsage: 65,
    workload: 0.9
  },
  tester: {
    type: 'tester',
    health: 91,
    cpuUsage: 50,
    memoryUsage: 40,
    workload: 0.6
  },
  researcher: {
    type: 'researcher',
    health: 89,
    cpuUsage: 40,
    memoryUsage: 50,
    workload: 0.5
  },
  reviewer: {
    type: 'reviewer',
    health: 93,
    cpuUsage: 30,
    memoryUsage: 35,
    workload: 0.4
  },
  documenter: {
    type: 'documenter',
    health: 96,
    cpuUsage: 20,
    memoryUsage: 25,
    workload: 0.3
  },
  specialist: {
    type: 'specialist',
    health: 87,
    cpuUsage: 60,
    memoryUsage: 58,
    workload: 0.75
  }
};

// Generate mock agents with realistic distribution
export function generateMockAgents(): BotsAgent[] {
  const agents: BotsAgent[] = [];
  const now = Date.now();
  
  // Create coordinator (always 1)
  agents.push({
    id: 'coordinator-1',
    name: 'Swarm Coordinator Alpha',
    type: 'coordinator',
    status: 'busy',
    health: 95,
    cpuUsage: 45,
    memoryUsage: 38,
    workload: 0.6,
    createdAt: new Date(now - 3600000).toISOString(), // 1 hour ago
    age: 3600000,
    position: { x: 0, y: 0, z: 0 }, // Central position
    velocity: { x: 0, y: 0, z: 0 }
  });

  // Create other meta agents
  const metaAgents = ['analyst', 'architect', 'optimizer', 'monitor'];
  metaAgents.forEach((type, index) => {
    const template = AGENT_TEMPLATES[type];
    agents.push({
      id: `${type}-${index + 1}`,
      name: `${type.charAt(0).toUpperCase() + type.slice(1)} Agent ${index + 1}`,
      type: template.type!,
      status: Math.random() > 0.7 ? 'busy' : 'idle',
      health: template.health! + (Math.random() - 0.5) * 10,
      cpuUsage: template.cpuUsage! + (Math.random() - 0.5) * 15,
      memoryUsage: template.memoryUsage! + (Math.random() - 0.5) * 10,
      workload: Math.max(0, Math.min(1, template.workload! + (Math.random() - 0.5) * 0.2)),
      createdAt: new Date(now - Math.random() * 7200000).toISOString(),
      age: Math.floor(Math.random() * 7200000),
      position: {
        x: Math.cos((index / metaAgents.length) * Math.PI * 2) * 15,
        y: Math.sin((index / metaAgents.length) * Math.PI * 2) * 15,
        z: (Math.random() - 0.5) * 10
      },
      velocity: { x: 0, y: 0, z: 0 }
    });
  });

  // Create primary agents (multiple of each type)
  const primaryAgents = [
    { type: 'coder', count: 3 },
    { type: 'tester', count: 2 },
    { type: 'researcher', count: 2 },
    { type: 'reviewer', count: 1 },
    { type: 'documenter', count: 1 },
    { type: 'specialist', count: 1 }
  ];

  let agentIndex = 0;
  primaryAgents.forEach(({ type, count }) => {
    for (let i = 0; i < count; i++) {
      const template = AGENT_TEMPLATES[type];
      agents.push({
        id: `${type}-${i + 1}`,
        name: `${type.charAt(0).toUpperCase() + type.slice(1)} ${i + 1}`,
        type: template.type!,
        status: Math.random() > 0.5 ? 'busy' : (Math.random() > 0.8 ? 'error' : 'idle'),
        health: template.health! + (Math.random() - 0.5) * 15,
        cpuUsage: template.cpuUsage! + (Math.random() - 0.5) * 20,
        memoryUsage: template.memoryUsage! + (Math.random() - 0.5) * 15,
        workload: Math.max(0, Math.min(1, template.workload! + (Math.random() - 0.5) * 0.3)),
        createdAt: new Date(now - Math.random() * 10800000).toISOString(),
        age: Math.floor(Math.random() * 10800000),
        position: {
          x: (Math.random() - 0.5) * 40,
          y: (Math.random() - 0.5) * 40,
          z: (Math.random() - 0.5) * 30
        },
        velocity: {
          x: (Math.random() - 0.5) * 0.5,
          y: (Math.random() - 0.5) * 0.5,
          z: (Math.random() - 0.5) * 0.3
        }
      });
      agentIndex++;
    }
  });

  return agents;
}

// Generate communication patterns between agents
export function generateMockCommunications(agents: BotsAgent[]): BotsCommunication[] {
  const communications: BotsCommunication[] = [];
  const now = Date.now();
  
  // Coordinator broadcasts to all agents periodically
  const coordinator = agents.find(a => a.type === 'coordinator');
  if (coordinator) {
    communications.push({
      id: `comm-broadcast-${now}`,
      type: 'communication',
      timestamp: new Date(now).toISOString(),
      sender: coordinator.id,
      receivers: agents.filter(a => a.id !== coordinator.id).map(a => a.id),
      metadata: {
        size: 2048,
        type: 'coordination_update'
      }
    });
  }

  // Create realistic communication patterns
  const patterns = [
    { from: 'architect', to: ['coder'], type: 'design_spec', size: 8192 },
    { from: 'coder', to: ['tester'], type: 'code_ready', size: 4096 },
    { from: 'tester', to: ['coder', 'reviewer'], type: 'test_results', size: 2048 },
    { from: 'researcher', to: ['architect', 'analyst'], type: 'research_findings', size: 16384 },
    { from: 'analyst', to: ['optimizer'], type: 'performance_metrics', size: 4096 },
    { from: 'monitor', to: ['coordinator'], type: 'health_status', size: 1024 },
    { from: 'reviewer', to: ['coder', 'documenter'], type: 'review_feedback', size: 3072 },
    { from: 'documenter', to: ['specialist'], type: 'doc_update', size: 2048 }
  ];

  // Generate communications based on patterns
  patterns.forEach((pattern, index) => {
    const sender = agents.find(a => a.type === pattern.from);
    const receivers = agents.filter(a => pattern.to.includes(a.type));
    
    if (sender && receivers.length > 0 && Math.random() > 0.5) {
      communications.push({
        id: `comm-pattern-${now}-${index}`,
        type: 'communication',
        timestamp: new Date(now - Math.random() * 5000).toISOString(),
        sender: sender.id,
        receivers: receivers.map(r => r.id),
        metadata: {
          size: pattern.size + Math.floor(Math.random() * 1024),
          type: pattern.type
        }
      });
    }
  });

  return communications;
}

// Generate edges based on communication patterns
export function generateMockEdges(agents: BotsAgent[], communications: BotsCommunication[]): BotsEdge[] {
  const edgeMap = new Map<string, BotsEdge>();
  
  // Build edges from communications
  communications.forEach(comm => {
    comm.receivers.forEach(receiver => {
      const edgeId = `${comm.sender}-${receiver}`;
      const reverseEdgeId = `${receiver}-${comm.sender}`;
      
      // Use bidirectional edge ID (alphabetically sorted)
      const sortedId = [comm.sender, receiver].sort().join('-');
      
      if (edgeMap.has(sortedId)) {
        const edge = edgeMap.get(sortedId)!;
        edge.dataVolume += comm.metadata.size;
        edge.messageCount += 1;
        edge.lastMessageTime = Date.now();
      } else {
        edgeMap.set(sortedId, {
          id: sortedId,
          source: comm.sender,
          target: receiver,
          dataVolume: comm.metadata.size,
          messageCount: 1,
          lastMessageTime: Date.now()
        });
      }
    });
  });

  // Add some persistent connections (common communication paths)
  const persistentConnections = [
    { source: 'coordinator-1', targets: ['analyst-1', 'architect-1', 'monitor-1'] },
    { source: 'architect-1', targets: ['coder-1', 'coder-2'] },
    { source: 'coder-1', targets: ['tester-1', 'reviewer-1'] },
    { source: 'analyst-1', targets: ['optimizer-1', 'researcher-1'] }
  ];

  persistentConnections.forEach(conn => {
    const source = agents.find(a => a.id === conn.source);
    if (source) {
      conn.targets.forEach(targetId => {
        const target = agents.find(a => a.id === targetId);
        if (target) {
          const sortedId = [source.id, target.id].sort().join('-');
          if (!edgeMap.has(sortedId)) {
            edgeMap.set(sortedId, {
              id: sortedId,
              source: source.id,
              target: target.id,
              dataVolume: Math.floor(Math.random() * 10000) + 1000,
              messageCount: Math.floor(Math.random() * 10) + 1,
              lastMessageTime: Date.now() - Math.random() * 60000
            });
          }
        }
      });
    }
  });

  return Array.from(edgeMap.values());
}

// Generate token usage data
export function generateMockTokenUsage(agents: BotsAgent[]): TokenUsage {
  const byAgent: Record<string, number> = {};
  let total = 0;
  
  // Token usage based on agent type and workload
  const tokenMultipliers: Record<string, number> = {
    coordinator: 3000,
    analyst: 2500,
    architect: 2000,
    optimizer: 2200,
    monitor: 800,
    coder: 4000,
    tester: 1500,
    researcher: 3500,
    reviewer: 1200,
    documenter: 1000,
    specialist: 2800
  };

  agents.forEach(agent => {
    const baseTokens = tokenMultipliers[agent.type] || 1000;
    const tokens = Math.floor(baseTokens * agent.workload! * (0.8 + Math.random() * 0.4));
    byAgent[agent.type] = (byAgent[agent.type] || 0) + tokens;
    total += tokens;
  });

  return { total, byAgent };
}

// Dynamic update functions for animation
export function updateAgentMetrics(agent: BotsAgent, deltaTime: number): void {
  // Update CPU usage with smooth variation
  const cpuTarget = agent.status === 'busy' ? 70 + Math.random() * 20 : 20 + Math.random() * 20;
  agent.cpuUsage += (cpuTarget - agent.cpuUsage) * 0.1;

  // Update memory usage
  const memTarget = agent.status === 'busy' ? 50 + Math.random() * 30 : 20 + Math.random() * 20;
  agent.memoryUsage += (memTarget - agent.memoryUsage) * 0.05;

  // Update workload
  if (agent.status === 'busy') {
    agent.workload = Math.min(1, agent.workload! + Math.random() * 0.1);
  } else {
    agent.workload = Math.max(0, agent.workload! - Math.random() * 0.1);
  }

  // Update health based on workload and metrics
  const stress = (agent.cpuUsage + agent.memoryUsage) / 200;
  const healthTarget = 100 - (stress * 30);
  agent.health += (healthTarget - agent.health) * 0.02;

  // Update age
  agent.age += deltaTime;

  // Occasionally change status
  if (Math.random() < 0.01) { // 1% chance per update
    const statuses: Array<BotsAgent['status']> = ['idle', 'busy', 'error'];
    const weights = agent.type === 'monitor' ? [0.7, 0.3, 0] : [0.3, 0.6, 0.1];
    const random = Math.random();
    let cumulative = 0;
    for (let i = 0; i < statuses.length; i++) {
      cumulative += weights[i];
      if (random < cumulative) {
        agent.status = statuses[i];
        break;
      }
    }
  }
}

// Create complete mock data state
export function createMockBotsState() {
  const agents = generateMockAgents();
  const communications = generateMockCommunications(agents);
  const edges = generateMockEdges(agents, communications);
  const tokenUsage = generateMockTokenUsage(agents);

  return {
    agents: new Map(agents.map(a => [a.id, a])),
    edges: new Map(edges.map(e => [e.id, e])),
    communications,
    tokenUsage,
    lastUpdate: Date.now()
  };
}

// Export mock data for testing
export const MOCK_BOTS_DATA = {
  agents: generateMockAgents(),
  createState: createMockBotsState,
  updateAgent: updateAgentMetrics,
  generateCommunications: generateMockCommunications,
  generateEdges: generateMockEdges,
  generateTokenUsage: generateMockTokenUsage
};