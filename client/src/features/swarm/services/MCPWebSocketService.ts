import { createLogger } from '../../../utils/logger';
import type { MCPMessage, MCPRequest, SwarmAgent, SwarmCommunication, TokenUsage } from '../types/swarmTypes';

const logger = createLogger('MCPWebSocketService');

export class MCPWebSocketService {
  private ws: WebSocket | null = null;
  private reconnectTimer: NodeJS.Timeout | null = null;
  private pingInterval: NodeJS.Timeout | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private clientId: string | null = null;
  private listeners: Map<string, Set<(data: any) => void>> = new Map();
  private requestCallbacks: Map<string, (data: any) => void> = new Map();
  private useMCPRelay = true; // Flag to control which endpoint to use

  constructor(private wsUrl?: string) {
    // Determine the WebSocket URL based on environment
    if (!wsUrl) {
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const host = window.location.host;
      
      // Check if we should use the MCP relay endpoint
      if (this.useMCPRelay) {
        // Use the new MCP relay endpoint that connects to orchestrator
        this.wsUrl = `${protocol}//${host}/ws/mcp`;
        logger.info('MCP WebSocket configured to use relay endpoint:', this.wsUrl);
      } else {
        // Use the original /wss endpoint for backward compatibility
        this.wsUrl = `${protocol}//${host}/wss`;
        logger.info('MCP WebSocket configured to use legacy endpoint:', this.wsUrl);
      }
    } else {
      this.wsUrl = wsUrl;
    }
  }

  async connect(): Promise<void> {
    // Connect to the backend's WebSocket endpoint through Nginx proxy
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const possibleUrls = [];
    
    if (this.useMCPRelay) {
      // Try MCP relay endpoints first
      possibleUrls.push(
        this.wsUrl!,  // Primary URL: backend /ws/mcp endpoint
        `${protocol}//${window.location.host}/ws/mcp`,  // MCP relay endpoint
        `ws://${window.location.hostname}/ws/mcp`,      // Without port (Nginx default)
        `ws://localhost:3001/ws/mcp`,                    // Direct Nginx port (dev)
        `ws://localhost:8080/ws/mcp`                     // Fallback port
      );
    }
    
    // Add legacy endpoints as fallback
    possibleUrls.push(
      `${protocol}//${window.location.host}/wss`,  // Legacy WebSocket endpoint
      `ws://${window.location.hostname}/wss`,      // Without port (Nginx default)
      `ws://localhost:3001/wss`,                   // Direct Nginx port (dev)
      `ws://localhost:8080/wss`                    // Fallback port
    );

    for (const url of possibleUrls) {
      try {
        logger.info('Trying MCP WebSocket at:', url);
        await this.connectToUrl(url);
        this.wsUrl = url;  // Update to successful URL
        return;
      } catch (error) {
        logger.warn(`Failed to connect to ${url}:`, error);
      }
    }

    throw new Error('Failed to connect to MCP server at any known location');
  }

