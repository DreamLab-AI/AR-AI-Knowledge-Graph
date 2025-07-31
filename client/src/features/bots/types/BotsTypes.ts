// Bots visualization type definitions

// UPDATED: Enhanced agent types to match claude-flow hive-mind system (15+ types including Maestro specs-driven agents)
export interface BotsAgent {
  id: string;
  type: 'coordinator' | 'researcher' | 'coder' | 'analyst' | 'architect' | 'tester' | 'reviewer' | 'optimizer' | 'documenter' | 'monitor' | 'specialist' | 
        'requirements_analyst' | 'design_architect' | 'task_planner' | 'implementation_coder' | 'quality_reviewer' | 'steering_documenter' | 'queen';
  status: 'idle' | 'busy' | 'active' | 'error' | 'initializing' | 'terminating' | 'offline';
  health: number; // 0-100
  cpuUsage: number; // percentage
  memoryUsage: number; // percentage
  workload?: number; // 0-1
  createdAt: string; // ISO 8601
  age: number; // milliseconds
  name?: string;
  
  // UPDATED: Enhanced agent properties from claude-flow hive-mind
  capabilities?: string[]; // agent capabilities like 'task_management', 'code_generation', etc.
  currentTask?: string; // active task description
  tasksActive?: number; // number of active tasks
  tasksCompleted?: number; // total completed tasks
  successRate?: number; // 0-1 success rate
  tokens?: number; // token usage
  tokenRate?: number; // tokens per minute
  activity?: number; // 0-1 activity level
  
  // 3D positioning for force-directed graph
  position?: {
    x: number;
    y: number;
    z: number;
  };
  velocity?: {
    x: number;
    y: number;
    z: number;
  };
  
  // UPDATED: Swarm and hive-mind metadata
  swarmId?: string;
  agentMode?: 'centralized' | 'distributed' | 'strategic';
  parentQueenId?: string; // for hierarchical topologies
}

export interface BotsCommunication {
  id: string;
  type: 'communication';
  timestamp: string;
  sender: string; // agent ID
  receivers: string[]; // agent IDs
  metadata: {
    size: number; // bytes
    type?: string; // communication type
  };
}

export interface TokenUsage {
  total: number;
  byAgent: {
    [agentType: string]: number;
  };
}

export interface BotsEdge {
  id: string;
  source: string;
  target: string;
  dataVolume: number; // total bytes exchanged
  messageCount: number;
  lastMessageTime: number;
}

export interface BotsState {
  agents: Map<string, BotsAgent>;
  edges: Map<string, BotsEdge>;
  communications: BotsCommunication[];
  tokenUsage: TokenUsage;
  lastUpdate: number;
}

// MCP WebSocket message types
export interface MCPMessage {
  type: 'welcome' | 'mcp-update' | 'mcp-response' | 'ping' | 'pong';
  clientId?: string;
  data?: any;
  requestId?: string;
}

export interface MCPRequest {
  jsonrpc: '2.0';
  id: string;
  method: 'tools/call';
  params: {
    name: string;
    arguments: any;
  };
}

// UPDATED: Enhanced visual configuration for all claude-flow hive-mind agent types
export interface BotsVisualConfig {
  colors: {
    // Core agent types
    coordinator: string;
    researcher: string;
    coder: string;
    analyst: string;
    architect: string;
    tester: string;
    reviewer: string;
    optimizer: string;
    documenter: string;
    monitor: string;
    specialist: string;
    
    // UPDATED: Maestro specs-driven agent types
    requirements_analyst: string;
    design_architect: string;
    task_planner: string;
    implementation_coder: string;
    quality_reviewer: string;
    steering_documenter: string;
    
    // UPDATED: Special hive-mind types
    queen: string; // Queen agent gets distinctive color
  };
  
  // UPDATED: Enhanced physics configuration for swarm behavior
  physics: {
    springStrength: number;
    linkDistance: number;
    damping: number;
    nodeRepulsion: number;
    gravityStrength: number;
    maxVelocity: number;
    
    // UPDATED: Hive-mind specific physics
    queenGravity: number; // Additional gravity toward Queen agents
    swarmCohesion: number; // Cohesion within same swarm
    hierarchicalForce: number; // Force for hierarchical topologies
  };
  
  // UPDATED: Agent size configuration based on role importance
  sizes: {
    queen: number; // Largest
    coordinator: number;
    architect: number;
    specialist: number;
    default: number; // Default size for other agents
  };
}