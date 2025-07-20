import { createLogger } from '../../../utils/logger';
import type { SwarmAgent, SwarmCommunication, TokenUsage } from '../types/swarmTypes';

const logger = createLogger('MockSwarmDataProvider');

export class MockSwarmDataProvider {
  private agents: Map<string, SwarmAgent> = new Map();
  private updateInterval: number | null = null;
  private communicationInterval: number | null = null;
  private listeners: Map<string, Set<(data: any) => void>> = new Map();

  constructor() {
    this.initializeMockAgents();
  }

  private initializeMockAgents() {
    const agentTypes = [
      'coordinator', 'researcher', 'coder', 'analyst', 
      'architect', 'tester', 'reviewer', 'optimizer'
    ];

    // Create 8 mock agents
    for (let i = 0; i < 8; i++) {
      const agentId = `agent-${i + 1}`;
      const agent: SwarmAgent = {
        id: agentId,
        name: `${agentTypes[i % agentTypes.length]}-${i + 1}`,
        type: agentTypes[i % agentTypes.length] as any,
        status: 'idle',
        health: 80 + Math.random() * 20,
        cpuUsage: Math.random() * 30,
        memoryUsage: Math.random() * 40,
        workload: Math.random() * 0.5,
        createdAt: Date.now() - Math.random() * 3600000,
        lastHeartbeat: Date.now(),
        position: {
          x: (Math.random() - 0.5) * 30,
          y: (Math.random() - 0.5) * 30,
          z: (Math.random() - 0.5) * 30
        }
      };
      this.agents.set(agentId, agent);
    }
  }

  connect(): Promise<void> {
    return new Promise((resolve) => {
      logger.info('Mock swarm data provider connected');
      
      // Start simulating updates
      this.startSimulation();
      
      setTimeout(() => {
        this.emit('welcome', { message: 'Connected to mock swarm' });
        resolve();
      }, 100);
    });
  }

  disconnect() {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
    }
    if (this.communicationInterval) {
      clearInterval(this.communicationInterval);
      this.communicationInterval = null;
    }
    logger.info('Mock swarm data provider disconnected');
  }

  private startSimulation() {
    // Update agent states every 2 seconds
    this.updateInterval = window.setInterval(() => {
      this.updateAgentStates();
    }, 2000);

    // Generate communications every 3 seconds
    this.communicationInterval = window.setInterval(() => {
      this.generateCommunications();
    }, 3000);
  }

  private updateAgentStates() {
    const agents = Array.from(this.agents.values());
    
    agents.forEach(agent => {
      // Randomly update status
      if (Math.random() < 0.3) {
        const statuses = ['idle', 'busy', 'processing'] as const;
        agent.status = statuses[Math.floor(Math.random() * statuses.length)];
      }

      // Update metrics with some variation
      agent.cpuUsage = Math.max(0, Math.min(100, agent.cpuUsage + (Math.random() - 0.5) * 20));
      agent.memoryUsage = Math.max(0, Math.min(100, agent.memoryUsage + (Math.random() - 0.5) * 10));
      agent.workload = Math.max(0, Math.min(1, agent.workload + (Math.random() - 0.5) * 0.2));
      agent.health = Math.max(0, Math.min(100, agent.health + (Math.random() - 0.5) * 5));
      agent.lastHeartbeat = Date.now();
    });

    this.emit('update', { agents });
  }

  private generateCommunications(): SwarmCommunication[] {
    const agents = Array.from(this.agents.values());
    const communications: SwarmCommunication[] = [];

    // Generate 1-3 random communications
    const numComms = Math.floor(Math.random() * 3) + 1;
    
    for (let i = 0; i < numComms; i++) {
      const sender = agents[Math.floor(Math.random() * agents.length)];
      const receiver = agents[Math.floor(Math.random() * agents.length)];
      
      if (sender.id !== receiver.id) {
        communications.push({
          id: `comm-${Date.now()}-${i}`,
          sender: sender.id,
          receivers: [receiver.id],
          type: 'data_transfer',
          metadata: {
            size: Math.floor(Math.random() * 10000) + 1000,
            timestamp: Date.now()
          }
        });
      }
    }

    return communications;
  }

  getAgents(): Promise<SwarmAgent[]> {
    return Promise.resolve(Array.from(this.agents.values()));
  }

  getTokenUsage(): Promise<TokenUsage> {
    const agents = Array.from(this.agents.values());
    const byAgent: Record<string, number> = {};
    let total = 0;

    agents.forEach(agent => {
      const tokens = Math.floor(Math.random() * 5000) + 1000;
      byAgent[agent.id] = tokens;
      total += tokens;
    });

    return Promise.resolve({ total, byAgent });
  }

  getCommunications(): Promise<SwarmCommunication[]> {
    return Promise.resolve(this.generateCommunications());
  }

  on(event: string, handler: (data: any) => void) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(handler);
  }

  off(event: string, handler: (data: any) => void) {
    this.listeners.get(event)?.delete(handler);
  }

  private emit(event: string, data: any) {
    this.listeners.get(event)?.forEach(handler => handler(data));
  }
}