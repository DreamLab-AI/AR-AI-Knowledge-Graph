import React, { useRef, useEffect, useState, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import { Text, Billboard, Line } from '@react-three/drei';
import * as THREE from 'three';
import { SwarmAgent, SwarmEdge, SwarmState, SwarmVisualConfig } from '../types/swarmTypes';
import { swarmPhysicsWorker } from '../workers/swarmPhysicsWorker';
import { mcpWebSocketService } from '../services/MCPWebSocketService';
import { MockSwarmDataProvider } from '../services/MockSwarmDataProvider';
import { SwarmStatusIndicator } from './SwarmStatusIndicator';
import { createLogger } from '../../../utils/logger';
import { apiService } from '../../../services/api';

const logger = createLogger('SwarmVisualization');

// Use mock provider as fallback
const mockProvider = new MockSwarmDataProvider();

// Gold and green color palette as specified
const VISUAL_CONFIG: SwarmVisualConfig = {
  colors: {
    coder: '#2ECC71',       // Emerald green
    tester: '#27AE60',      // Nephritis green
    coordinator: '#F1C40F', // Sunflower gold
    analyst: '#F39C12',     // Orange gold
    researcher: '#1ABC9C',  // Turquoise green
    architect: '#E67E22',   // Carrot gold
    reviewer: '#16A085',    // Green sea
    optimizer: '#D68910',   // Dark gold
    documenter: '#229954',  // Medium green
    monitor: '#D4AC0D',     // Gold yellow
    specialist: '#239B56'   // Dark green
  },
  physics: {
    springStrength: 0.3,
    linkDistance: 20,
    damping: 0.95,
    nodeRepulsion: 15,
    gravityStrength: 0.1,
    maxVelocity: 0.5
  }
};

interface SwarmNodeProps {
  agent: SwarmAgent;
  index: number;
}

const SwarmNode: React.FC<SwarmNodeProps> = ({ agent, index }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);
  const [hover, setHover] = useState(false);

  // Calculate visual properties based on agent data
  const color = new THREE.Color(VISUAL_CONFIG.colors[agent.type] || '#CCCCCC');
  const size = 0.5 + (agent.workload || agent.cpuUsage / 100) * 1.5; // Size based on workload
  const healthColor = new THREE.Color().lerpColors(
    new THREE.Color('#E74C3C'), // Red for low health
    new THREE.Color('#2ECC71'), // Green for high health
    agent.health / 100
  );

  // Shape based on status
  const geometry = useMemo(() => {
    switch (agent.status) {
      case 'error':
      case 'terminating':
        return new THREE.TetrahedronGeometry(size);
      case 'initializing':
        return new THREE.BoxGeometry(size, size, size);
      default:
        return new THREE.SphereGeometry(size, 32, 16);
    }
  }, [agent.status, size]);

  // Animate pulse based on CPU usage
  useFrame((state) => {
    if (meshRef.current && glowRef.current) {
      const pulseSpeed = 1 + agent.cpuUsage / 20; // Faster pulse for higher CPU
      const pulseScale = 1 + Math.sin(state.clock.elapsedTime * pulseSpeed) * 0.1;
      glowRef.current.scale.setScalar(pulseScale);

      // Update position from physics engine
      if (agent.position) {
        meshRef.current.position.set(agent.position.x, agent.position.y, agent.position.z);
        glowRef.current.position.set(agent.position.x, agent.position.y, agent.position.z);
      }
    }
  });

  return (
    <group>
      {/* Main node mesh */}
      <mesh
        ref={meshRef}
        geometry={geometry}
        onPointerOver={() => setHover(true)}
        onPointerOut={() => setHover(false)}
      >
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={0.3}
          metalness={0.7}
          roughness={0.3}
        />
      </mesh>

      {/* Health indicator border */}
      <mesh position={agent.position ? [agent.position.x, agent.position.y, agent.position.z] : [0, 0, 0]}>
        <ringGeometry args={[size * 1.2, size * 1.4, 32]} />
        <meshBasicMaterial color={healthColor} transparent opacity={0.8} />
      </mesh>

      {/* Glow effect */}
      <mesh ref={glowRef} position={agent.position ? [agent.position.x, agent.position.y, agent.position.z] : [0, 0, 0]}>
        <sphereGeometry args={[size * 1.5, 16, 16]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0.2 + agent.cpuUsage / 200}
          side={THREE.BackSide}
        />
      </mesh>

      {/* Label */}
      <Billboard position={agent.position ? [agent.position.x, agent.position.y + size * 2, agent.position.z] : [0, size * 2, 0]}>
        <Text
          fontSize={0.5}
          color={hover ? '#FFFFFF' : '#CCCCCC'}
          anchorX="center"
          anchorY="middle"
        >
          {agent.name || agent.id.slice(0, 8)}
        </Text>
      </Billboard>
    </group>
  );
};

