import { createLogger, createErrorMetadata } from '../../../utils/logger';
import { debugState } from '../../../utils/debugState';
import { WebSocketAdapter } from '../../../services/WebSocketService';
import { BinaryNodeData, parseBinaryNodeData, createBinaryNodeData, Vec3, BINARY_NODE_SIZE } from '../../../types/binaryProtocol';
import { graphWorkerProxy } from './graphWorkerProxy';
import type { GraphData, Node, Edge } from './graphWorkerProxy';
import { startTransition } from 'react';

const logger = createLogger('GraphDataManager');

// Re-export types from worker proxy for compatibility
export type { Node, Edge, GraphData } from './graphWorkerProxy';

type GraphDataChangeListener = (data: GraphData) => void;
type PositionUpdateListener = (positions: Float32Array) => void;

class GraphDataManager {
  private static instance: GraphDataManager;
  private binaryUpdatesEnabled: boolean = false;
  public webSocketService: WebSocketAdapter | null = null;
  private graphDataListeners: GraphDataChangeListener[] = [];
  private positionUpdateListeners: PositionUpdateListener[] = [];
  private lastBinaryUpdateTime: number = 0;
  private retryTimeout: number | null = null;
  public nodeIdMap: Map<string, number> = new Map();
  private reverseNodeIdMap: Map<number, string> = new Map();
  private workerInitialized: boolean = false;

  private constructor() {
    // Worker proxy initializes automatically, just wait for it to be ready
    this.waitForWorker();
  }

  private async waitForWorker(): Promise<void> {
    try {
      // Poll until worker is ready
      while (!graphWorkerProxy.isReady()) {
        await new Promise(resolve => setTimeout(resolve, 10));
      }
      this.workerInitialized = true;
      
      // Connect to worker proxy listeners
      this.setupWorkerListeners();
      
      if (debugState.isEnabled()) {
        logger.info('Graph worker proxy is ready');
      }
    } catch (error) {
      logger.error('Failed to wait for graph worker proxy:', createErrorMetadata(error));
    }
  }

  private setupWorkerListeners(): void {
    // Forward graph data changes from worker to our listeners
    graphWorkerProxy.onGraphDataChange((data) => {
      this.graphDataListeners.forEach(listener => {
        try {
          startTransition(() => {
            listener(data);
          });
        } catch (error) {
          logger.error('Error in forwarded graph data listener:', createErrorMetadata(error));
        }
      });
    });

    // Forward position updates from worker to our listeners
    graphWorkerProxy.onPositionUpdate((positions) => {
      this.positionUpdateListeners.forEach(listener => {
        try {
          listener(positions);
        } catch (error) {
          logger.error('Error in forwarded position update listener:', createErrorMetadata(error));
        }
      });
    });
  }

  public static getInstance(): GraphDataManager {
    if (!GraphDataManager.instance) {
      GraphDataManager.instance = new GraphDataManager();
    }
    return GraphDataManager.instance;
  }

  // Set WebSocket service for sending binary updates
  public setWebSocketService(service: WebSocketAdapter): void {
    this.webSocketService = service;
    if (debugState.isDataDebugEnabled()) {
      logger.debug('WebSocket service set');
    }
  }

  // Fetch initial graph data from the API
  public async fetchInitialData(): Promise<GraphData> {
    try {
      if (debugState.isEnabled()) {
        logger.info('Fetching initial graph data');
      }

      const response = await fetch('/api/graph/data');
      if (!response.ok) {
        throw new Error(`API request failed with status ${response.status}`);
      }

      try {
        const data = await response.json();
        
        // Validate that the response has the expected structure
        if (!data || typeof data !== 'object') {
          throw new Error('Invalid graph data format: data is not an object');
        }
        
        // Ensure nodes and edges exist, even if empty
        const validatedData = {
          nodes: Array.isArray(data.nodes) ? data.nodes : [],
          edges: Array.isArray(data.edges) ? data.edges : []
        };
        
        if (debugState.isEnabled()) {
          logger.info(`Received initial graph data: ${validatedData.nodes.length} nodes, ${validatedData.edges.length} edges`);
          if (validatedData.nodes.length > 0) {
            logger.debug('Sample node data:', {
              id: validatedData.nodes[0].id,
              label: validatedData.nodes[0].label,
              position: validatedData.nodes[0].position,
              metadata: validatedData.nodes[0].metadata
            });
          }
          if (validatedData.edges.length > 0) {
            logger.debug('Sample edge data:', {
              id: validatedData.edges[0].id,
              source: validatedData.edges[0].source,
              target: validatedData.edges[0].target
            });
          }
        }
        
        await this.setGraphData(validatedData);
        
        const currentData = await graphWorkerProxy.getGraphData();
        if (debugState.isEnabled()) {
          logger.info(`Loaded initial graph data: ${currentData.nodes.length} nodes, ${currentData.edges.length} edges`);
          logger.debug('Node ID mappings created:', {
            numericIds: Array.from(this.nodeIdMap.entries()).slice(0, 5),
            totalMappings: this.nodeIdMap.size
          });
        }
        
        return currentData;
      } catch (parseError) {
        throw new Error(`Failed to parse graph data: ${parseError}`);
      }
    } catch (error) {
      logger.error('Failed to fetch initial graph data:', createErrorMetadata(error));
      throw error;
    }
  }

