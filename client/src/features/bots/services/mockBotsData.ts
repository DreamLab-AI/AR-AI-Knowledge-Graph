import type { BotsAgent, BotsCommunication, BotsEdge, TokenUsage } from '../types/BotsTypes';

// UPDATED: Enhanced agent role categorization for claude-flow hive-mind system
export const AGENT_ROLES = {
  // Queen Role (Highest level - distinctive color)
  queen: ['queen'],
  // Meta & Coordinating Roles (Gold/Yellow)
  meta: ['coordinator', 'analyst', 'architect', 'optimizer', 'monitor'],
  // Primary Agent Roles (Green)
  primary: ['coder', 'tester', 'researcher', 'reviewer', 'documenter', 'specialist'],
  // UPDATED: Maestro specs-driven roles (Blue/Purple)
  maestro: ['requirements_analyst', 'design_architect', 'task_planner', 'implementation_coder', 'quality_reviewer', 'steering_documenter']
} as const;

// UPDATED: Enhanced mock agent templates with hive-mind properties
export const AGENT_TEMPLATES: Record<string, Partial<BotsAgent>> = {
  // UPDATED: Queen agent template - central coordinator
  queen: {
    type: 'queen',
    health: 100,
    cpuUsage: 35,
    memoryUsage: 40,
    workload: 0.8,
    capabilities: ['strategic_planning', 'resource_allocation', 'swarm_coordination', 'decision_making'],
    currentTask: 'Orchestrating multi-agent development swarm',
    tasksActive: 5,
    tasksCompleted: 127,
    successRate: 0.95,
    tokens: 125000,
    tokenRate: 320.5,
    activity: 0.9,
    agentMode: 'centralized'
  },
  coordinator: {
    type: 'coordinator',
    health: 95,
    cpuUsage: 45,
    memoryUsage: 38,
    workload: 0.6,
    capabilities: ['task_management', 'resource_allocation', 'team_coordination'],
    currentTask: 'Managing development tasks',
    tasksActive: 3,
    tasksCompleted: 89,
    successRate: 0.91,
    tokens: 45000,
    tokenRate: 120.3,
    activity: 0.85,
    agentMode: 'distributed'
  },
  analyst: {
    type: 'analyst',
    health: 92,
    cpuUsage: 55,
    memoryUsage: 42,
    workload: 0.7,
    capabilities: ['data_analysis', 'pattern_recognition', 'insights_generation'],
    currentTask: 'Analyzing system performance metrics',
    tasksActive: 2,
    tasksCompleted: 67,
    successRate: 0.88,
    tokens: 38000,
    tokenRate: 95.5,
    activity: 0.72,
    agentMode: 'distributed'
  },
  architect: {
    type: 'architect',
    health: 94,
    cpuUsage: 35,
    memoryUsage: 45,
    workload: 0.5,
    capabilities: ['system_architecture', 'design_patterns', 'integration_planning'],
    currentTask: 'Designing microservices architecture',
    tasksActive: 2,
    tasksCompleted: 54,
    successRate: 0.92,
    tokens: 42000,
    tokenRate: 105.8,
    activity: 0.68,
    agentMode: 'distributed'
  },
  optimizer: {
    type: 'optimizer',
    health: 90,
    cpuUsage: 65,
    memoryUsage: 55,
    workload: 0.8,
    capabilities: ['performance_optimization', 'resource_tuning', 'efficiency_analysis'],
    currentTask: 'Optimizing database queries',
    tasksActive: 3,
    tasksCompleted: 78,
    successRate: 0.86,
    tokens: 44000,
    tokenRate: 115.3,
    activity: 0.82,
    agentMode: 'distributed'
  },
  monitor: {
    type: 'monitor',
    health: 98,
    cpuUsage: 25,
    memoryUsage: 30,
    workload: 0.3,
    capabilities: ['system_monitoring', 'alerting', 'health_checks'],
    currentTask: 'Monitoring swarm health metrics',
    tasksActive: 1,
    tasksCompleted: 156,
    successRate: 0.99,
    tokens: 16000,
    tokenRate: 45.2,
    activity: 0.4,
    agentMode: 'distributed'
  },
  coder: {
    type: 'coder',
    health: 88,
    cpuUsage: 75,
    memoryUsage: 65,
    workload: 0.9,
    capabilities: ['code_generation', 'implementation', 'debugging'],
    currentTask: 'Implementing REST API endpoints',
    tasksActive: 4,
    tasksCompleted: 123,
    successRate: 0.87,
    tokens: 80000,
    tokenRate: 210.5,
    activity: 0.92,
    agentMode: 'distributed'
  },
  tester: {
    type: 'tester',
    health: 91,
    cpuUsage: 50,
    memoryUsage: 40,
    workload: 0.6,
    capabilities: ['unit_testing', 'integration_testing', 'test_automation'],
    currentTask: 'Writing unit tests for auth module',
    tasksActive: 2,
    tasksCompleted: 89,
    successRate: 0.93,
    tokens: 30000,
    tokenRate: 82.3,
    activity: 0.65,
    agentMode: 'distributed'
  },
  researcher: {
    type: 'researcher',
    health: 89,
    cpuUsage: 40,
    memoryUsage: 50,
    workload: 0.5,
    capabilities: ['information_gathering', 'analysis', 'documentation_research'],
    currentTask: 'Researching best practices for API design',
    tasksActive: 1,
    tasksCompleted: 45,
    successRate: 0.91,
    tokens: 70000,
    tokenRate: 185.7,
    activity: 0.58,
    agentMode: 'distributed'
  },
  reviewer: {
    type: 'reviewer',
    health: 93,
    cpuUsage: 30,
    memoryUsage: 35,
    workload: 0.4,
    capabilities: ['code_review', 'quality_assurance', 'best_practices_enforcement'],
    currentTask: 'Reviewing pull request #42',
    tasksActive: 1,
    tasksCompleted: 67,
    successRate: 0.95,
    tokens: 24000,
    tokenRate: 65.8,
    activity: 0.48,
    agentMode: 'distributed'
  },
  documenter: {
    type: 'documenter',
    health: 96,
    cpuUsage: 20,
    memoryUsage: 25,
    workload: 0.3,
    capabilities: ['technical_writing', 'api_documentation', 'user_guides'],
    currentTask: 'Updating API documentation',
    tasksActive: 1,
    tasksCompleted: 38,
    successRate: 0.97,
    tokens: 20000,
    tokenRate: 55.2,
    activity: 0.35,
    agentMode: 'distributed'
  },
  specialist: {
    type: 'specialist',
    health: 87,
    cpuUsage: 60,
    memoryUsage: 58,
    workload: 0.75,
    capabilities: ['domain_expertise', 'specialized_processing'],
    currentTask: 'Processing specialized domain logic',
    tasksActive: 2,
    tasksCompleted: 34,
    successRate: 0.88,
    tokens: 35000,
    tokenRate: 95.5,
    activity: 0.7,
    agentMode: 'distributed'
  },
  
  // UPDATED: Add Maestro specs-driven agent templates
  requirements_analyst: {
    type: 'requirements_analyst',
    health: 94,
    cpuUsage: 42,
    memoryUsage: 36,
    workload: 0.65,
    capabilities: ['requirements_gathering', 'stakeholder_analysis', 'specification_writing'],
    currentTask: 'Analyzing user authentication requirements',
    tasksActive: 2,
    tasksCompleted: 56,
    successRate: 0.92,
    tokens: 42000,
    tokenRate: 110.2,
    activity: 0.75,
    agentMode: 'strategic'
  },
  design_architect: {
    type: 'design_architect',
    health: 96,
    cpuUsage: 38,
    memoryUsage: 44,
    workload: 0.7,
    capabilities: ['system_design', 'architecture_patterns', 'specs_driven_design'],
    currentTask: 'Designing microservices architecture',
    tasksActive: 3,
    tasksCompleted: 67,
    successRate: 0.94,
    tokens: 58000,
    tokenRate: 140.8,
    activity: 0.8,
    agentMode: 'strategic'
  },
  task_planner: {
    type: 'task_planner',
    health: 93,
    cpuUsage: 35,
    memoryUsage: 32,
    workload: 0.55,
    capabilities: ['task_breakdown', 'scheduling', 'dependency_analysis'],
    currentTask: 'Planning sprint tasks',
    tasksActive: 4,
    tasksCompleted: 102,
    successRate: 0.9,
    tokens: 38000,
    tokenRate: 98.5,
    activity: 0.72,
    agentMode: 'strategic'
  },
  implementation_coder: {
    type: 'implementation_coder',
    health: 89,
    cpuUsage: 78,
    memoryUsage: 68,
    workload: 0.92,
    capabilities: ['code_generation', 'best_practices', 'refactoring', 'optimization'],
    currentTask: 'Implementing authentication service',
    tasksActive: 5,
    tasksCompleted: 178,
    successRate: 0.91,
    tokens: 95000,
    tokenRate: 250.5,
    activity: 0.95,
    agentMode: 'strategic'
  },
  quality_reviewer: {
    type: 'quality_reviewer',
    health: 97,
    cpuUsage: 32,
    memoryUsage: 30,
    workload: 0.45,
    capabilities: ['code_quality', 'testing_strategy', 'security_review'],
    currentTask: 'Reviewing API endpoints',
    tasksActive: 2,
    tasksCompleted: 89,
    successRate: 0.96,
    tokens: 32000,
    tokenRate: 85.3,
    activity: 0.68,
    agentMode: 'strategic'
  },
  steering_documenter: {
    type: 'steering_documenter',
    health: 98,
    cpuUsage: 25,
    memoryUsage: 28,
    workload: 0.4,
    capabilities: ['technical_writing', 'knowledge_base', 'api_documentation'],
    currentTask: 'Documenting architecture decisions',
    tasksActive: 1,
    tasksCompleted: 67,
    successRate: 0.98,
    tokens: 28000,
    tokenRate: 72.5,
    activity: 0.6,
    agentMode: 'strategic'
  }
};

