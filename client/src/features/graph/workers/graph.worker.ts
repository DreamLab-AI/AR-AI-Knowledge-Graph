/**
 * Graph Data Web Worker
 * Handles graph data processing, binary decompression, and position updates off the main thread
 */

import { expose } from 'comlink';
import { BinaryNodeData, parseBinaryNodeData, createBinaryNodeData, Vec3 } from '../../../types/binaryProtocol';

export interface Node {
  id: string;
  label: string;
  position: {
    x: number;
    y: number;
    z: number;
  };
  metadata?: Record<string, any>;
}

export interface Edge {
  id: string;
  source: string;
  target: string;
  label?: string;
  weight?: number;
  metadata?: Record<string, any>;
}

export interface GraphData {
  nodes: Node[];
  edges: Edge[];
}

/**
 * Decompress zlib compressed data in worker thread
 */
async function decompressZlib(compressedData: ArrayBuffer): Promise<ArrayBuffer> {
  if (typeof DecompressionStream !== 'undefined') {
    try {
      const cs = new DecompressionStream('deflate-raw');
      const writer = cs.writable.getWriter();
      writer.write(new Uint8Array(compressedData.slice(2))); // Skip zlib header
      writer.close();

      const output = [];
      const reader = cs.readable.getReader();

      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        output.push(value);
      }

      const totalLength = output.reduce((acc, arr) => acc + arr.length, 0);
      const result = new Uint8Array(totalLength);
      let offset = 0;

      for (const arr of output) {
        result.set(arr, offset);
        offset += arr.length;
      }

      return result.buffer;
    } catch (error) {
      console.error('Worker decompression failed:', error);
      throw error;
    }
  }
  throw new Error('DecompressionStream not available');
}

/**
 * Check if data is zlib compressed
 */
function isZlibCompressed(data: ArrayBuffer): boolean {
  if (data.byteLength < 2) return false;
  const view = new Uint8Array(data);
  return view[0] === 0x78 && [0x01, 0x5E, 0x9C, 0xDA].includes(view[1]);
}

/**
 * Graph Worker API exposed to main thread
 */
class GraphWorker {
  private graphData: GraphData = { nodes: [], edges: [] };
  private nodeIdMap: Map<string, number> = new Map();
  private reverseNodeIdMap: Map<number, string> = new Map();

  // --- NEW STATE FOR ANIMATION ---
  private currentPositions: Float32Array | null = null;
  private targetPositions: Float32Array | null = null;
  private velocities: Float32Array | null = null;
  private pinnedNodeIds: Set<number> = new Set(); // For user-dragged nodes
  private physicsSettings = {
    springStrength: 0.2,
    damping: 0.95,
    maxVelocity: 0.02,
    updateThreshold: 0.05,
  };
  private positionBuffer: SharedArrayBuffer | null = null;
  private positionView: Float32Array | null = null;

  /**
   * Initialize the worker with graph data
   */
  async setGraphData(data: GraphData): Promise<void> {
    this.graphData = {
      nodes: data.nodes.map(node => this.ensureNodeHasValidPosition(node)),
      edges: data.edges
    };

    // (existing ID mapping logic...)
    this.nodeIdMap.clear();
    this.reverseNodeIdMap.clear();
    this.graphData.nodes.forEach((node, index) => {
        const numericId = parseInt(node.id, 10);
        if (!isNaN(numericId) && numericId >= 0 && numericId <= 0xFFFFFFFF) {
            this.nodeIdMap.set(node.id, numericId);
            this.reverseNodeIdMap.set(numericId, node.id);
        } else {
            const mappedId = index + 1;
            this.nodeIdMap.set(node.id, mappedId);
            this.reverseNodeIdMap.set(mappedId, node.id);
        }
    });

    // --- INITIALIZE ANIMATION ARRAYS ---
    const nodeCount = data.nodes.length;
    this.currentPositions = new Float32Array(nodeCount * 3);
    this.targetPositions = new Float32Array(nodeCount * 3);
    this.velocities = new Float32Array(nodeCount * 3).fill(0); // All start with 0 velocity

    data.nodes.forEach((node, index) => {
      const i3 = index * 3;
      const pos = node.position;
      this.currentPositions![i3] = pos.x;
      this.currentPositions![i3 + 1] = pos.y;
      this.currentPositions![i3 + 2] = pos.z;
      // Target starts same as current
      this.targetPositions![i3] = pos.x;
      this.targetPositions![i3 + 1] = pos.y;
      this.targetPositions![i3 + 2] = pos.z;
    });

    console.log(`GraphWorker: Initialized with ${this.graphData.nodes.length} nodes`);
  }