  // Set graph data and notify listeners
  public async setGraphData(data: GraphData): Promise<void> {
    if (debugState.isEnabled()) {
      logger.info(`Setting graph data: ${data.nodes.length} nodes, ${data.edges.length} edges`);
    }

    // Ensure all nodes have valid positions before setting the data
    let validatedData = data;
    if (data && data.nodes) {
      const validatedNodes = data.nodes.map(node => this.ensureNodeHasValidPosition(node));
      validatedData = {
        ...data,
        nodes: validatedNodes
      };
      
      if (debugState.isEnabled()) {
        logger.info(`Validated ${validatedNodes.length} nodes with positions`);
      }
    } else {
      // Initialize with empty arrays if data is invalid
      validatedData = { nodes: [], edges: data?.edges || [] };
      logger.warn('Initialized with empty graph data');
    }
    
    // Reset ID maps and rebuild them
    this.nodeIdMap.clear();
    this.reverseNodeIdMap.clear();
    
    // Create mappings between string IDs and numeric IDs (now u32 instead of u16)
    validatedData.nodes.forEach((node, index) => {
      const numericId = parseInt(node.id, 10);
      if (!isNaN(numericId) && numericId >= 0 && numericId <= 0xFFFFFFFF) {
        // If the ID can be parsed as a u32 number, use it directly
        this.nodeIdMap.set(node.id, numericId);
        this.reverseNodeIdMap.set(numericId, node.id);
      } else {
        // For non-numeric IDs, use the index + 1 as the numeric ID
        // We add 1 to avoid using 0 as an ID
        const mappedId = index + 1;
        this.nodeIdMap.set(node.id, mappedId);
        this.reverseNodeIdMap.set(mappedId, node.id);
      }
    });
    
    // Delegate to worker
    await graphWorkerProxy.setGraphData(validatedData);
    
    if (debugState.isDataDebugEnabled()) {
      logger.debug(`Graph data updated: ${validatedData.nodes.length} nodes, ${validatedData.edges.length} edges`);
    }
  }

  // Setup node IDs (simplified since data is now in worker)
  private validateNodeMappings(nodes: Node[]): void {
    if (debugState.isDataDebugEnabled()) {
      logger.debug(`Validated ${nodes.length} nodes with ID mapping`);
    }
  }

  // Enable binary updates and start the retry mechanism
  public enableBinaryUpdates(): void {
    if (!this.webSocketService) {
      logger.warn('Cannot enable binary updates: WebSocket service not set');
      return;
    }

    // If WebSocket is already ready, enable binary updates immediately
    if (this.webSocketService.isReady()) {
      this.setBinaryUpdatesEnabled(true);
      return;
    }

    // Otherwise, start a retry mechanism
    if (this.retryTimeout) {
      window.clearTimeout(this.retryTimeout);
    }

    this.retryTimeout = window.setTimeout(() => {
      if (this.webSocketService && this.webSocketService.isReady()) {
        this.setBinaryUpdatesEnabled(true);
        if (debugState.isEnabled()) {
          logger.info('WebSocket ready, binary updates enabled');
        }
      } else {
        if (debugState.isEnabled()) {
          logger.info('WebSocket not ready yet, retrying...');
        }
        this.enableBinaryUpdates();
      }
    }, 500);
  }

  public setBinaryUpdatesEnabled(enabled: boolean): void {
    this.binaryUpdatesEnabled = enabled;
    
    if (debugState.isEnabled()) {
      logger.info(`Binary updates ${enabled ? 'enabled' : 'disabled'}`);
    }
  }