// UPDATED: Generate enhanced mock agents with hive-mind hierarchy
export function generateMockAgents(): BotsAgent[] {
  const agents: BotsAgent[] = [];
  const now = Date.now();
  const swarmId = `swarm-${Date.now()}`;
  
  // UPDATED: Create Queen agent first (always 1, at center)
  const queenTemplate = AGENT_TEMPLATES.queen;
  const queenAgent: BotsAgent = {
    id: 'queen-alpha',
    name: 'Queen Alpha',
    type: 'queen',
    status: 'active',
    health: queenTemplate.health!,
    cpuUsage: queenTemplate.cpuUsage!,
    memoryUsage: queenTemplate.memoryUsage!,
    workload: queenTemplate.workload!,
    createdAt: new Date(now - 7200000).toISOString(), // 2 hours ago
    age: 7200000,
    position: { x: 0, y: 0, z: 0 }, // Central position for Queen
    velocity: { x: 0, y: 0, z: 0 },
    // Hive-mind properties
    capabilities: queenTemplate.capabilities,
    currentTask: queenTemplate.currentTask,
    tasksActive: queenTemplate.tasksActive,
    tasksCompleted: queenTemplate.tasksCompleted,
    successRate: queenTemplate.successRate,
    tokens: queenTemplate.tokens,
    tokenRate: queenTemplate.tokenRate,
    activity: queenTemplate.activity,
    swarmId: swarmId,
    agentMode: queenTemplate.agentMode,
    parentQueenId: undefined // Queen has no parent
  };
  agents.push(queenAgent);
  
  // Create coordinator (reports to Queen)
  const coordTemplate = AGENT_TEMPLATES.coordinator;
  agents.push({
    id: 'coordinator-1',
    name: 'Swarm Coordinator Beta',
    type: 'coordinator',
    status: 'busy',
    health: coordTemplate.health!,
    cpuUsage: coordTemplate.cpuUsage!,
    memoryUsage: coordTemplate.memoryUsage!,
    workload: coordTemplate.workload!,
    createdAt: new Date(now - 3600000).toISOString(), // 1 hour ago
    age: 3600000,
    position: { x: 10, y: 0, z: 0 }, // Near Queen
    velocity: { x: 0, y: 0, z: 0 },
    // Hive-mind properties
    capabilities: coordTemplate.capabilities,
    currentTask: coordTemplate.currentTask,
    tasksActive: coordTemplate.tasksActive,
    tasksCompleted: coordTemplate.tasksCompleted,
    successRate: coordTemplate.successRate,
    tokens: coordTemplate.tokens,
    tokenRate: coordTemplate.tokenRate,
    activity: coordTemplate.activity,
    swarmId: swarmId,
    agentMode: coordTemplate.agentMode,
    parentQueenId: 'queen-alpha'
  });

  // UPDATED: Create Maestro specs-driven agents (inner circle around Queen)
  const maestroAgents = ['requirements_analyst', 'design_architect', 'task_planner', 'implementation_coder', 'quality_reviewer', 'steering_documenter'];
  maestroAgents.forEach((type, index) => {
    const template = AGENT_TEMPLATES[type];
    if (!template) return;
    
    agents.push({
      id: `${type}-1`,
      name: `${type.split('_').map(s => s.charAt(0).toUpperCase() + s.slice(1)).join(' ')}`,
      type: template.type!,
      status: Math.random() > 0.3 ? 'active' : 'busy',
      health: template.health! + (Math.random() - 0.5) * 5,
      cpuUsage: template.cpuUsage! + (Math.random() - 0.5) * 10,
      memoryUsage: template.memoryUsage! + (Math.random() - 0.5) * 8,
      workload: Math.max(0, Math.min(1, template.workload! + (Math.random() - 0.5) * 0.15)),
      createdAt: new Date(now - Math.random() * 5400000).toISOString(),
      age: Math.floor(Math.random() * 5400000),
      position: {
        x: Math.cos((index / maestroAgents.length) * Math.PI * 2) * 20,
        y: Math.sin((index / maestroAgents.length) * Math.PI * 2) * 20,
        z: (Math.random() - 0.5) * 5
      },
      velocity: { x: 0, y: 0, z: 0 },
      // Hive-mind properties
      capabilities: template.capabilities,
      currentTask: template.currentTask,
      tasksActive: template.tasksActive,
      tasksCompleted: template.tasksCompleted,
      successRate: template.successRate,
      tokens: template.tokens,
      tokenRate: template.tokenRate,
      activity: template.activity,
      swarmId: swarmId,
      agentMode: template.agentMode,
      parentQueenId: 'queen-alpha'
    });
  });
  
  // Create other meta agents (middle circle)
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
        x: Math.cos((index / metaAgents.length) * Math.PI * 2) * 35,
        y: Math.sin((index / metaAgents.length) * Math.PI * 2) * 35,
        z: (Math.random() - 0.5) * 10
      },
      velocity: { x: 0, y: 0, z: 0 },
      // Basic hive-mind properties for non-maestro agents
      capabilities: ['general_processing'],
      currentTask: 'Processing assigned tasks',
      tasksActive: Math.floor(Math.random() * 3),
      tasksCompleted: Math.floor(Math.random() * 50) + 20,
      successRate: 0.8 + Math.random() * 0.15,
      tokens: Math.floor(Math.random() * 30000) + 20000,
      tokenRate: 50 + Math.random() * 100,
      activity: Math.random() * 0.8 + 0.2,
      swarmId: swarmId,
      agentMode: 'distributed',
      parentQueenId: 'queen-alpha'
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
      // UPDATED: Position primary agents in outer circle
      const angle = (agentIndex / 10) * Math.PI * 2;
      const radius = 50 + Math.random() * 20;
      
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
          x: Math.cos(angle) * radius,
          y: Math.sin(angle) * radius,
          z: (Math.random() - 0.5) * 20
        },
        velocity: {
          x: (Math.random() - 0.5) * 0.5,
          y: (Math.random() - 0.5) * 0.5,
          z: (Math.random() - 0.5) * 0.3
        },
        // UPDATED: Add hive-mind properties for primary agents
        capabilities: template.capabilities || ['general_processing'],
        currentTask: Math.random() > 0.5 ? template.currentTask || 'Processing tasks' : undefined,
        tasksActive: Math.floor(Math.random() * 4),
        tasksCompleted: Math.floor(Math.random() * 80) + 10,
        successRate: 0.75 + Math.random() * 0.2,
        tokens: Math.floor(Math.random() * 50000) + 15000,
        tokenRate: 40 + Math.random() * 120,
        activity: Math.random() * 0.9 + 0.1,
        swarmId: swarmId,
        agentMode: 'distributed',
        parentQueenId: 'queen-alpha'
      });
      agentIndex++;
    }
  });

  return agents;
}

