/**
 * Parallel Graph Coordinator
 * Manages both Logseq and VisionFlow graphs running in parallel
 */

import { createLogger } from '../../../utils/logger';
import { graphDataManager } from '../managers/graphDataManager';
import { mcpWebSocketService } from '../../swarm/services/MCPWebSocketService';
import { swarmPhysicsWorker } from '../../swarm/workers/swarmPhysicsWorker';
import type { GraphData } from '../managers/graphWorkerProxy';
import type { SwarmAgent, SwarmEdge, TokenUsage } from '../../swarm/types/swarmTypes';

const logger = createLogger('ParallelGraphCoordinator');

export interface ParallelGraphState {
  logseq: {
    enabled: boolean;
    data: GraphData | null;
    lastUpdate: number;
  };
  visionflow: {
    enabled: boolean;
    agents: SwarmAgent[];
    edges: SwarmEdge[];
    tokenUsage: TokenUsage | null;
    lastUpdate: number;
  };
}

class ParallelGraphCoordinator {
  private static instance: ParallelGraphCoordinator;
  private state: ParallelGraphState = {
    logseq: {
      enabled: false,
      data: null,
      lastUpdate: 0
    },
    visionflow: {
      enabled: false,
      agents: [],
      edges: [],
      tokenUsage: null,
      lastUpdate: 0
    }
  };
  private listeners: Set<(state: ParallelGraphState) => void> = new Set();

  private constructor() {}

  public static getInstance(): ParallelGraphCoordinator {
    if (!ParallelGraphCoordinator.instance) {
      ParallelGraphCoordinator.instance = new ParallelGraphCoordinator();
    }
    return ParallelGraphCoordinator.instance;
  }

  /**
   * Initialize both graph systems
   */
  public async initialize(): Promise<void> {
    logger.info('Initializing parallel graph coordinator');

    // Configure graph data manager for Logseq
    graphDataManager.setGraphType('logseq');

    // Configure MCP WebSocket service for VisionFlow
    mcpWebSocketService.setDataType('visionflow');

    // Configure swarm physics worker for VisionFlow
    swarmPhysicsWorker.setDataType('visionflow');

    // Set up listeners for both data sources
    this.setupLogseqListeners();
    this.setupVisionFlowListeners();

    logger.info('Parallel graph coordinator initialized');
  }

  /**
   * Enable/disable Logseq graph
   */
  public setLogseqEnabled(enabled: boolean): void {
    this.state.logseq.enabled = enabled;
    logger.info(`Logseq graph ${enabled ? 'enabled' : 'disabled'}`);
    this.notifyListeners();

    if (enabled) {
      // Start fetching Logseq data
      this.fetchLogseqData();
    }
  }

  /**
   * Enable/disable VisionFlow graph
   */
  public setVisionFlowEnabled(enabled: boolean): void {
    this.state.visionflow.enabled = enabled;
    logger.info(`VisionFlow graph ${enabled ? 'enabled' : 'disabled'}`);
    this.notifyListeners();

    if (enabled) {
      // Connect to VisionFlow WebSocket
      this.connectVisionFlow();
    } else {
      // Disconnect from VisionFlow
      mcpWebSocketService.disconnect();
    }
  }

  /**
   * Get current state of both graphs
   */
  public getState(): ParallelGraphState {
    return { ...this.state };
  }

  /**
   * Subscribe to state changes
   */
  public onStateChange(listener: (state: ParallelGraphState) => void): () => void {
    this.listeners.add(listener);
    // Immediately notify with current state
    listener(this.getState());

    // Return unsubscribe function
    return () => {
      this.listeners.delete(listener);
    };
  }

  /**
   * Setup Logseq data listeners
   */
  private setupLogseqListeners(): void {
    // Listen for graph data changes from Logseq
    graphDataManager.onGraphDataChange((data) => {
      if (this.state.logseq.enabled) {
        this.state.logseq.data = data;
        this.state.logseq.lastUpdate = Date.now();
        this.notifyListeners();
      }
    });
  }

