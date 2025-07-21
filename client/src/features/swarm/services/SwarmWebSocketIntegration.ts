import { createLogger } from '../../../utils/logger';
import { mcpWebSocketService } from './MCPWebSocketService';
import { webSocketService } from '../../../services/WebSocketService';
import type { SwarmAgent, SwarmEdge, SwarmCommunication } from '../types/swarmTypes';

const logger = createLogger('SwarmWebSocketIntegration');

/**
 * Integration service that connects both Logseq graph and VisionFlow swarm
 * to their respective WebSocket services for real-time updates
 */
export class SwarmWebSocketIntegration {
  private static instance: SwarmWebSocketIntegration;
  private mcpConnected = false;
  private logseqConnected = false;
  private listeners: Map<string, Set<(data: any) => void>> = new Map();
  
  private constructor() {
    this.initializeConnections();
  }
  
  static getInstance(): SwarmWebSocketIntegration {
    if (!SwarmWebSocketIntegration.instance) {
      SwarmWebSocketIntegration.instance = new SwarmWebSocketIntegration();
    }
    return SwarmWebSocketIntegration.instance;
  }
  
  private async initializeConnections() {
    logger.info('Initializing dual WebSocket connections for graph and swarm');
    
    // Initialize MCP WebSocket for VisionFlow swarm data
    this.initializeMCPConnection();
    
    // Initialize main WebSocket for Logseq graph data
    this.initializeLogseqConnection();
  }
  
  private async initializeMCPConnection() {
    try {
      // Set MCP service to handle VisionFlow data
      mcpWebSocketService.setDataType('visionflow');
      
      // Listen for MCP connection events
      mcpWebSocketService.on('connected', () => {
        logger.info('MCP WebSocket connected for VisionFlow swarm');
        this.mcpConnected = true;
        this.emit('mcp-connected', { connected: true });
      });
      
      mcpWebSocketService.on('orchestrator_connected', () => {
        logger.info('Connected to MCP orchestrator');
        this.emit('orchestrator-connected', { connected: true });
      });
      
      mcpWebSocketService.on('update', (data) => {
        // Only process VisionFlow data
        if (data.dataType === 'visionflow') {
          logger.debug('Received VisionFlow swarm update', data);
          this.processSwarmUpdate(data);
        }
      });
      
      mcpWebSocketService.on('error', (error) => {
        logger.error('MCP WebSocket error:', error);
        this.emit('mcp-error', error);
      });
      
      // Connect to MCP relay
      await mcpWebSocketService.connect();
      
    } catch (error) {
      logger.error('Failed to initialize MCP connection:', error);
      this.mcpConnected = false;
    }
  }
  
  private initializeLogseqConnection() {
    // Listen for main WebSocket connection events (Logseq graph)
    webSocketService.onConnectionStatusChange((connected) => {
      logger.info(`Logseq WebSocket connection status: ${connected}`);
      this.logseqConnected = connected;
      this.emit('logseq-connected', { connected });
    });
    
    // Listen for Logseq graph messages
    webSocketService.onMessage((message) => {
      if (message.type === 'graph-update') {
        logger.debug('Received Logseq graph update', message.data);
        this.emit('logseq-graph-update', message.data);
      }
    });
    
    // Listen for binary updates (node positions from Logseq)
    webSocketService.onBinaryMessage((data) => {
      logger.debug(`Received Logseq binary update: ${data.byteLength} bytes`);
      this.emit('logseq-binary-update', data);
    });
  }
  
  private processSwarmUpdate(data: any) {
    // Process different types of swarm updates
    if (data.agents) {
      this.emit('swarm-agents-update', data.agents);
    }
    
    if (data.edges) {
      this.emit('swarm-edges-update', data.edges);
    }
    
    if (data.communications) {
      this.emit('swarm-communications-update', data.communications);
    }
    
    if (data.tokenUsage) {
      this.emit('swarm-token-usage', data.tokenUsage);
    }
    
    // Emit general swarm update
    this.emit('swarm-update', data);
  }
  
  // Public API methods
  
  /**
   * Request initial data for both visualizations
   */
  async requestInitialData() {
    logger.info('Requesting initial data for both visualizations');
    
    // Request Logseq graph data
    if (this.logseqConnected) {
      webSocketService.sendMessage('requestInitialData');
    }
    
    // Request VisionFlow swarm data
    if (this.mcpConnected) {
      try {
        const [agents, tokenUsage] = await Promise.all([
          mcpWebSocketService.getAgents(),
          mcpWebSocketService.getTokenUsage()
        ]);
        
        this.processSwarmUpdate({ agents, tokenUsage });
      } catch (error) {
        logger.error('Failed to fetch initial swarm data:', error);
      }
    }
  }
  
  /**
   * Send swarm position update (from programmatic monitor)
   */
  async sendSwarmUpdate(data: {
    nodes: SwarmAgent[],
    edges: SwarmEdge[]
  }) {
    if (!this.mcpConnected) {
      logger.warn('Cannot send swarm update: MCP not connected');
      return;
    }
    
    try {
      // Send update through MCP WebSocket
      await mcpWebSocketService.requestTool('swarm/update', {
        nodes: data.nodes,
        edges: data.edges,
        timestamp: Date.now()
      });
      
      logger.debug('Sent swarm update with', {
        nodeCount: data.nodes.length,
        edgeCount: data.edges.length
      });
    } catch (error) {
      logger.error('Failed to send swarm update:', error);
    }
  }
  
  /**
   * Check connection status for both services
   */
  getConnectionStatus() {
    return {
      mcp: this.mcpConnected,
      logseq: this.logseqConnected,
      overall: this.mcpConnected && this.logseqConnected
    };
  }
  
  /**
   * Subscribe to events
   */
  on(event: string, callback: (data: any) => void) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(callback);
    
    return () => {
      this.listeners.get(event)?.delete(callback);
    };
  }
  
  private emit(event: string, data: any) {
    this.listeners.get(event)?.forEach(callback => {
      try {
        callback(data);
      } catch (error) {
        logger.error(`Error in event listener for ${event}:`, error);
      }
    });
  }
  
  /**
   * Cleanup connections
   */
  disconnect() {
    logger.info('Disconnecting WebSocket services');
    mcpWebSocketService.disconnect();
    webSocketService.close();
    this.mcpConnected = false;
    this.logseqConnected = false;
  }
}

// Export singleton instance
export const swarmWebSocketIntegration = SwarmWebSocketIntegration.getInstance();