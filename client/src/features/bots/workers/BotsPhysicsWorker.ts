import { BotsAgent, BotsEdge, BotsVisualConfig } from '../types/BotsTypes';

interface PhysicsNode {
  id: string;
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
  mass: number;
  health: number;
  type: string;
}

interface PhysicsEdge {
  source: string;
  target: string;
  strength: number;
}

class BotsPhysicsSimulation {
  private nodes: Map<string, PhysicsNode> = new Map();
  private edges: Map<string, PhysicsEdge> = new Map();
  private dataType: 'visionflow' | 'logseq' = 'visionflow'; // Data type identifier
  private config: BotsVisualConfig['physics'] = {
    springStrength: 0.3,
    linkDistance: 20,
    damping: 0.95,
    nodeRepulsion: 15,
    gravityStrength: 0.1,
    maxVelocity: 0.5
  };

  setDataType(type: 'visionflow' | 'logseq') {
    this.dataType = type;
  }

  updateConfig(config: Partial<BotsVisualConfig['physics']>) {
    this.config = { ...this.config, ...config };
  }

  updateAgents(agents: BotsAgent[]) {
    // Only process VisionFlow agents
    if (this.dataType !== 'visionflow') {
      return;
    }

    // Update existing nodes or create new ones
    agents.forEach(agent => {
      if (!this.nodes.has(agent.id)) {
        // Initialize new node with random position
        const radius = 20;
        const phi = Math.acos(2 * Math.random() - 1);
        const theta = Math.random() * Math.PI * 2;

        this.nodes.set(agent.id, {
          id: agent.id,
          x: radius * Math.sin(phi) * Math.cos(theta),
          y: radius * Math.sin(phi) * Math.sin(theta),
          z: radius * Math.cos(phi),
          vx: 0,
          vy: 0,
          vz: 0,
          mass: 1,
          health: agent.health,
          type: agent.type
        });
      } else {
        // Update existing node properties
        const node = this.nodes.get(agent.id)!;
        node.health = agent.health;
        node.type = agent.type;
      }
    });

    // Remove nodes that no longer exist
    const agentIds = new Set(agents.map(a => a.id));
    Array.from(this.nodes.keys()).forEach(id => {
      if (!agentIds.has(id)) {
        this.nodes.delete(id);
      }
    });
  }

  updateEdges(edges: BotsEdge[]) {
    this.edges.clear();
    edges.forEach(edge => {
      // Calculate spring strength based on communication volume
      const strength = Math.min(edge.dataVolume / 10000, 1) * this.config.springStrength;
      this.edges.set(edge.id, {
        source: edge.source,
        target: edge.target,
        strength
      });
    });
  }

  simulate(tokenUsage?: { byAgent: { [key: string]: number } }) {
    const nodes = Array.from(this.nodes.values());

    // Reset forces
    nodes.forEach(node => {
      node.vx *= this.config.damping;
      node.vy *= this.config.damping;
      node.vz *= this.config.damping;
    });

    // Apply spring forces (edges)
    this.edges.forEach(edge => {
      const source = this.nodes.get(edge.source);
      const target = this.nodes.get(edge.target);

      if (source && target) {
        const dx = target.x - source.x;
        const dy = target.y - source.y;
        const dz = target.z - source.z;
        const distance = Math.sqrt(dx * dx + dy * dy + dz * dz) || 0.001;

        const force = (distance - this.config.linkDistance) * edge.strength;
        const fx = (dx / distance) * force;
        const fy = (dy / distance) * force;
        const fz = (dz / distance) * force;

        source.vx += fx;
        source.vy += fy;
        source.vz += fz;
        target.vx -= fx;
        target.vy -= fy;
        target.vz -= fz;
      }
    });

    // Apply node repulsion
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const nodeA = nodes[i];
        const nodeB = nodes[j];

        const dx = nodeB.x - nodeA.x;
        const dy = nodeB.y - nodeA.y;
        const dz = nodeB.z - nodeA.z;
        const distance = Math.sqrt(dx * dx + dy * dy + dz * dz) || 0.001;

        // Increase repulsion for unhealthy nodes
        const repulsionA = this.config.nodeRepulsion * (2 - nodeA.health / 100);
        const repulsionB = this.config.nodeRepulsion * (2 - nodeB.health / 100);
        const repulsion = (repulsionA + repulsionB) / 2;

        const force = repulsion / (distance * distance);
        const fx = (dx / distance) * force;
        const fy = (dy / distance) * force;
        const fz = (dz / distance) * force;

        nodeA.vx -= fx;
        nodeA.vy -= fy;
        nodeA.vz -= fz;
        nodeB.vx += fx;
        nodeB.vy += fy;
        nodeB.vz += fz;
      }
    }

    // Apply gravity based on token usage
    if (tokenUsage) {
      nodes.forEach(node => {
        const tokens = tokenUsage.byAgent[node.type] || 0;
        const gravity = (tokens / 1000) * this.config.gravityStrength;

        // Pull towards center based on token usage
        node.vx -= node.x * gravity;
        node.vy -= node.y * gravity;
        node.vz -= node.z * gravity;
      });
    }

    // Update positions
    nodes.forEach(node => {
      // Limit velocity
      const velocity = Math.sqrt(node.vx * node.vx + node.vy * node.vy + node.vz * node.vz);
      if (velocity > this.config.maxVelocity) {
        const scale = this.config.maxVelocity / velocity;
        node.vx *= scale;
        node.vy *= scale;
        node.vz *= scale;
      }

      // Update position
      node.x += node.vx;
      node.y += node.vy;
      node.z += node.vz;
    });
  }

  getPositions(): Map<string, { x: number; y: number; z: number }> {
    const positions = new Map();
    this.nodes.forEach((node, id) => {
      positions.set(id, { x: node.x, y: node.y, z: node.z });
    });
    return positions;
  }
}

// Worker interface
class BotsPhysicsWorker {
  private simulation: BotsPhysicsSimulation;
  private animationFrame: number | null = null;
  private tokenUsage: { byAgent: { [key: string]: number } } | undefined;
  private dataType: 'visionflow' | 'logseq' = 'visionflow';

  constructor() {
    this.simulation = new BotsPhysicsSimulation();
  }

  setDataType(type: 'visionflow' | 'logseq') {
    this.dataType = type;
    this.simulation.setDataType(type);
  }

  updateConfig(config: Partial<BotsVisualConfig['physics']>) {
    this.simulation.updateConfig(config);
  }

  init() {
    this.startSimulation();
  }

  updateAgents(agents: BotsAgent[]) {
    // Only update if this is VisionFlow data
    if (this.dataType === 'visionflow') {
      this.simulation.updateAgents(agents);
    }
  }

  updateEdges(edges: BotsEdge[]) {
    // Only update if this is VisionFlow data
    if (this.dataType === 'visionflow') {
      this.simulation.updateEdges(edges);
    }
  }

  updateTokenUsage(usage: { byAgent: { [key: string]: number } }) {
    this.tokenUsage = usage;
  }

  private startSimulation() {
    const tick = () => {
      this.simulation.simulate(this.tokenUsage);
      this.animationFrame = requestAnimationFrame(tick);
    };
    tick();
  }

  getPositions() {
    return this.simulation.getPositions();
  }

  cleanup() {
    if (this.animationFrame !== null) {
      cancelAnimationFrame(this.animationFrame);
      this.animationFrame = null;
    }
  }
}

// Export singleton instance
export const botsPhysicsWorker = new BotsPhysicsWorker();