interface SwarmEdgeProps {
  edge: SwarmEdge;
  sourcePos: THREE.Vector3;
  targetPos: THREE.Vector3;
}

const SwarmEdgeComponent: React.FC<SwarmEdgeProps> = ({ edge, sourcePos, targetPos }) => {
  const lineRef = useRef<THREE.Line>(null);
  const particlesRef = useRef<THREE.Points>(null);

  // Calculate edge thickness based on data volume
  const thickness = Math.min(0.1 + Math.log10(edge.dataVolume + 1) * 0.05, 0.5);

  // Animation speed based on message frequency
  const animationSpeed = edge.messageCount / 10;

  useFrame((state) => {
    if (particlesRef.current) {
      // Animate particles along the edge
      const time = state.clock.elapsedTime * animationSpeed;
      const particlePositions = particlesRef.current.geometry.attributes.position;

      for (let i = 0; i < particlePositions.count; i++) {
        const t = (time + i * 0.1) % 1;
        const x = sourcePos.x + (targetPos.x - sourcePos.x) * t;
        const y = sourcePos.y + (targetPos.y - sourcePos.y) * t;
        const z = sourcePos.z + (targetPos.z - sourcePos.z) * t;
        particlePositions.setXYZ(i, x, y, z);
      }
      particlePositions.needsUpdate = true;
    }
  });

  const points = useMemo(() => [sourcePos, targetPos], [sourcePos, targetPos]);

  return (
    <group>
      {/* Edge line */}
      <Line
        ref={lineRef}
        points={points}
        color="#F1C40F"
        lineWidth={thickness}
        transparent
        opacity={0.6}
      />

      {/* Animated particles */}
      <points ref={particlesRef}>
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={5}
            array={new Float32Array(15)}
            itemSize={3}
          />
        </bufferGeometry>
        <pointsMaterial
          size={0.2}
          color="#FFD700"
          transparent
          opacity={0.8}
          blending={THREE.AdditiveBlending}
        />
      </points>
    </group>
  );
};

