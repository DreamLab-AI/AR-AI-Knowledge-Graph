import * as THREE from 'three';
import { createLogger } from '../../../utils/logger';

const logger = createLogger('AgentVisualizationClient');

// Message types matching server protocol
interface InitializeMessage {
  timestamp: number;
  swarm_id: string;
  topology: string;
  agents: AgentInit[];
  connections: ConnectionInit[];
  visual_config: VisualConfig;
  physics_config: PhysicsConfig;
  positions: Record<string, Position>;
}

interface AgentInit {
  id: string;
  name: string;
  type: string;
  status: string;
  color: string;
  shape: string;
  size: number;
  health: number;
  cpu: number;
  memory: number;
  activity: number;
  tasks_active: number;
  tasks_completed: number;
  success_rate: number;
  tokens: number;
  token_rate: number;
  capabilities: string[];
  created_at: number;
}

interface ConnectionInit {
  id: string;
  source: string;
  target: string;
  strength: number;
  flow_rate: number;
  color: string;
  active: boolean;
}

interface PositionUpdate {
  id: string;
  x: number;
  y: number;
  z: number;
  vx?: number;
  vy?: number;
  vz?: number;
}

interface PositionUpdateMessage {
  timestamp: number;
  positions: PositionUpdate[];
}

interface StateUpdate {
  id: string;
  status?: string;
  health?: number;
  cpu?: number;
  memory?: number;
  activity?: number;
  tasks_active?: number;
  current_task?: string;
}

interface Position {
  x: number;
  y: number;
  z: number;
}

interface VisualConfig {
  colors: Record<string, string>;
  sizes: Record<string, number>;
  animations: Record<string, AnimationConfig>;
  effects: EffectsConfig;
}

interface AnimationConfig {
  speed: number;
  amplitude: number;
  enabled: boolean;
}

interface EffectsConfig {
  glow: boolean;
  particles: boolean;
  bloom: boolean;
  shadows: boolean;
}

interface PhysicsConfig {
  spring_strength: number;
  link_distance: number;
  damping: number;
  node_repulsion: number;
  gravity_strength: number;
  max_velocity: number;
}

// Client-side agent representation
export interface VisualizationAgent {
  id: string;
  name: string;
  type: string;
  status: string;
  
  // Visual properties
  position: THREE.Vector3;
  velocity: THREE.Vector3;
  color: THREE.Color;
  shape: string;
  size: number;
  
  // Metrics
  health: number;
  cpu: number;
  memory: number;
  activity: number;
  
  // Tasks
  tasksActive: number;
  tasksCompleted: number;
  successRate: number;
  currentTask?: string;
  
  // Tokens
  tokens: number;
  tokenRate: number;
  
  // Metadata
  capabilities: string[];
  createdAt: Date;
  
  // Rendering state
  mesh?: THREE.Mesh;
  glow?: THREE.Mesh;
  label?: THREE.Object3D;
}

// Client-side connection representation
export interface VisualizationConnection {
  id: string;
  sourceId: string;
  targetId: string;
  strength: number;
  flowRate: number;
  color: THREE.Color;
  active: boolean;
  
  // Rendering state
  line?: THREE.Line;
  particles?: THREE.Points;
}

export class AgentVisualizationClient {
  private agents: Map<string, VisualizationAgent> = new Map();
  private connections: Map<string, VisualizationConnection> = new Map();
  private visualConfig: VisualConfig | null = null;
  private physicsConfig: PhysicsConfig | null = null;
  
  // Position interpolation
  private targetPositions: Map<string, THREE.Vector3> = new Map();
  private interpolationSpeed = 0.1;
  
  // Callbacks
  private onInitialized?: (agents: VisualizationAgent[], connections: VisualizationConnection[]) => void;
  private onPositionUpdate?: (agentId: string, position: THREE.Vector3) => void;
  private onStateUpdate?: (agentId: string, agent: VisualizationAgent) => void;
  private onConnectionUpdate?: (connectionId: string, connection: VisualizationConnection) => void;
  
  constructor() {
    logger.info('AgentVisualizationClient initialized');
  }
  
  /**
   * Process initialization message from server
   */
  public processInitMessage(data: InitializeMessage): void {
    logger.info('Processing initialization message', {
      agentCount: data.agents.length,
      connectionCount: data.connections.length,
      topology: data.topology
    });
    
    // Store configs
    this.visualConfig = data.visual_config;
    this.physicsConfig = data.physics_config;
    
    // Clear existing data
    this.agents.clear();
    this.connections.clear();
    this.targetPositions.clear();
    
    // Process agents
    data.agents.forEach(agentData => {
      const agent: VisualizationAgent = {
        id: agentData.id,
        name: agentData.name,
        type: agentData.type,
        status: agentData.status,
        
        // Initialize position (will be set by physics or server)
        position: new THREE.Vector3(
          data.positions[agentData.id]?.x || Math.random() * 40 - 20,
          data.positions[agentData.id]?.y || Math.random() * 40 - 20,
          data.positions[agentData.id]?.z || Math.random() * 40 - 20
        ),
        velocity: new THREE.Vector3(),
        
        color: new THREE.Color(agentData.color),
        shape: agentData.shape,
        size: agentData.size,
        
        health: agentData.health,
        cpu: agentData.cpu,
        memory: agentData.memory,
        activity: agentData.activity,
        
        tasksActive: agentData.tasks_active,
        tasksCompleted: agentData.tasks_completed,
        successRate: agentData.success_rate,
        
        tokens: agentData.tokens,
        tokenRate: agentData.token_rate,
        
        capabilities: agentData.capabilities,
        createdAt: new Date(agentData.created_at * 1000),
      };
      
      this.agents.set(agent.id, agent);
      this.targetPositions.set(agent.id, agent.position.clone());
    });
    
    // Process connections
    data.connections.forEach(connData => {
      const connection: VisualizationConnection = {
        id: connData.id,
        sourceId: connData.source,
        targetId: connData.target,
        strength: connData.strength,
        flowRate: connData.flow_rate,
        color: new THREE.Color(connData.color),
        active: connData.active,
      };
      
      this.connections.set(connection.id, connection);
    });
    
    // Notify initialization complete
    if (this.onInitialized) {
      this.onInitialized(
        Array.from(this.agents.values()),
        Array.from(this.connections.values())
      );
    }
  }
  