  private connectToUrl(url: string): Promise<void> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        if (this.ws) {
          this.ws.close();
        }
        reject(new Error('Connection timeout'));
      }, 5000);

      try {
        this.ws = new WebSocket(url);

        this.ws.onopen = () => {
          clearTimeout(timeout);
          logger.info('MCP WebSocket connected to:', url);
          this.reconnectAttempts = 0;
          this.startPingInterval();
          resolve();
        };

        this.ws.onmessage = (event) => {
          try {
            const message: MCPMessage = JSON.parse(event.data);
            this.handleMessage(message);
          } catch (error) {
            logger.error('Failed to parse MCP message:', error);
          }
        };

        this.ws.onerror = (error) => {
          clearTimeout(timeout);
          logger.error('MCP WebSocket error:', error);
          reject(error);
        };

        this.ws.onclose = () => {
          clearTimeout(timeout);
          logger.warn('MCP WebSocket closed');
          this.cleanup();
          this.scheduleReconnect();
        };
      } catch (error) {
        clearTimeout(timeout);
        logger.error('Failed to create WebSocket:', error);
        reject(error);
      }
    });
  }

  private handleMessage(message: MCPMessage) {
    switch (message.type) {
      case 'welcome':
        this.clientId = message.clientId || null;
        logger.info('Received welcome message, clientId:', this.clientId);
        this.emit('welcome', message.data);
        break;

      case 'connection_established':
        logger.info('MCP relay connection established');
        this.emit('connected', message);
        break;

      case 'orchestrator_connected':
        logger.info('Connected to MCP orchestrator');
        this.emit('orchestrator_connected', message);
        break;

      case 'orchestrator_disconnected':
        logger.warn('Disconnected from MCP orchestrator');
        this.emit('orchestrator_disconnected', message);
        break;

      case 'mcp-update':
        logger.debug('Received MCP update');
        this.emit('update', message.data);
        break;

      case 'mcp-response':
        if (message.requestId && this.requestCallbacks.has(message.requestId)) {
          const callback = this.requestCallbacks.get(message.requestId)!;
          this.requestCallbacks.delete(message.requestId);
          callback(message.data);
        }
        break;

      case 'pong':
        logger.debug('Received pong');
        break;

      case 'error':
        logger.error('MCP error:', message.message);
        this.emit('error', message);
        break;

      default:
        logger.warn('Unknown message type:', message.type);
    }
  }

  async requestTool(toolName: string, args: any = {}): Promise<any> {
    return new Promise((resolve, reject) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        reject(new Error('WebSocket not connected'));
        return;
      }

      const requestId = `req_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
      const request: MCPRequest = {
        jsonrpc: '2.0',
        id: requestId,
        method: 'tools/call',
        params: {
          name: toolName,
          arguments: args
        }
      };

      this.requestCallbacks.set(requestId, (data) => {
        resolve(data);
      });

      // Timeout after 10 seconds
      setTimeout(() => {
        if (this.requestCallbacks.has(requestId)) {
          this.requestCallbacks.delete(requestId);
          reject(new Error('Request timeout'));
        }
      }, 10000);

      this.ws.send(JSON.stringify({
        type: 'mcp-request',
        requestId,
        ...request
      }));
    });
  }

  // Convenience methods for common tools
  async getAgents(): Promise<SwarmAgent[]> {
    const response = await this.requestTool('agents/list');
    return response.agents || [];
  }

  async getTokenUsage(): Promise<TokenUsage> {
    return await this.requestTool('analysis/token-usage');
  }

  async getCommunications(filter: any = { type: 'communication' }): Promise<SwarmCommunication[]> {
    const response = await this.requestTool('memory/query', { filter });
    return response.memories || [];
  }

  on(event: string, callback: (data: any) => void) {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(callback);
  }

  off(event: string, callback: (data: any) => void) {
    if (this.listeners.has(event)) {
      this.listeners.get(event)!.delete(callback);
    }
  }

  private emit(event: string, data: any) {
    if (this.listeners.has(event)) {
      this.listeners.get(event)!.forEach(callback => {
        try {
          callback(data);
        } catch (error) {
          logger.error('Error in event listener:', error);
        }
      });
    }
  }

  private startPingInterval() {
    this.pingInterval = setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: 'ping' }));
      }
    }, 30000); // Ping every 30 seconds
  }

  private scheduleReconnect() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      logger.error('Max reconnection attempts reached');
      return;
    }

    const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);
    this.reconnectAttempts++;

    logger.info(`Scheduling reconnect attempt ${this.reconnectAttempts} in ${delay}ms`);
    this.reconnectTimer = setTimeout(() => {
      this.connect().catch(error => {
        logger.error('Reconnection failed:', error);
      });
    }, delay);
  }

  private cleanup() {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.requestCallbacks.clear();
  }

  disconnect() {
    this.cleanup();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
}

// Singleton instance
export const mcpWebSocketService = new MCPWebSocketService();