  // Get the current graph data
  public async getGraphData(): Promise<GraphData> {
    return await graphWorkerProxy.getGraphData();
  }

  // Add a node to the graph
  public async addNode(node: Node): Promise<void> {
    // Update node mappings
    const numericId = parseInt(node.id, 10);
    if (!isNaN(numericId)) {
      this.nodeIdMap.set(node.id, numericId);
      this.reverseNodeIdMap.set(numericId, node.id);
    } else {
      // For non-numeric IDs, we'll get the current length from worker
      const currentData = await graphWorkerProxy.getGraphData();
      const mappedId = currentData.nodes.length + 1;
      this.nodeIdMap.set(node.id, mappedId);
      this.reverseNodeIdMap.set(mappedId, node.id);
    }
    
    await graphWorkerProxy.updateNode(node);
  }

  // Add an edge to the graph (for backward compatibility - edges go through worker)
  public async addEdge(edge: Edge): Promise<void> {
    // Worker doesn't have addEdge method, so we get current data, add edge, and set it back
    const currentData = await graphWorkerProxy.getGraphData();
    const existingIndex = currentData.edges.findIndex(e => e.id === edge.id);
    
    if (existingIndex >= 0) {
      currentData.edges[existingIndex] = {
        ...currentData.edges[existingIndex],
        ...edge
      };
    } else {
      currentData.edges.push(edge);
    }
    
    await graphWorkerProxy.setGraphData(currentData);
  }

  // Remove a node from the graph
  public async removeNode(nodeId: string): Promise<void> {
    // Get numeric ID before removing the node
    const numericId = this.nodeIdMap.get(nodeId);
    
    await graphWorkerProxy.removeNode(nodeId);
    
    // Remove from ID maps
    if (numericId !== undefined) {
      this.nodeIdMap.delete(nodeId);
      this.reverseNodeIdMap.delete(numericId);
    }
  }

  // Remove an edge from the graph
  public async removeEdge(edgeId: string): Promise<void> {
    // Worker doesn't have removeEdge method, so we get current data, remove edge, and set it back
    const currentData = await graphWorkerProxy.getGraphData();
    currentData.edges = currentData.edges.filter(edge => edge.id !== edgeId);
    await graphWorkerProxy.setGraphData(currentData);
  }

  // Update node positions from binary data
  public async updateNodePositions(positionData: ArrayBuffer): Promise<void> {
    if (!positionData || positionData.byteLength === 0) {
      return;
    }

    // Check if this is a duplicate update (can happen with WebSocket)
    const now = Date.now();
    if (now - this.lastBinaryUpdateTime < 16) { // Less than 16ms (60fps)
      if (debugState.isDataDebugEnabled()) {
        logger.debug('Skipping duplicate position update');
      }
      return;
    }
    this.lastBinaryUpdateTime = now;

    try {
      // Add diagnostic information about the received data
      if (debugState.isDataDebugEnabled()) {
        logger.debug(`Received binary data: ${positionData.byteLength} bytes`);
        
        // Check if data length is a multiple of our expected node size
        const remainder = positionData.byteLength % BINARY_NODE_SIZE;
        if (remainder !== 0) {
          logger.warn(`Binary data size (${positionData.byteLength} bytes) is not a multiple of ${BINARY_NODE_SIZE}. Remainder: ${remainder} bytes`);
        }
      }
      
      // Delegate to worker proxy for processing
      await graphWorkerProxy.processBinaryData(positionData);
      
      if (debugState.isDataDebugEnabled()) {
        logger.debug(`Processed binary data through worker`);
      }
    } catch (error) {
      logger.error('Error processing binary position data:', createErrorMetadata(error));
      
      // Add additional diagnostic information
      if (debugState.isEnabled()) {
        try {
          // Try to display the first few bytes for debugging
          const view = new DataView(positionData);
          const byteArray = [];
          const maxBytesToShow = Math.min(64, positionData.byteLength);
          
          for (let i = 0; i < maxBytesToShow; i++) {
            byteArray.push(view.getUint8(i).toString(16).padStart(2, '0'));
          }
          
          logger.debug(`First ${maxBytesToShow} bytes of binary data: ${byteArray.join(' ')}${positionData.byteLength > maxBytesToShow ? '...' : ''}`);
        } catch (e) {
          logger.debug('Could not display binary data preview:', e);
        }
      }
    }
  }

