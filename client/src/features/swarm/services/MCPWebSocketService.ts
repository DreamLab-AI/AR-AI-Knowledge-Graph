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
  private dataType: 'visionflow' | 'logseq' = 'visionflow'; // Data type identifier

  constructor(private wsUrl?: string) {
    // Determine the WebSocket URL based on environment
    if (!wsUrl) {
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const host = window.location.host;
      
      // Use MCP relay endpoint
      this.wsUrl = `${protocol}//${host}/ws/mcp`;
      logger.info('MCP WebSocket configured for MCP relay endpoint:', this.wsUrl);
    } else {
      this.wsUrl = wsUrl;
    }
  }

  async connect(): Promise<void> {
    // Connect to the backend's WebSocket endpoint through Nginx proxy
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const possibleUrls = [];
    
    // Try MCP relay endpoints first
    possibleUrls.push(
      `${protocol}//${window.location.host}/ws/mcp`,
      `ws://${window.location.hostname}/ws/mcp`,
      `ws://localhost:3001/ws/mcp`
    );
    
    // Add fallback to main WebSocket endpoints if MCP not available
    possibleUrls.push(
      `${protocol}//${window.location.host}/wss`,  // Main WebSocket endpoint
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

        this.ws.onmessage = async (event) => {
          try {
            let data: string;
            
            // Handle both text and binary (Blob) data
            if (event.data instanceof Blob) {
              // Convert Blob to text
              data = await event.data.text();
            } else {
              // Already text
              data = event.data;
            }
            
            // --- START OF THE FIX ---
            const trimmedData = data.trim();
            
            // Check for empty, whitespace-only, or non-JSON messages
            if (!trimmedData || trimmedData === '' || trimmedData === ' ') {
              logger.debug('Received an empty or whitespace-only message, likely a keep-alive. Ignoring.');
              return; // Ignore empty messages
            }
            
            // Additional check for common keep-alive patterns
            if (trimmedData === 'ping' || trimmedData === 'pong' || trimmedData === '1') {
              logger.debug('Received a keep-alive message:', trimmedData);
              return; // Ignore keep-alive messages
            }
            
            // Check if it looks like JSON before parsing
            if (!trimmedData.startsWith('{') && !trimmedData.startsWith('[')) {
              logger.debug('Received non-JSON message, ignoring:', trimmedData);
              return;
            }
            // --- END OF THE FIX ---
            
            // Now, parse the sanitized data
            const message: MCPMessage = JSON.parse(trimmedData);
            this.handleMessage(message);
          } catch (error) {
            logger.error('Failed to parse MCP message:', error);
            if (data) {
              logger.error('Raw message data:', data);
              logger.error('Data length:', data.length);
              logger.error('Data char codes:', Array.from(data).map(c => c.charCodeAt(0)));
            }
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

  // Set the data type this service handles
  public setDataType(type: 'visionflow' | 'logseq'): void {
    this.dataType = type;
    logger.info(`MCPWebSocketService configured for ${type} data`);
  }

  private handleMessage(message: MCPMessage) {
    switch (message.type) {
      case 'welcome':
        this.clientId = message.clientId || null;
        logger.info(`Received welcome message for ${this.dataType}, clientId:`, this.clientId);
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
        logger.debug(`Received MCP update for ${this.dataType}`);
        // Add data type to the event for proper routing
        this.emit('update', { ...message.data, dataType: this.dataType });
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
    // Only return agents for VisionFlow data
    if (this.dataType !== 'visionflow') {
      return [];
    }
    const response = await this.requestTool('agents/list');
    return response.agents || [];
  }

  async getTokenUsage(): Promise<TokenUsage> {
    return await this.requestTool('analysis/token-usage');
  }

  async getCommunications(filter: any = { type: 'communication' }): Promise<SwarmCommunication[]> {
    // Only return communications for VisionFlow data
    if (this.dataType !== 'visionflow') {
      return [];
    }
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