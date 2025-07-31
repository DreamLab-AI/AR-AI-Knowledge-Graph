import { BotsAgent, BotsEdge, TokenUsage } from '../types/BotsTypes';

/**
 * Mock Data Generator for Bots Visualization
 * Creates realistic swarm agent data for development and testing
 */
export class MockDataGenerator {
  private static instance: MockDataGenerator;
  private agents: Map<string, BotsAgent> = new Map();
  private edges: Map<string, BotsEdge> = new Map();
  private updateInterval: NodeJS.Timer | null = null;

  private constructor() {}

  static getInstance(): MockDataGenerator {
    if (!MockDataGenerator.instance) {
      MockDataGenerator.instance = new MockDataGenerator();
    }
    return MockDataGenerator.instance;
  }

  // UPDATED: Helper method to generate realistic current tasks based on agent type
  private generateCurrentTask(agentType: string): string {
    const taskTemplates: Record<string, string[]> = {
      queen: [
        'Coordinating full-stack development swarm',
        'Strategic planning for microservices architecture',
        'Resource allocation for AI training pipeline',
        'Quality control review across all teams'
      ],
      coordinator: [
        'Managing React component development',
        'Coordinating API integration tasks',
        'Synchronizing team workflows',
        'Orchestrating deployment pipeline'
      ],
      researcher: [
        'Analyzing latest AI safety research',
        'Investigating performance optimization techniques',
        'Researching blockchain scalability solutions',
        'Studying neural network architectures'
      ],
      coder: [
        'Implementing JWT authentication service',
        'Developing REST API endpoints',
        'Refactoring legacy database queries',
        'Building real-time WebSocket handlers'
      ],
      requirements_analyst: [
        'Analyzing user authentication requirements',
        'Defining acceptance criteria for payments',
        'Gathering stakeholder requirements',
        'Creating user story specifications'
      ],
      implementation_coder: [
        'Implementing JWT authentication service',
        'Building microservice APIs',
        'Developing frontend components',
        'Integrating third-party services'
      ],
      design_architect: [
        'Designing microservices architecture',
        'Planning system integration patterns',
        'Architecting distributed database schema',
        'Defining API gateway configuration'
      ]
    };

    const templates = taskTemplates[agentType] || [
      'Processing assigned tasks',
      'Analyzing system requirements',
      'Optimizing performance metrics',
      'Coordinating with team members'
    ];

    return templates[Math.floor(Math.random() * templates.length)];
  }

  /**
   * Initialize mock data with specified number of agents
   */
  initialize(agentCount: number = 12): void {
    this.createAgents(agentCount);
    this.createEdges();
    this.startSimulation();
  }