export const SwarmVisualization: React.FC = () => {
  const [swarmState, setSwarmState] = useState<SwarmState>({
    agents: new Map(),
    edges: new Map(),
    communications: [],
    tokenUsage: { total: 0, byAgent: {} },
    lastUpdate: Date.now()
  });
  const [connected, setConnected] = useState(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);

  const groupRef = useRef<THREE.Group>(null);

  // Initialize MCP connection and physics worker
  useEffect(() => {
    // Start physics worker
    swarmPhysicsWorker.init();

    // Fetch initial data from backend
    fetchSwarmDataFromBackend();
    setConnected(true);

    // Poll for updates every 2 seconds
    const pollInterval = setInterval(fetchSwarmDataFromBackend, 2000);

    return () => {
      swarmPhysicsWorker.cleanup();
      clearInterval(pollInterval);
    };
  }, []);

  const handleMCPUpdate = async (data: any) => {
    try {
      // Process agent data
      if (data.agents) {
        const agents = new Map<string, SwarmAgent>();
        data.agents.forEach((agent: SwarmAgent) => {
          agents.set(agent.id, agent);
        });

        // Update physics simulation
        swarmPhysicsWorker.updateAgents(Array.from(agents.values()));

        setSwarmState(prev => ({
          ...prev,
          agents,
          lastUpdate: Date.now()
        }));
      }

      // Process token usage
      if (data.tokenUsage) {
        setSwarmState(prev => ({
          ...prev,
          tokenUsage: data.tokenUsage
        }));
      }
    } catch (error) {
      logger.error('Error handling MCP update:', error);
    }
  };

  const fetchSwarmDataFromBackend = async () => {
    try {
      // Fetch swarm data from backend API
      const response = await apiService.getSwarmData();
      
      if (response && response.nodes) {
        logger.info('Received swarm data from backend:', response.nodes.length, 'nodes');
        
        // Convert backend nodes to SwarmAgent format
        const agents: SwarmAgent[] = response.nodes.map((node: any) => ({
          id: node.metadata_id || node.id.toString(),
          name: node.label,
          type: node.node_type || node.metadata?.type || 'unknown',
          status: node.metadata?.status || 'active',
          cpuUsage: parseFloat(node.metadata?.cpu_usage || '50'),
          health: parseFloat(node.metadata?.health || '90'),
          workload: parseFloat(node.metadata?.workload || node.weight || '0.5'),
          position: node.data?.position ? {
            x: node.data.position.x,
            y: node.data.position.y,
            z: node.data.position.z
          } : undefined
        }));

        const agentMap = new Map<string, SwarmAgent>();
        agents.forEach(agent => agentMap.set(agent.id, agent));

        // Convert edges to SwarmEdge format
        const edgeMap = new Map<string, SwarmEdge>();
        if (response.edges) {
          response.edges.forEach((edge: any) => {
            const swarmEdge: SwarmEdge = {
              id: edge.id,
              source: edge.source.toString(),
              target: edge.target.toString(),
              dataVolume: edge.weight * 1024, // Convert to bytes
              messageCount: parseInt(edge.edge_type?.match(/\d+/)?.[0] || '10'),
              lastMessageTime: Date.now()
            };
            edgeMap.set(edge.id, swarmEdge);
          });
        }

        setSwarmState(prev => ({
          ...prev,
          agents: agentMap,
          edges: edgeMap,
          lastUpdate: Date.now()
        }));

        // Update physics with agent data
        swarmPhysicsWorker.updateAgents(agents);
        swarmPhysicsWorker.updateEdges(Array.from(edgeMap.values()));
      } else {
        logger.warn('No swarm data received from backend');
      }
    } catch (error) {
      logger.error('Error fetching swarm data from backend:', error);
      setConnectionError('Failed to fetch swarm data from server');
      
      // Fall back to mock data if backend fails
      logger.info('Falling back to mock data');
      fetchMockData();
    }
  };

  const fetchMockData = async () => {
    try {
      await mockProvider.connect();
      const agents = await mockProvider.getAgents();
      const agentMap = new Map<string, SwarmAgent>();
      agents.forEach(agent => agentMap.set(agent.id, agent));

      setSwarmState(prev => ({
        ...prev,
        agents: agentMap,
        lastUpdate: Date.now()
      }));

      swarmPhysicsWorker.updateAgents(agents);
    } catch (error) {
      logger.error('Error with mock data:', error);
    }
  };

  // Communications are now handled by edge data from backend
  const pollCommunications = async () => {
    // This is now handled by fetchSwarmDataFromBackend which includes edges
  };

  // Update positions from physics simulation
  useFrame(() => {
    const positions = swarmPhysicsWorker.getPositions();
    if (positions && positions.size > 0) {
      setSwarmState(prev => {
        const newAgents = new Map(prev.agents);
        positions.forEach((pos, id) => {
          const agent = newAgents.get(id);
          if (agent) {
            agent.position = pos;
          }
        });
        return { ...prev, agents: newAgents };
      });
    }
  });

  // Position the swarm visualization to the side of the main graph
  return (
    <group ref={groupRef} position={[0, 0, 0]}>
      {/* Status indicator */}
      <SwarmStatusIndicator
        agentCount={swarmState.agents.size}
        edgeCount={swarmState.edges.size}
        totalTokens={swarmState.tokenUsage.total}
        connected={connected}
      />

      {/* Error message if not connected */}
      {connectionError && (
        <group>
          <mesh position={[0, 0, 0]}>
            <tetrahedronGeometry args={[5]} />
            <meshBasicMaterial color="#000000" />
          </mesh>
          <Billboard position={[0, 0, 0]}>
            <Text
              fontSize={0.5}
              color="#FFFFFF"
              anchorX="center"
              anchorY="middle"
            >
              !
            </Text>
          </Billboard>
          <Billboard position={[0, -3, 0]}>
            <Text
              fontSize={0.3}
              color="#E74C3C"
              anchorX="center"
              anchorY="middle"
              maxWidth={10}
            >
              {connectionError}
            </Text>
          </Billboard>
        </group>
      )}

      {/* Render nodes */}
      {Array.from(swarmState.agents.values()).map((agent, index) => (
        <SwarmNode key={agent.id} agent={agent} index={index} />
      ))}

      {/* Render edges */}
      {Array.from(swarmState.edges.values()).map(edge => {
        const sourceAgent = swarmState.agents.get(edge.source);
        const targetAgent = swarmState.agents.get(edge.target);

        if (sourceAgent?.position && targetAgent?.position) {
          const sourcePos = new THREE.Vector3(
            sourceAgent.position.x,
            sourceAgent.position.y,
            sourceAgent.position.z
          );
          const targetPos = new THREE.Vector3(
            targetAgent.position.x,
            targetAgent.position.y,
            targetAgent.position.z
          );

          return (
            <SwarmEdgeComponent
              key={edge.id}
              edge={edge}
              sourcePos={sourcePos}
              targetPos={targetPos}
            />
          );
        }
        return null;
      })}

      {/* Ambient particle field for "living hive" effect */}
      <points>
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={100}
            array={new Float32Array(300).map(() => (Math.random() - 0.5) * 50)}
            itemSize={3}
          />
        </bufferGeometry>
        <pointsMaterial
          size={0.05}
          color="#F1C40F"
          transparent
          opacity={0.3}
          blending={THREE.AdditiveBlending}
        />
      </points>
    </group>
  );
};