  /**
   * Set up shared array buffer for position data
   */
  async setupSharedPositions(buffer: SharedArrayBuffer): Promise<void> {
    this.positionBuffer = buffer;
    this.positionView = new Float32Array(buffer);
    console.log(`GraphWorker: SharedArrayBuffer set up with ${buffer.byteLength} bytes`);
  }

  // --- NEW METHOD to receive settings from the main thread ---
  async updateSettings(settings: any): Promise<void> {
    this.physicsSettings = {
      springStrength: settings?.visualisation?.physics?.springStrength ?? 0.2,
      damping: settings?.visualisation?.physics?.damping ?? 0.95,
      maxVelocity: settings?.visualisation?.physics?.maxVelocity ?? 0.02,
      updateThreshold: settings?.visualisation?.physics?.updateThreshold ?? 0.05
    };
  }

  /**
   * Process binary position data with decompression
   */
  async processBinaryData(data: ArrayBuffer): Promise<Float32Array> { // The return value is no longer directly used for rendering but can be for listeners.
    // ... (decompression logic remains the same)
    if (isZlibCompressed(data)) {
      data = await decompressZlib(data);
    }
    const nodeUpdates = parseBinaryNodeData(data);

    // Create a flat array for listeners/main thread
    const positionArray = new Float32Array(nodeUpdates.length * 4);

    nodeUpdates.forEach((update, index) => {
      const stringNodeId = this.reverseNodeIdMap.get(update.nodeId);
      if (stringNodeId) {
        const nodeIndex = this.graphData.nodes.findIndex(n => n.id === stringNodeId);
        if (nodeIndex !== -1 && !this.pinnedNodeIds.has(update.nodeId)) {
          // --- THIS IS THE KEY CHANGE ---
          // --- UPDATE TARGET, NOT CURRENT ---
          const i3 = nodeIndex * 3;
          this.targetPositions![i3] = update.position.x;
          this.targetPositions![i3 + 1] = update.position.y;
          this.targetPositions![i3 + 2] = update.position.z;
          // We don't touch currentPositions or velocities here. The `tick` method will handle that.
        }
      }

      const arrayOffset = index * 4;
      positionArray[arrayOffset] = update.nodeId;
      positionArray[arrayOffset + 1] = update.position.x;
      positionArray[arrayOffset + 2] = update.position.y;
      positionArray[arrayOffset + 3] = update.position.z;
    });

    // We still return the raw server positions for any listeners that might need it
    return positionArray;
  }

  /**
   * Get current graph data
   */
  async getGraphData(): Promise<GraphData> {
    return this.graphData;
  }

  /**
   * Add or update a node
   */
  async updateNode(node: Node): Promise<void> {
    const existingIndex = this.graphData.nodes.findIndex(n => n.id === node.id);

    if (existingIndex >= 0) {
      this.graphData.nodes[existingIndex] = { ...this.graphData.nodes[existingIndex], ...node };
    } else {
      this.graphData.nodes.push(this.ensureNodeHasValidPosition(node));

      // Update ID mappings
      const numericId = parseInt(node.id, 10);
      if (!isNaN(numericId)) {
        this.nodeIdMap.set(node.id, numericId);
        this.reverseNodeIdMap.set(numericId, node.id);
      } else {
        const mappedId = this.graphData.nodes.length;
        this.nodeIdMap.set(node.id, mappedId);
        this.reverseNodeIdMap.set(mappedId, node.id);
      }
    }
  }

  /**
   * Remove a node
   */
  async removeNode(nodeId: string): Promise<void> {
    const numericId = this.nodeIdMap.get(nodeId);

    this.graphData.nodes = this.graphData.nodes.filter(node => node.id !== nodeId);
    this.graphData.edges = this.graphData.edges.filter(
      edge => edge.source !== nodeId && edge.target !== nodeId
    );

    if (numericId !== undefined) {
      this.nodeIdMap.delete(nodeId);
      this.reverseNodeIdMap.delete(numericId);
    }
  }