  /**
   * UPDATED: Create mock agents with enhanced hive-mind types and hierarchical structure
   */
  private createAgents(count: number): void {
    // UPDATED: Include all hive-mind agent types including Maestro specs-driven types
    const agentTypes: BotsAgent['type'][] = [
      'queen', 'coordinator', 'researcher', 'coder', 'analyst', 'architect', 'tester', 
      'reviewer', 'optimizer', 'documenter', 'monitor', 'specialist',
      'requirements_analyst', 'design_architect', 'task_planner', 
      'implementation_coder', 'quality_reviewer', 'steering_documenter'
    ];

    const statuses: BotsAgent['status'][] = ['idle', 'busy', 'active', 'active', 'busy']; // More active agents
    
    // UPDATED: Capability mappings for each agent type
    const capabilityMap: Record<string, string[]> = {
      queen: ['strategic_planning', 'resource_allocation', 'coordination', 'quality_control'],
      coordinator: ['task_management', 'team_coordination', 'workflow_optimization'],
      researcher: ['information_gathering', 'pattern_recognition', 'knowledge_synthesis'],
      coder: ['code_generation', 'refactoring', 'debugging'],
      analyst: ['data_analysis', 'performance_metrics', 'bottleneck_detection'],
      architect: ['system_design', 'architecture_patterns', 'integration_planning'],
      tester: ['test_generation', 'quality_assurance', 'edge_case_detection'],
      reviewer: ['code_review', 'standards_enforcement', 'best_practices'],
      optimizer: ['performance_optimization', 'resource_optimization', 'algorithm_improvement'],
      documenter: ['documentation_generation', 'api_docs', 'user_guides'],
      monitor: ['system_monitoring', 'health_checks', 'alerting'],
      specialist: ['domain_expertise', 'custom_capabilities', 'problem_solving'],
      requirements_analyst: ['requirements_analysis', 'user_story_creation', 'acceptance_criteria'],
      design_architect: ['specs_driven_design', 'workflow_orchestration', 'architecture'],
      task_planner: ['task_management', 'resource_allocation', 'scheduling'],
      implementation_coder: ['code_generation', 'api_development', 'integration'],
      quality_reviewer: ['quality_assurance', 'compliance', 'validation'],
      steering_documenter: ['governance', 'documentation_generation', 'standards']
    };

    // UPDATED: Create Queen agent first (always agent-0)
    let queenId: string | undefined;
    
    for (let i = 0; i < count; i++) {
      let agentType: BotsAgent['type'];
      let parentQueenId: string | undefined;
      
      // First agent is always Queen
      if (i === 0) {
        agentType = 'queen';
        queenId = `agent-${i}`;
      } else {
        // Other agents get random types (excluding queen)
        const nonQueenTypes = agentTypes.filter(t => t !== 'queen');
        agentType = nonQueenTypes[i % nonQueenTypes.length];
        parentQueenId = queenId;
      }
      
      const status = statuses[Math.floor(Math.random() * statuses.length)];
      const capabilities = capabilityMap[agentType] || ['general_capabilities'];
      
      const agent: BotsAgent = {
        id: `agent-${i}`,
        type: agentType,
        status: status,
        health: 70 + Math.random() * 30, // 70-100%
        cpuUsage: status === 'busy' ? 40 + Math.random() * 50 : 5 + Math.random() * 25,
        memoryUsage: 20 + Math.random() * 60,
        workload: status === 'busy' ? 0.5 + Math.random() * 0.5 : Math.random() * 0.3,
        createdAt: new Date(Date.now() - Math.random() * 3600000).toISOString(),
        
        // UPDATED: Enhanced hive-mind properties
        capabilities: capabilities,
        currentTask: status === 'active' || status === 'busy' ? this.generateCurrentTask(agentType) : undefined,
        tasksActive: status === 'active' || status === 'busy' ? Math.floor(Math.random() * 5) + 1 : 0,
        tasksCompleted: Math.floor(Math.random() * 100) + 10,
        successRate: 0.75 + Math.random() * 0.25, // 75-100% success rate
        tokens: Math.floor(Math.random() * 50000) + 1000,
        tokenRate: Math.random() * 20 + 2, // 2-22 tokens per minute
        activity: status === 'active' ? 0.7 + Math.random() * 0.3 : Math.random() * 0.6,
        swarmId: 'hive-mind-mock-001',
        agentMode: agentType === 'queen' ? 'centralized' : 
                  ['hierarchical', 'distributed', 'strategic'][Math.floor(Math.random() * 3)],
        parentQueenId: parentQueenId,
        age: Math.random() * 3600000,
        name: `${agentType.charAt(0).toUpperCase() + agentType.slice(1)}-${i}`,
        position: {
          x: (Math.random() - 0.5) * 40,
          y: (Math.random() - 0.5) * 40,
          z: (Math.random() - 0.5) * 20
        },
        velocity: {
          x: 0,
          y: 0,
          z: 0
        }
      };

      this.agents.set(agent.id, agent);
    }
  }

  /**
   * Create edges between agents based on their types
   */
  private createEdges(): void {
    const agentArray = Array.from(this.agents.values());
    
    // Coordinator connects to everyone
    const coordinators = agentArray.filter(a => a.type === 'coordinator');
    coordinators.forEach(coordinator => {
      agentArray.forEach(agent => {
        if (agent.id !== coordinator.id) {
          this.createEdge(coordinator.id, agent.id, true);
        }
      });
    });

    // Coders connect to testers and reviewers
    const coders = agentArray.filter(a => a.type === 'coder');
    const testers = agentArray.filter(a => a.type === 'tester');
    const reviewers = agentArray.filter(a => a.type === 'reviewer');

    coders.forEach(coder => {
      // Each coder connects to a tester
      if (testers.length > 0) {
        const tester = testers[Math.floor(Math.random() * testers.length)];
        this.createEdge(coder.id, tester.id);
      }
      // And to a reviewer
      if (reviewers.length > 0) {
        const reviewer = reviewers[Math.floor(Math.random() * reviewers.length)];
        this.createEdge(coder.id, reviewer.id);
      }
    });

    // Analysts connect to researchers
    const analysts = agentArray.filter(a => a.type === 'analyst');
    const researchers = agentArray.filter(a => a.type === 'researcher');

    analysts.forEach(analyst => {
      researchers.forEach(researcher => {
        this.createEdge(analyst.id, researcher.id);
      });
    });
  }

