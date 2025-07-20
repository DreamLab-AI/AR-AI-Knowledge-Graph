// Swarm visualization type definitions

export interface SwarmAgent {
  id: string;
  type: 'coder' | 'tester' | 'coordinator' | 'analyst' | 'researcher' | 'architect' | 'reviewer' | 'optimizer' | 'documenter' | 'monitor' | 'specialist';
  status: 'idle' | 'busy' | 'error' | 'initializing' | 'terminating';
  health: number; // 0-100
  cpuUsage: number; // percentage
  memoryUsage: number; // percentage
  workload?: number; // 0-1
  createdAt: string; // ISO 8601
  age: number; // milliseconds
  name?: string;
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
}

export interface SwarmCommunication {
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

export interface SwarmEdge {
  id: string;
  source: string;
  target: string;
  dataVolume: number; // total bytes exchanged
  messageCount: number;
  lastMessageTime: number;
}

export interface SwarmState {
  agents: Map<string, SwarmAgent>;
  edges: Map<string, SwarmEdge>;
  communications: SwarmCommunication[];
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

// Visual configuration
export interface SwarmVisualConfig {
  colors: {
    coder: string;
    tester: string;
    coordinator: string;
    analyst: string;
    researcher: string;
    architect: string;
    reviewer: string;
    optimizer: string;
    documenter: string;
    monitor: string;
    specialist: string;
  };
  physics: {
    springStrength: number;
    linkDistance: number;
    damping: number;
    nodeRepulsion: number;
    gravityStrength: number;
    maxVelocity: number;
  };
}