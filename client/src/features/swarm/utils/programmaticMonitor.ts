/**
 * Programmatic monitor utilities for sending swarm updates
 * Compatible with the swarm-monitor.py pattern
 */

import { apiService } from '../../../services/api';
import { createLogger } from '../../../utils/logger';
import type { SwarmAgent, SwarmEdge } from '../types/swarmTypes';

const logger = createLogger('ProgrammaticMonitor');

export interface SwarmUpdatePayload {
  nodes: SwarmAgent[];
  edges: SwarmEdge[];
  timestamp?: string;
}

/**
 * Send swarm update through HTTP API endpoint
 * This follows the same pattern as the programmatic-monitor.py
 */
export async function sendSwarmUpdate(data: SwarmUpdatePayload): Promise<void> {
  try {
    // Add timestamp if not provided
    const payload = {
      ...data,
      timestamp: data.timestamp || new Date().toISOString()
    };
    
    // Send through API service
    const response = await apiService.post('/api/swarm/update', payload);
    
    logger.debug('Sent swarm update:', {
      nodeCount: data.nodes.length,
      edgeCount: data.edges.length,
      timestamp: payload.timestamp
    });
    
    return response;
  } catch (error) {
    logger.error('Failed to send swarm update:', error);
    throw error;
  }
}

/**
 * Generate mock agent data for testing
 */
export function generateMockAgent(id: string, type: string, name: string): SwarmAgent {
  return {
    id,
    type,
    status: 'active',
    name,
    cpuUsage: Math.random() * 100,
    memoryUsage: Math.random() * 100,
    health: 80 + Math.random() * 20,
    workload: Math.random(),
    createdAt: new Date().toISOString(),
    age: Math.floor(Math.random() * 3600)
  };
}

/**
 * Generate mock edge data for testing
 */
export function generateMockEdge(source: string, target: string): SwarmEdge {
  return {
    id: `edge_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
    source,
    target,
    dataVolume: Math.random() * 5000,
    messageCount: Math.floor(Math.random() * 50) + 1,
    lastMessageTime: Date.now()
  };
}

/**
 * Create a programmatic monitor that sends periodic updates
 */
export class ProgrammaticMonitor {
  private intervalId: number | null = null;
  private agents: Map<string, SwarmAgent> = new Map();
  
  constructor() {
    // Initialize with some default agents
    this.agents.set('agent-1', generateMockAgent('agent-1', 'coordinator', 'Swarm Coordinator'));
    this.agents.set('agent-2', generateMockAgent('agent-2', 'researcher', 'Data Researcher'));
    this.agents.set('agent-3', generateMockAgent('agent-3', 'coder', 'Code Generator'));
    this.agents.set('agent-4', generateMockAgent('agent-4', 'analyst', 'Performance Analyst'));
    this.agents.set('agent-5', generateMockAgent('agent-5', 'tester', 'Quality Tester'));
  }
  
  /**
   * Start monitoring and sending updates
   */
  start(intervalMs: number = 2000): void {
    if (this.intervalId) {
      logger.warn('Monitor already running');
      return;
    }
    
    logger.info(`Starting programmatic monitor with ${intervalMs}ms interval`);
    
    this.intervalId = window.setInterval(() => {
      this.sendUpdate();
    }, intervalMs);
    
    // Send initial update
    this.sendUpdate();
  }
  
  /**
   * Stop monitoring
   */
  stop(): void {
    if (this.intervalId) {
      window.clearInterval(this.intervalId);
      this.intervalId = null;
      logger.info('Programmatic monitor stopped');
    }
  }
  
  /**
   * Update agent metrics
   */
  private updateAgentMetrics(agent: SwarmAgent): void {
    // Random walk for CPU usage
    agent.cpuUsage = Math.max(0, Math.min(100, 
      agent.cpuUsage + (Math.random() - 0.5) * 20));
    
    // Workload affects health
    if (agent.workload > 0.8) {
      agent.health = Math.max(0, agent.health - Math.random() * 2);
    } else {
      agent.health = Math.min(100, agent.health + Math.random());
    }
    
    // Random workload changes
    agent.workload = Math.max(0, Math.min(1.0, 
      agent.workload + (Math.random() - 0.5) * 0.2));
    
    // Update age
    agent.age = (agent.age || 0) + 2; // seconds
  }
  
  /**
   * Generate communications between agents
   */
  private generateCommunications(): SwarmEdge[] {
    const edges: SwarmEdge[] = [];
    const agentIds = Array.from(this.agents.keys());
    
    if (agentIds.length >= 2) {
      // Generate 1-3 random communications
      const numComms = Math.floor(Math.random() * 3) + 1;
      
      for (let i = 0; i < numComms; i++) {
        const source = agentIds[Math.floor(Math.random() * agentIds.length)];
        const target = agentIds.find(id => id !== source) || agentIds[0];
        
        if (source !== target) {
          edges.push(generateMockEdge(source, target));
        }
      }
    }
    
    return edges;
  }
  
  /**
   * Send update
   */
  private async sendUpdate(): Promise<void> {
    // Update all agent metrics
    this.agents.forEach(agent => this.updateAgentMetrics(agent));
    
    // Generate communications
    const edges = this.generateCommunications();
    
    // Prepare payload
    const payload: SwarmUpdatePayload = {
      nodes: Array.from(this.agents.values()),
      edges
    };
    
    try {
      await sendSwarmUpdate(payload);
    } catch (error) {
      logger.error('Failed to send update:', error);
    }
  }
  
  /**
   * Add or update an agent
   */
  addAgent(agent: SwarmAgent): void {
    this.agents.set(agent.id, agent);
  }
  
  /**
   * Remove an agent
   */
  removeAgent(agentId: string): void {
    this.agents.delete(agentId);
  }
  
  /**
   * Get current agents
   */
  getAgents(): SwarmAgent[] {
    return Array.from(this.agents.values());
  }
}

// Export singleton instance for easy use
export const programmaticMonitor = new ProgrammaticMonitor();