  /**
   * Setup VisionFlow data listeners
   */
  private setupVisionFlowListeners(): void {
    // Listen for MCP updates
    mcpWebSocketService.on('update', async (data) => {
      if (!this.state.visionflow.enabled || data.dataType !== 'visionflow') {
        return;
      }

      try {
        // Fetch latest agents and communications
        const agents = await mcpWebSocketService.getAgents();
        const tokenUsage = await mcpWebSocketService.getTokenUsage();
        const communications = await mcpWebSocketService.getCommunications();

        // Process communications into edges
        const edgeMap = new Map<string, SwarmEdge>();
        communications.forEach(comm => {
          comm.receivers.forEach(receiver => {
            const edgeId = `${comm.sender}-${receiver}`;
            const reverseEdgeId = `${receiver}-${comm.sender}`;
            
            // Check if edge already exists (in either direction)
            let edge = edgeMap.get(edgeId) || edgeMap.get(reverseEdgeId);
            
            if (!edge) {
              edge = {
                id: edgeId,
                source: comm.sender,
                target: receiver,
                dataVolume: 0,
                messageCount: 0,
                lastMessageTime: 0
              };
              edgeMap.set(edgeId, edge);
            }
            
            // Update edge metrics
            edge.dataVolume += comm.metadata.size;
            edge.messageCount += 1;
            edge.lastMessageTime = Math.max(
              edge.lastMessageTime,
              new Date(comm.timestamp).getTime()
            );
          });
        });

        // Update physics simulation
        swarmPhysicsWorker.updateAgents(agents);
        swarmPhysicsWorker.updateEdges(Array.from(edgeMap.values()));
        swarmPhysicsWorker.updateTokenUsage(tokenUsage);

        // Update state
        this.state.visionflow.agents = agents;
        this.state.visionflow.edges = Array.from(edgeMap.values());
        this.state.visionflow.tokenUsage = tokenUsage;
        this.state.visionflow.lastUpdate = Date.now();

        this.notifyListeners();
      } catch (error) {
        logger.error('Error processing VisionFlow update:', error);
      }
    });
  }

  /**
   * Fetch Logseq graph data
   */
  private async fetchLogseqData(): Promise<void> {
    if (!this.state.logseq.enabled) {
      return;
    }

    try {
      const data = await graphDataManager.fetchInitialData();
      this.state.logseq.data = data;
      this.state.logseq.lastUpdate = Date.now();
      this.notifyListeners();
    } catch (error) {
      logger.error('Error fetching Logseq data:', error);
    }
  }

  /**
   * Connect to VisionFlow WebSocket
   */
  private async connectVisionFlow(): Promise<void> {
    if (!this.state.visionflow.enabled) {
      return;
    }

    try {
      await mcpWebSocketService.connect();
      logger.info('Connected to VisionFlow WebSocket');
    } catch (error) {
      logger.error('Error connecting to VisionFlow:', error);
    }
  }

  /**
   * Notify all listeners of state changes
   */
  private notifyListeners(): void {
    const state = this.getState();
    this.listeners.forEach(listener => {
      try {
        listener(state);
      } catch (error) {
        logger.error('Error in state change listener:', error);
      }
    });
  }

  /**
   * Get Logseq node positions
   */
  public async getLogseqPositions(): Promise<Map<string, { x: number; y: number; z: number }>> {
    if (!this.state.logseq.enabled || !this.state.logseq.data) {
      return new Map();
    }

    const positions = new Map();
    this.state.logseq.data.nodes.forEach(node => {
      positions.set(node.id, {
        x: node.position.x,
        y: node.position.y,
        z: node.position.z
      });
    });

    return positions;
  }

  /**
   * Get VisionFlow node positions
   */
  public getVisionFlowPositions(): Map<string, { x: number; y: number; z: number }> {
    if (!this.state.visionflow.enabled) {
      return new Map();
    }

    return swarmPhysicsWorker.getPositions();
  }

  /**
   * Cleanup resources
   */
  public dispose(): void {
    this.listeners.clear();
    if (this.state.visionflow.enabled) {
      mcpWebSocketService.disconnect();
    }
    logger.info('Parallel graph coordinator disposed');
  }
}

// Export singleton instance
export const parallelGraphCoordinator = ParallelGraphCoordinator.getInstance();