  /**
   * Create binary node data
   */
  async createBinaryData(nodes: BinaryNodeData[]): Promise<ArrayBuffer> {
    return createBinaryNodeData(nodes);
  }

  private ensureNodeHasValidPosition(node: Node): Node {
    if (!node.position) {
      return { ...node, position: { x: 0, y: 0, z: 0 } };
    }

    return {
      ...node,
      position: {
        x: typeof node.position.x === 'number' ? node.position.x : 0,
        y: typeof node.position.y === 'number' ? node.position.y : 0,
        z: typeof node.position.z === 'number' ? node.position.z : 0
      }
    };
  }

  // --- NEW METHODS for managing user drag interactions ---
  async pinNode(nodeId: number): Promise<void> { this.pinnedNodeIds.add(nodeId); }
  async unpinNode(nodeId: number): Promise<void> { this.pinnedNodeIds.delete(nodeId); }

  async updateUserDrivenNodePosition(nodeId: number, position: Vec3): Promise<void> {
    const stringNodeId = this.reverseNodeIdMap.get(nodeId);
    if (stringNodeId) {
      const nodeIndex = this.graphData.nodes.findIndex(n => n.id === stringNodeId);
      if (nodeIndex !== -1) {
        const i3 = nodeIndex * 3;
        // For user drags, update both current and target to snap the node and stop it from moving
        this.currentPositions![i3] = position.x;
        this.currentPositions![i3 + 1] = position.y;
        this.currentPositions![i3 + 2] = position.z;
        this.targetPositions![i3] = position.x;
        this.targetPositions![i3 + 1] = position.y;
        this.targetPositions![i3 + 2] = position.z;
        // Reset velocity
        this.velocities!.fill(0, i3, i3 + 3);
      }
    }
  }

  // --- NEW: The core animation method, called on every frame from the main thread ---
  async tick(deltaTime: number): Promise<Float32Array> {
    if (!this.currentPositions || !this.targetPositions || !this.velocities) {
      return new Float32Array(0);
    }

    const { springStrength, damping, maxVelocity, updateThreshold } = this.physicsSettings;

    for (let i = 0; i < this.graphData.nodes.length; i++) {
      const numericId = this.nodeIdMap.get(this.graphData.nodes[i].id)!;
      if (this.pinnedNodeIds.has(numericId)) continue; // Skip physics for pinned (dragged) nodes

      const i3 = i * 3;

      const dx = this.targetPositions[i3] - this.currentPositions[i3];
      const dy = this.targetPositions[i3 + 1] - this.currentPositions[i3 + 1];
      const dz = this.targetPositions[i3 + 2] - this.currentPositions[i3 + 2];

      const distSq = dx * dx + dy * dy + dz * dz;

      // Apply tolerance check
      if (distSq < updateThreshold * updateThreshold) {
        this.velocities.fill(0, i3, i3 + 3); // Node has settled
        continue;
      }

      // Spring force (F = k * x), assuming mass = 1
      let ax = dx * springStrength;
      let ay = dy * springStrength;
      let az = dz * springStrength;

      // Update velocity with acceleration
      this.velocities[i3] += ax * deltaTime;
      this.velocities[i3 + 1] += ay * deltaTime;
      this.velocities[i3 + 2] += az * deltaTime;

      // Apply damping
      this.velocities[i3] *= damping;
      this.velocities[i3 + 1] *= damping;
      this.velocities[i3 + 2] *= damping;

      // Update position with velocity
      this.currentPositions[i3] += this.velocities[i3] * deltaTime;
      this.currentPositions[i3 + 1] += this.velocities[i3 + 1] * deltaTime;
      this.currentPositions[i3 + 2] += this.velocities[i3 + 2] * deltaTime;
    }

    // Return the smoothly animated positions for rendering
    return this.currentPositions;
  }
}

// Expose the worker API using Comlink
const worker = new GraphWorker();
expose(worker);

export type GraphWorkerType = GraphWorker;