/**
 * Graph Type Definitions
 * Common types for graph data management
 */

export type GraphType = 'logseq' | 'visionflow';

export interface GraphNode {
  id: string;
  label: string;
  position: {
    x: number;
    y: number;
    z: number;
  };
  metadata?: Record<string, any>;
  graphType?: GraphType; // Optional graph type identifier
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  label?: string;
  weight?: number;
  metadata?: Record<string, any>;
  graphType?: GraphType; // Optional graph type identifier
}

export interface TypedGraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
  graphType: GraphType;
  lastUpdate?: number;
}

// Message types for graph updates
export interface GraphUpdateMessage {
  type: 'node-update' | 'edge-update' | 'position-update' | 'bulk-update';
  graphType: GraphType;
  data: any;
  timestamp: number;
}

// Physics settings per graph type
export interface GraphPhysicsConfig {
  springStrength: number;
  damping: number;
  maxVelocity: number;
  updateThreshold: number;
  nodeRepulsion?: number;
  linkDistance?: number;
  gravityStrength?: number;
}

export interface GraphTypeConfig {
  logseq: {
    physics: GraphPhysicsConfig;
    rendering: {
      nodeSize: number;
      edgeWidth: number;
      labelSize: number;
    };
  };
  visionflow: {
    physics: GraphPhysicsConfig;
    rendering: {
      agentSize: number;
      connectionWidth: number;
      healthIndicator: boolean;
    };
  };
}

// Default configurations
export const DEFAULT_GRAPH_CONFIG: GraphTypeConfig = {
  logseq: {
    physics: {
      springStrength: 0.2,
      damping: 0.95,
      maxVelocity: 0.02,
      updateThreshold: 0.05,
      nodeRepulsion: 10,
      linkDistance: 30
    },
    rendering: {
      nodeSize: 5,
      edgeWidth: 1,
      labelSize: 12
    }
  },
  visionflow: {
    physics: {
      springStrength: 0.3,
      damping: 0.95,
      maxVelocity: 0.5,
      updateThreshold: 0.1,
      nodeRepulsion: 15,
      linkDistance: 20,
      gravityStrength: 0.1
    },
    rendering: {
      agentSize: 8,
      connectionWidth: 2,
      healthIndicator: true
    }
  }
};