// UPDATED: Generate enhanced communication patterns with Queen-centric hierarchy
export function generateMockCommunications(agents: BotsAgent[]): BotsCommunication[] {
  const communications: BotsCommunication[] = [];
  const now = Date.now();
  
  // UPDATED: Queen broadcasts strategic directives to all agents
  const queen = agents.find(a => a.type === 'queen');
  if (queen) {
    communications.push({
      id: `comm-queen-directive-${now}`,
      type: 'communication',
      timestamp: new Date(now).toISOString(),
      sender: queen.id,
      receivers: agents.filter(a => a.id !== queen.id).map(a => a.id),
      metadata: {
        size: 4096,
        type: 'strategic_directive'
      }
    });
  }
  
  // Coordinator broadcasts to worker agents
  const coordinator = agents.find(a => a.type === 'coordinator');
  if (coordinator) {
    const workers = agents.filter(a => 
      ['coder', 'tester', 'researcher', 'reviewer', 'documenter', 'specialist'].includes(a.type)
    );
    communications.push({
      id: `comm-broadcast-${now}`,
      type: 'communication',
      timestamp: new Date(now - 1000).toISOString(),
      sender: coordinator.id,
      receivers: workers.map(a => a.id),
      metadata: {
        size: 2048,
        type: 'task_assignment'
      }
    });
  }

  // UPDATED: Enhanced communication patterns including Maestro agents
  const patterns = [
    // Queen coordination patterns
    { from: 'queen', to: ['coordinator', 'design_architect'], type: 'strategic_plan', size: 12288 },
    { from: 'coordinator', to: ['queen'], type: 'status_report', size: 6144 },
    
    // Maestro workflow patterns
    { from: 'requirements_analyst', to: ['design_architect', 'queen'], type: 'requirements_spec', size: 10240 },
    { from: 'design_architect', to: ['implementation_coder', 'task_planner'], type: 'architecture_design', size: 16384 },
    { from: 'task_planner', to: ['coordinator', 'implementation_coder'], type: 'task_breakdown', size: 8192 },
    { from: 'implementation_coder', to: ['quality_reviewer', 'tester'], type: 'code_complete', size: 20480 },
    { from: 'quality_reviewer', to: ['implementation_coder', 'steering_documenter'], type: 'review_results', size: 6144 },
    { from: 'steering_documenter', to: ['queen', 'architect'], type: 'documentation_update', size: 12288 },
    
    // Traditional patterns
    { from: 'architect', to: ['coder'], type: 'design_spec', size: 8192 },
    { from: 'coder', to: ['tester'], type: 'code_ready', size: 4096 },
    { from: 'tester', to: ['coder', 'reviewer'], type: 'test_results', size: 2048 },
    { from: 'researcher', to: ['architect', 'analyst'], type: 'research_findings', size: 16384 },
    { from: 'analyst', to: ['optimizer'], type: 'performance_metrics', size: 4096 },
    { from: 'monitor', to: ['coordinator', 'queen'], type: 'health_status', size: 1024 },
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

  // UPDATED: Add hierarchical persistent connections centered on Queen
  const persistentConnections = [
    // Queen connections (hub of the swarm)
    { source: 'queen-alpha', targets: ['coordinator-1', 'design_architect-1', 'requirements_analyst-1'] },
    { source: 'coordinator-1', targets: ['analyst-1', 'architect-1', 'monitor-1', 'task_planner-1'] },
    
    // Maestro workflow connections
    { source: 'requirements_analyst-1', targets: ['design_architect-1', 'task_planner-1'] },
    { source: 'design_architect-1', targets: ['implementation_coder-1', 'architect-1'] },
    { source: 'task_planner-1', targets: ['implementation_coder-1', 'coder-1', 'coder-2'] },
    { source: 'implementation_coder-1', targets: ['quality_reviewer-1', 'tester-1'] },
    { source: 'quality_reviewer-1', targets: ['steering_documenter-1', 'reviewer-1'] },
    
    // Traditional connections
    { source: 'architect-1', targets: ['coder-1', 'coder-2', 'coder-3'] },
    { source: 'coder-1', targets: ['tester-1', 'reviewer-1'] },
    { source: 'analyst-1', targets: ['optimizer-1', 'researcher-1'] },
    { source: 'monitor-1', targets: ['coordinator-1', 'queen-alpha'] }
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
  
  // UPDATED: Token usage based on agent type and workload (including hive-mind agents)
  const tokenMultipliers: Record<string, number> = {
    queen: 5000, // Highest token usage
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
    specialist: 2800,
    // Maestro agents
    requirements_analyst: 2800,
    design_architect: 3200,
    task_planner: 2400,
    implementation_coder: 4500,
    quality_reviewer: 2000,
    steering_documenter: 1800
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