  /**
   * Process position update message from server
   */
  public processPositionUpdate(data: PositionUpdateMessage): void {
    data.positions.forEach(update => {
      // Update target position for smooth interpolation
      const targetPos = this.targetPositions.get(update.id);
      if (targetPos) {
        targetPos.set(update.x, update.y, update.z);
      }
      
      // Update velocity if provided
      const agent = this.agents.get(update.id);
      if (agent && update.vx !== undefined) {
        agent.velocity.set(update.vx, update.vy!, update.vz!);
      }
    });
  }
  
  /**
   * Process state update message from server
   */
  public processStateUpdate(updates: StateUpdate[]): void {
    updates.forEach(update => {
      const agent = this.agents.get(update.id);
      if (!agent) return;
      
      // Update agent properties
      if (update.status !== undefined) agent.status = update.status;
      if (update.health !== undefined) agent.health = update.health;
      if (update.cpu !== undefined) agent.cpu = update.cpu;
      if (update.memory !== undefined) agent.memory = update.memory;
      if (update.activity !== undefined) agent.activity = update.activity;
      if (update.tasks_active !== undefined) agent.tasksActive = update.tasks_active;
      if (update.current_task !== undefined) agent.currentTask = update.current_task;
      
      // Notify state update
      if (this.onStateUpdate) {
        this.onStateUpdate(agent.id, agent);
      }
    });
  }
  
  /**
   * Update positions with interpolation - call this in render loop
   */
  public updatePositions(deltaTime: number): void {
    this.agents.forEach((agent, id) => {
      const targetPos = this.targetPositions.get(id);
      if (!targetPos) return;
      
      // Smooth interpolation
      agent.position.lerp(targetPos, this.interpolationSpeed);
      
      // Notify position update
      if (this.onPositionUpdate) {
        this.onPositionUpdate(agent.id, agent.position);
      }
    });
  }
  
  /**
   * Get agent by ID
   */
  public getAgent(id: string): VisualizationAgent | undefined {
    return this.agents.get(id);
  }
  
  /**
   * Get all agents
   */
  public getAgents(): VisualizationAgent[] {
    return Array.from(this.agents.values());
  }
  
  /**
   * Get connection by ID
   */
  public getConnection(id: string): VisualizationConnection | undefined {
    return this.connections.get(id);
  }
  
  /**
   * Get all connections
   */
  public getConnections(): VisualizationConnection[] {
    return Array.from(this.connections.values());
  }
  
  /**
   * Get visual configuration
   */
  public getVisualConfig(): VisualConfig | null {
    return this.visualConfig;
  }
  
  /**
   * Get physics configuration
   */
  public getPhysicsConfig(): PhysicsConfig | null {
    return this.physicsConfig;
  }
  
  /**
   * Set initialization callback
   */
  public onInit(callback: (agents: VisualizationAgent[], connections: VisualizationConnection[]) => void): void {
    this.onInitialized = callback;
  }
  
  /**
   * Set position update callback
   */
  public onPositionChange(callback: (agentId: string, position: THREE.Vector3) => void): void {
    this.onPositionUpdate = callback;
  }
  
  /**
   * Set state update callback
   */
  public onStateChange(callback: (agentId: string, agent: VisualizationAgent) => void): void {
    this.onStateUpdate = callback;
  }
  
  /**
   * Set connection update callback
   */
  public onConnectionChange(callback: (connectionId: string, connection: VisualizationConnection) => void): void {
    this.onConnectionUpdate = callback;
  }
  
  /**
   * Process raw WebSocket message
   */
  public processMessage(message: any): void {
    switch (message.type) {
      case 'init':
        this.processInitMessage(message as InitializeMessage);
        break;
        
      case 'positions':
        this.processPositionUpdate(message as PositionUpdateMessage);
        break;
        
      case 'state':
        this.processStateUpdate(message.updates);
        break;
        
      case 'connections':
        // TODO: Implement connection updates
        logger.debug('Connection update received', message);
        break;
        
      case 'metrics':
        // TODO: Implement metrics updates
        logger.debug('Metrics update received', message);
        break;
        
      default:
        logger.warn('Unknown message type', message.type);
    }
  }
}