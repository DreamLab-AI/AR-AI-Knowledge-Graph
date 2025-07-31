import { createLogger } from '../../../utils/logger';
import { webSocketService } from '../../../services/WebSocketService';
import type { BotsAgent, BotsEdge, BotsCommunication } from '../types/BotsTypes';

const logger = createLogger('BotsWebSocketIntegration');

/**
 * Integration service that handles WebSocket connections for graph data
 * Note: MCP connections are handled by the backend only - frontend uses REST API
 */
export class BotsWebSocketIntegration {
  private static instance: BotsWebSocketIntegration;
  private logseqConnected = false;
  private listeners: Map<string, Set<(data: any) => void>> = new Map();

  private constructor() {
    this.initializeConnections();
  }

  static getInstance(): BotsWebSocketIntegration {
    if (!BotsWebSocketIntegration.instance) {
      BotsWebSocketIntegration.instance = new BotsWebSocketIntegration();
    }
    return BotsWebSocketIntegration.instance;
  }

  private async initializeConnections() {
    logger.info('Initializing WebSocket connection for graph data');

    // Initialize main WebSocket for Logseq graph data and position updates
    this.initializeLogseqConnection();
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

  private processBotsUpdate(data: any) {
    // Process different types of bots updates
    if (data.agents) {
      this.emit('bots-agents-update', data.agents);
    }

    if (data.edges) {
      this.emit('bots-edges-update', data.edges);
    }

    if (data.communications) {
      this.emit('bots-communications-update', data.communications);
    }

    if (data.tokenUsage) {
      this.emit('bots-token-usage', data.tokenUsage);
    }

    // Emit general bots update
    this.emit('bots-update', data);
  }

  // Public API methods

  /**
   * Request initial data for both visualizations
   */
  async requestInitialData() {
    logger.info('Requesting initial data for graph visualization');

    // Request Logseq graph data
    if (this.logseqConnected) {
      webSocketService.sendMessage('requestInitialData');
    }

    // Fetch bots data via REST API from the backend
    try {
      const { apiService } = await import('../../../services/apiService');
      const botsData = await apiService.get('/bots/data');
      logger.info('Fetched bots data:', botsData);
      
      // Emit the data for components to use
      if (botsData && botsData.nodes) {
        this.emit('bots-data', botsData);
      }
    } catch (error) {
      logger.error('Failed to fetch bots data:', error);
    }
  }

  /**
   * Send bots position update (deprecated - use REST API instead)
   */
  async sendBotsUpdate(data: {
    nodes: BotsAgent[],
    edges: BotsEdge[]
  }) {
    logger.warn('sendBotsUpdate is deprecated. Use REST API POST /api/bots/update instead');
    // Updates should be sent via REST API to the backend
    // The backend handles all communication with Claude Flow MCP
  }

  /**
   * Check connection status
   */
  getConnectionStatus() {
    return {
      mcp: false, // MCP connections are handled by backend only
      logseq: this.logseqConnected,
      overall: this.logseqConnected
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
    webSocketService.close();
    this.logseqConnected = false;
  }
}

// Export singleton instance
export const botsWebSocketIntegration = BotsWebSocketIntegration.getInstance();