  // Send node positions to the server via WebSocket
  public async sendNodePositions(): Promise<void> {
    if (!this.binaryUpdatesEnabled || !this.webSocketService) {
      return;
    }

    try {
      // Get current graph data from worker
      const currentData = await graphWorkerProxy.getGraphData();
      
      // Create binary node data array in the format expected by the server
      const binaryNodes: BinaryNodeData[] = currentData.nodes
        .filter(node => node && node.id) // Filter out invalid nodes
        .map(node => {
          // Ensure node has a valid position
          const validatedNode = this.ensureNodeHasValidPosition(node);
          
          // Get numeric ID from map or create a new one
          const numericId = this.nodeIdMap.get(validatedNode.id) || 0;
          if (numericId === 0) {
            logger.warn(`No numeric ID found for node ${validatedNode.id}, skipping`);
            return null;
          }
          
          // Get velocity from metadata or default to zero
          const velocity: Vec3 = (validatedNode.metadata?.velocity as Vec3) || { x: 0, y: 0, z: 0 };
          
          return {
            nodeId: numericId,
            position: {
              x: validatedNode.position.x || 0,
              y: validatedNode.position.y || 0,
              z: validatedNode.position.z || 0
            },
            velocity
          };
        })
        .filter((node): node is BinaryNodeData => node !== null);

      // Create binary buffer using our protocol encoder
      const buffer = createBinaryNodeData(binaryNodes);
      
      // Send the buffer via WebSocket
      this.webSocketService.send(buffer);
      
      if (debugState.isDataDebugEnabled()) {
        logger.debug(`Sent positions for ${binaryNodes.length} nodes using binary protocol`);
      }
    } catch (error) {
      logger.error('Error sending node positions:', createErrorMetadata(error));
    }
  }

  // Add listener for graph data changes
  public onGraphDataChange(listener: GraphDataChangeListener): () => void {
    this.graphDataListeners.push(listener);
    
    // Call immediately with current data from worker
    graphWorkerProxy.getGraphData().then(data => {
      listener(data);
    }).catch(error => {
      logger.error('Error getting initial graph data for listener:', createErrorMetadata(error));
    });
    
    // Return unsubscribe function
    return () => {
      this.graphDataListeners = this.graphDataListeners.filter(l => l !== listener);
    };
  }

  // Add listener for position updates
  public onPositionUpdate(listener: PositionUpdateListener): () => void {
    this.positionUpdateListeners.push(listener);
    
    // Return unsubscribe function
    return () => {
      this.positionUpdateListeners = this.positionUpdateListeners.filter(l => l !== listener);
    };
  }

  // Notify all graph data listeners
  private async notifyGraphDataListeners(): Promise<void> {
    try {
      const currentData = await graphWorkerProxy.getGraphData();
      this.graphDataListeners.forEach(listener => {
        try {
          listener(currentData);
        } catch (error) {
          logger.error('Error in graph data listener:', createErrorMetadata(error));
        }
      });
    } catch (error) {
      logger.error('Error getting graph data for listeners:', createErrorMetadata(error));
    }
  }

  // Notify all position update listeners
  private notifyPositionUpdateListeners(positions: Float32Array): void {
    this.positionUpdateListeners.forEach(listener => {
      try {
        listener(positions);
      } catch (error) {
        logger.error('Error in position update listener:', createErrorMetadata(error));
      }
    });
  }

  // Initialize a node with default position if needed
  public ensureNodeHasValidPosition(node: Node): Node {
    if (!node.position) {
      // Provide a default position if none exists
      return {
        ...node,
        position: { x: 0, y: 0, z: 0 }
      };
    } else if (typeof node.position.x !== 'number' || 
               typeof node.position.y !== 'number' || 
               typeof node.position.z !== 'number') {
      // Fix any NaN or undefined coordinates
      node.position.x = typeof node.position.x === 'number' ? node.position.x : 0;
      node.position.y = typeof node.position.y === 'number' ? node.position.y : 0;
      node.position.z = typeof node.position.z === 'number' ? node.position.z : 0;
    }
    return node;
  }

  // Clean up resources
  public dispose(): void {
    if (this.retryTimeout) {
      window.clearTimeout(this.retryTimeout);
      this.retryTimeout = null;
    }
    
    this.graphDataListeners = [];
    this.positionUpdateListeners = [];
    this.webSocketService = null;
    this.nodeIdMap.clear();
    this.reverseNodeIdMap.clear();
    
    if (debugState.isEnabled()) {
      logger.info('GraphDataManager disposed');
    }
  }
}

// Create singleton instance
export const graphDataManager = GraphDataManager.getInstance();