  /**
   * Create an edge between two agents
   */
  private createEdge(sourceId: string, targetId: string, highTraffic: boolean = false): void {
    const edgeId = [sourceId, targetId].sort().join('-');
    
    if (!this.edges.has(edgeId)) {
      this.edges.set(edgeId, {
        id: edgeId,
        source: sourceId,
        target: targetId,
        dataVolume: highTraffic ? 1000 + Math.random() * 4000 : Math.random() * 1000,
        messageCount: highTraffic ? 10 + Math.floor(Math.random() * 20) : Math.floor(Math.random() * 10),
        lastMessageTime: Date.now() - Math.random() * 5000
      });
    }
  }

  /**
   * Start real-time simulation
   */
  private startSimulation(): void {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
    }

    this.updateInterval = setInterval(() => {
      this.updateAgentStates();
      this.updateEdgeActivity();
    }, 1000);
  }

  /**
   * Update agent states dynamically
   */
  private updateAgentStates(): void {
    this.agents.forEach(agent => {
      // Random state changes
      if (Math.random() < 0.1) {
        const statuses: BotsAgent['status'][] = ['idle', 'busy'];
        agent.status = statuses[Math.floor(Math.random() * statuses.length)];
      }

      // Update CPU based on status
      if (agent.status === 'busy') {
        agent.cpuUsage = Math.min(95, agent.cpuUsage + (Math.random() - 0.3) * 10);
      } else {
        agent.cpuUsage = Math.max(5, agent.cpuUsage - Math.random() * 5);
      }

      // Update health (slowly)
      agent.health = Math.max(50, Math.min(100, agent.health + (Math.random() - 0.5) * 2));

      // Update workload
      agent.workload = agent.status === 'busy' ? 
        Math.min(1, agent.workload + Math.random() * 0.1) :
        Math.max(0, agent.workload - Math.random() * 0.1);

      // Update age
      agent.age += 1000;
    });
  }

  /**
   * Update edge activity
   */
  private updateEdgeActivity(): void {
    this.edges.forEach(edge => {
      // Active edges have recent communication
      if (Math.random() < 0.3) {
        edge.lastMessageTime = Date.now();
        edge.messageCount += 1;
        edge.dataVolume += Math.random() * 500;
      }
    });
  }

  /**
   * Get current agents
   */
  getAgents(): BotsAgent[] {
    return Array.from(this.agents.values());
  }

  /**
   * Get current edges
   */
  getEdges(): BotsEdge[] {
    return Array.from(this.edges.values());
  }

  /**
   * Get token usage mock data
   */
  getTokenUsage(): TokenUsage {
    const usage: TokenUsage = {
      total: 0,
      byAgent: {}
    };

    this.agents.forEach(agent => {
      const tokens = agent.status === 'busy' ? 
        1000 + Math.floor(Math.random() * 4000) :
        100 + Math.floor(Math.random() * 500);
      
      usage.byAgent[agent.type] = (usage.byAgent[agent.type] || 0) + tokens;
      usage.total += tokens;
    });

    return usage;
  }

  /**
   * Add a new agent dynamically
   */
  addAgent(type: BotsAgent['type']): BotsAgent {
    const id = `agent-${this.agents.size}`;
    const agent: BotsAgent = {
      id,
      type,
      status: 'initializing',
      health: 100,
      cpuUsage: 10,
      memoryUsage: 20,
      workload: 0,
      createdAt: new Date().toISOString(),
      age: 0,
      name: `${type.charAt(0).toUpperCase() + type.slice(1)}-${this.agents.size}`,
      position: {
        x: (Math.random() - 0.5) * 40,
        y: (Math.random() - 0.5) * 40,
        z: (Math.random() - 0.5) * 20
      },
      velocity: { x: 0, y: 0, z: 0 }
    };

    this.agents.set(id, agent);

    // Connect to coordinator
    const coordinator = Array.from(this.agents.values()).find(a => a.type === 'coordinator');
    if (coordinator) {
      this.createEdge(coordinator.id, id);
    }

    return agent;
  }

  /**
   * Remove an agent
   */
  removeAgent(id: string): void {
    this.agents.delete(id);
    
    // Remove associated edges
    const edgesToRemove: string[] = [];
    this.edges.forEach((edge, edgeId) => {
      if (edge.source === id || edge.target === id) {
        edgesToRemove.push(edgeId);
      }
    });
    
    edgesToRemove.forEach(edgeId => this.edges.delete(edgeId));
  }

  /**
   * Clean up
   */
  destroy(): void {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
    }
    this.agents.clear();
    this.edges.clear();
  }
}

export const mockDataGenerator = MockDataGenerator.getInstance();