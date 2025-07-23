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

  /**
   * Initialize mock data with specified number of agents
   */
  initialize(agentCount: number = 12): void {
    this.createAgents(agentCount);
    this.createEdges();
    this.startSimulation();
  }

  /**
   * Create mock agents with diverse types and states
   */
  private createAgents(count: number): void {
    const agentTypes: BotsAgent['type'][] = [
      'coordinator', 'coder', 'tester', 'analyst', 'researcher',
      'architect', 'reviewer', 'optimizer', 'documenter', 'monitor', 'specialist'
    ];

    const statuses: BotsAgent['status'][] = ['idle', 'busy', 'busy', 'idle']; // More busy agents

    for (let i = 0; i < count; i++) {
      const agentType = agentTypes[i % agentTypes.length];
      const status = statuses[Math.floor(Math.random() * statuses.length)];
      
      const agent: BotsAgent = {
        id: `agent-${i}`,
        type: agentType,
        status: status,
        health: 70 + Math.random() * 30, // 70-100%
        cpuUsage: status === 'busy' ? 40 + Math.random() * 50 : 5 + Math.random() * 25,
        memoryUsage: 20 + Math.random() * 60,
        workload: status === 'busy' ? 0.5 + Math.random() * 0.5 : Math.random() * 0.3,
        createdAt: new Date(Date.now() - Math.random() * 3600000).toISOString(),
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