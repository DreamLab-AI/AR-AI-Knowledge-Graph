import React, { useRef, useEffect, useState, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { Html, Text, Billboard, Line as DreiLine } from '@react-three/drei';
import { SwarmAgent, SwarmEdge } from '../types/swarmTypes';
import { createLogger } from '../../../utils/logger';
import { useSettingsStore } from '../../../store/settingsStore';
import { apiService } from '../../../services/api';
import { mcpWebSocketService } from '../services/MCPWebSocketService';
import { useSwarmBinaryUpdates } from '../hooks/useSwarmBinaryUpdates';
import { swarmPhysicsWorker } from '../workers/swarmPhysicsWorker';

const logger = createLogger('SwarmVisualizationEnhanced');

// Enhanced gold and green color palette for swarm as per task.md
const SWARM_COLORS = {
  // Primary agent types - Greens for roles
  coder: '#2ECC71',       // Emerald green
  tester: '#27AE60',      // Nephritis green  
  researcher: '#1ABC9C',  // Turquoise
  reviewer: '#16A085',    // Green sea
  documenter: '#229954',  // Forest green
  specialist: '#239B56',  // Emerald dark
  
  // Meta roles - Golds for coordination
  coordinator: '#F1C40F', // Sunflower gold
  analyst: '#F39C12',     // Orange gold
  architect: '#E67E22',   // Carrot gold
  optimizer: '#D68910',   // Dark gold
  monitor: '#D4AC0D',     // Bright gold
};

// Health color gradient
const getHealthColor = (health: number): THREE.Color => {
  const healthColor = new THREE.Color();
  if (health > 80) {
    healthColor.setHex(0x2ECC71); // Green
  } else if (health > 50) {
    healthColor.setHex(0xF1C40F); // Gold
  } else if (health > 30) {
    healthColor.setHex(0xE67E22); // Orange
  } else {
    healthColor.setHex(0xE74C3C); // Red
  }
  return healthColor;
};

interface SwarmNodeProps {
  agent: SwarmAgent;
  position: THREE.Vector3;
  index: number;
}

const SwarmNode: React.FC<SwarmNodeProps> = ({ agent, position, index }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);
  const borderRef = useRef<THREE.Mesh>(null);
  const [hover, setHover] = useState(false);
  
  // Visual properties based on agent data
  const color = new THREE.Color(SWARM_COLORS[agent.type] || '#CCCCCC');
  const baseSize = 0.5;
  const size = baseSize + (agent.workload || agent.cpuUsage / 100) * 1.5;
  const healthColor = getHealthColor(agent.health);
  
  // Pulse animation based on CPU usage
  useFrame((state) => {
    if (!meshRef.current || !glowRef.current) return;
    
    const pulseSpeed = 1 + agent.cpuUsage / 20;
    const pulseScale = 1 + Math.sin(state.clock.elapsedTime * pulseSpeed) * 0.1;
    
    // Pulse glow
    if (glowRef.current) {
      glowRef.current.scale.setScalar(pulseScale * 1.2);
      (glowRef.current.material as THREE.MeshBasicMaterial).opacity = 
        0.2 + agent.cpuUsage / 200 + (hover ? 0.2 : 0);
    }
    
    // Update position
    meshRef.current.position.copy(position);
    if (glowRef.current) glowRef.current.position.copy(position);
    if (borderRef.current) borderRef.current.position.copy(position);
  });
  
  // Shape based on status (as per task.md)
  const geometry = useMemo(() => {
    switch (agent.status) {
      case 'error':
      case 'terminating':
        return new THREE.TetrahedronGeometry(size);
      case 'initializing':
        return new THREE.BoxGeometry(size, size, size);
      case 'idle':
        return new THREE.SphereGeometry(size, 16, 16);
      case 'busy':
      default:
        return new THREE.SphereGeometry(size, 32, 32);
    }
  }, [agent.status, size]);
  
  return (
    <group>
      {/* Main node */}
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
          metalness={0.8}
          roughness={0.2}
        />
      </mesh>
      
      {/* Health border */}
      <mesh ref={borderRef} position={position}>
        <ringGeometry args={[size * 1.3, size * 1.5, 32]} />
        <meshBasicMaterial 
          color={healthColor} 
          transparent 
          opacity={0.8}
          side={THREE.DoubleSide}
        />
      </mesh>
      
      {/* Glow effect */}
      <mesh ref={glowRef} position={position}>
        <sphereGeometry args={[size * 1.8, 16, 16]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0.2}
          side={THREE.BackSide}
        />
      </mesh>
      
      {/* Label with details */}
      <Billboard position={[position.x, position.y + size * 2, position.z]}>
        <Text
          fontSize={hover ? 0.6 : 0.5}
          color={hover ? '#FFFFFF' : color}
          anchorX="center"
          anchorY="middle"
          outlineWidth={0.05}
          outlineColor="#000000"
        >
          {agent.name || agent.id.slice(0, 8)}
          {hover && (
            `\n${Math.round(agent.cpuUsage)}% CPU | ${Math.round(agent.health)}% Health`
          )}
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
  const particlesRef = useRef<THREE.Points>(null);
  
  // Edge thickness based on data volume
  const thickness = Math.min(0.1 + Math.log10(edge.dataVolume + 1) * 0.05, 0.5);
  
  // Animate particles along edge
  useFrame((state) => {
    if (!particlesRef.current) return;
    
    const particlePositions = particlesRef.current.geometry.attributes.position;
    const animationSpeed = edge.messageCount / 10;
    const time = state.clock.elapsedTime * animationSpeed;
    
    // Animate 10 particles along the edge
    for (let i = 0; i < 10; i++) {
      const t = (time + i * 0.1) % 1;
      const x = sourcePos.x + (targetPos.x - sourcePos.x) * t;
      const y = sourcePos.y + (targetPos.y - sourcePos.y) * t;
      const z = sourcePos.z + (targetPos.z - sourcePos.z) * t;
      particlePositions.setXYZ(i, x, y, z);
    }
    particlePositions.needsUpdate = true;
  });
  
  return (
    <group>
      {/* Edge line */}
      <DreiLine
        points={[sourcePos, targetPos]}
        color="#F1C40F"
        lineWidth={thickness * 10}
        transparent
        opacity={0.6}
      />
      
      {/* Animated particles showing data flow */}
      <points ref={particlesRef}>
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={10}
            array={new Float32Array(30)}
            itemSize={3}
          />
        </bufferGeometry>
        <pointsMaterial
          size={0.3}
          color="#FFD700"
          transparent
          opacity={0.9}
          blending={THREE.AdditiveBlending}
          sizeAttenuation={true}
        />
      </points>
    </group>
  );
};

interface SwarmGraphData {
  nodes: SwarmAgent[];
  edges: SwarmEdge[];
  tokenUsage?: { total: number; byAgent: { [key: string]: number } };
}

export const SwarmVisualizationEnhanced: React.FC = () => {
  console.log('[SWARM] SwarmVisualizationEnhanced component mounting...');
  
  const [swarmData, setSwarmData] = useState<SwarmGraphData>({ nodes: [], edges: [] });
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [dataSource, setDataSource] = useState<'mcp' | 'api' | 'mock'>('mock');
  const positionsRef = useRef<Map<string, THREE.Vector3>>(new Map());
  
  // Initialize physics and data connection
  useEffect(() => {
    console.log('[SWARM] useEffect - Starting initialization...');
    let cleanup = false;
    
    const initialize = async () => {
      try {
        console.log('[SWARM] Initializing swarm visualization...');
        setIsLoading(true);
        
        // Initialize physics worker
        swarmPhysicsWorker.init();
        
        // Try MCP connection first
        try {
          await mcpWebSocketService.connect();
          setDataSource('mcp');
          logger.info('Connected to MCP server');
          
          // Set up real-time updates
          mcpWebSocketService.on('update', handleMCPUpdate);
          
          // Fetch initial data
          await fetchMCPData();
        } catch (mcpError) {
          logger.warn('MCP connection failed, falling back to API:', mcpError);
          
          // Try API fallback
          try {
            const data = await apiService.getSwarmData();
            setDataSource(data._isMock ? 'mock' : 'api');
            processSwarmData(data);
          } catch (apiError) {
            logger.error('API also failed, using mock data:', apiError);
            setDataSource('mock');
            generateMockData();
          }
        }
        
        setError(null);
      } catch (err) {
        logger.error('Initialization failed:', err);
        setError('Failed to initialize swarm visualization');
        generateMockData(); // Use mock data as fallback
      } finally {
        setIsLoading(false);
      }
    };
    
    const fetchMCPData = async () => {
      try {
        const [agents, tokenUsage, communications] = await Promise.all([
          mcpWebSocketService.getAgents(),
          mcpWebSocketService.getTokenUsage(),
          mcpWebSocketService.getCommunications()
        ]);
        
        // Process communications into edges
        const edgeMap = new Map<string, SwarmEdge>();
        communications.forEach(comm => {
          comm.receivers?.forEach(receiver => {
            const edgeKey = [comm.sender, receiver].sort().join('-');
            if (!edgeMap.has(edgeKey)) {
              edgeMap.set(edgeKey, {
                id: edgeKey,
                source: comm.sender,
                target: receiver,
                dataVolume: 0,
                messageCount: 0,
                lastMessageTime: Date.now()
              });
            }
            const edge = edgeMap.get(edgeKey)!;
            edge.dataVolume += comm.metadata?.size || 100;
            edge.messageCount += 1;
          });
        });
        
        setSwarmData({
          nodes: agents,
          edges: Array.from(edgeMap.values()),
          tokenUsage
        });
        
        // Update physics
        swarmPhysicsWorker.updateAgents(agents);
        swarmPhysicsWorker.updateEdges(Array.from(edgeMap.values()));
        if (tokenUsage) swarmPhysicsWorker.updateTokenUsage(tokenUsage);
      } catch (error) {
        logger.error('Error fetching MCP data:', error);
      }
    };
    
    const handleMCPUpdate = (data: any) => {
      if (data.agents) fetchMCPData();
    };
    
    const processSwarmData = (data: any) => {
      // Convert backend format to SwarmAgent format if needed
      let nodes = data.nodes || [];
      if (nodes.length > 0 && nodes[0].metadata) {
        nodes = nodes.map((node: any) => ({
          id: node.metadata_id || node.id.toString(),
          type: node.type || node.node_type || 'specialist',
          status: node.metadata?.status || 'active',
          name: node.label || node.metadata_id || `Agent ${node.id}`,
          cpuUsage: parseFloat(node.metadata?.cpu_usage || '50'),
          memoryUsage: parseFloat(node.metadata?.memory_usage || '50'),
          health: parseFloat(node.metadata?.health || '90'),
          workload: parseFloat(node.metadata?.workload || '0.5'),
          createdAt: new Date().toISOString(),
          age: 0
        }));
      }
      
      setSwarmData({ nodes, edges: data.edges || [] });
      
      // Update physics
      swarmPhysicsWorker.updateAgents(nodes);
      swarmPhysicsWorker.updateEdges(data.edges || []);
    };
    
    const generateMockData = () => {
      console.log('[SWARM] Generating mock data...');
      // Enhanced mock data with all agent types
      const mockAgents: SwarmAgent[] = [
        // Meta roles (Gold)
        { id: 'coord-1', type: 'coordinator', status: 'busy', name: 'Master Coordinator', 
          cpuUsage: 65, memoryUsage: 45, health: 95, workload: 0.8, createdAt: new Date().toISOString(), age: 0 },
        { id: 'analyst-1', type: 'analyst', status: 'busy', name: 'Data Analyst Prime', 
          cpuUsage: 78, memoryUsage: 62, health: 88, workload: 0.9, createdAt: new Date().toISOString(), age: 0 },
        { id: 'arch-1', type: 'architect', status: 'idle', name: 'System Architect', 
          cpuUsage: 25, memoryUsage: 38, health: 92, workload: 0.3, createdAt: new Date().toISOString(), age: 0 },
        
        // Worker roles (Green)
        { id: 'coder-1', type: 'coder', status: 'busy', name: 'Code Builder Alpha', 
          cpuUsage: 85, memoryUsage: 72, health: 78, workload: 0.95, createdAt: new Date().toISOString(), age: 0 },
        { id: 'coder-2', type: 'coder', status: 'busy', name: 'Code Builder Beta', 
          cpuUsage: 72, memoryUsage: 68, health: 82, workload: 0.85, createdAt: new Date().toISOString(), age: 0 },
        { id: 'tester-1', type: 'tester', status: 'idle', name: 'Test Runner', 
          cpuUsage: 35, memoryUsage: 42, health: 90, workload: 0.4, createdAt: new Date().toISOString(), age: 0 },
        { id: 'researcher-1', type: 'researcher', status: 'busy', name: 'Knowledge Seeker', 
          cpuUsage: 58, memoryUsage: 55, health: 93, workload: 0.7, createdAt: new Date().toISOString(), age: 0 },
        { id: 'reviewer-1', type: 'reviewer', status: 'idle', name: 'Code Reviewer', 
          cpuUsage: 42, memoryUsage: 48, health: 87, workload: 0.5, createdAt: new Date().toISOString(), age: 0 }
      ];
      
      const mockEdges: SwarmEdge[] = [
        { id: 'e1', source: 'coord-1', target: 'coder-1', dataVolume: 2048, messageCount: 25, lastMessageTime: Date.now() },
        { id: 'e2', source: 'coord-1', target: 'coder-2', dataVolume: 1536, messageCount: 18, lastMessageTime: Date.now() },
        { id: 'e3', source: 'coord-1', target: 'analyst-1', dataVolume: 3072, messageCount: 32, lastMessageTime: Date.now() },
        { id: 'e4', source: 'coder-1', target: 'tester-1', dataVolume: 1024, messageCount: 15, lastMessageTime: Date.now() },
        { id: 'e5', source: 'coder-2', target: 'tester-1', dataVolume: 768, messageCount: 12, lastMessageTime: Date.now() },
        { id: 'e6', source: 'analyst-1', target: 'researcher-1', dataVolume: 2560, messageCount: 28, lastMessageTime: Date.now() },
        { id: 'e7', source: 'arch-1', target: 'reviewer-1', dataVolume: 512, messageCount: 8, lastMessageTime: Date.now() }
      ];
      
      const mockTokenUsage = {
        total: 15000,
        byAgent: {
          coordinator: 3500,
          analyst: 4200,
          coder: 5800,
          tester: 800,
          researcher: 2200,
          reviewer: 500
        }
      };
      
      setSwarmData({ nodes: mockAgents, edges: mockEdges, tokenUsage: mockTokenUsage });
      
      // Initialize physics with mock data
      swarmPhysicsWorker.updateAgents(mockAgents);
      swarmPhysicsWorker.updateEdges(mockEdges);
      swarmPhysicsWorker.updateTokenUsage(mockTokenUsage);
    };
    
    if (!cleanup) {
      initialize();
    }
    
    // Poll for updates if using MCP
    const pollInterval = dataSource === 'mcp' ? setInterval(fetchMCPData, 3000) : null;
    
    return () => {
      cleanup = true;
      if (pollInterval) clearInterval(pollInterval);
      if (dataSource === 'mcp') {
        mcpWebSocketService.off('update', handleMCPUpdate);
        mcpWebSocketService.disconnect();
      }
      swarmPhysicsWorker.cleanup();
    };
  }, []);
  
  // Update positions from physics simulation
  useFrame(() => {
    const positions = swarmPhysicsWorker.getPositions();
    if (positions) {
      positions.forEach((pos, id) => {
        if (!positionsRef.current.has(id)) {
          positionsRef.current.set(id, new THREE.Vector3());
        }
        positionsRef.current.get(id)!.set(pos.x, pos.y, pos.z);
      });
    }
  });
  
  if (isLoading) {
    console.log('[SWARM] Rendering loading state...');
    return (
      <group position={[60, 0, 0]}>
        {/* Add a simple mesh to verify rendering */}
        <mesh position={[0, 0, 0]}>
          <boxGeometry args={[5, 5, 5]} />
          <meshBasicMaterial color="#F1C40F" />
        </mesh>
        <Html center>
          <div style={{
            background: 'rgba(0, 0, 0, 0.9)',
            padding: '20px',
            borderRadius: '10px',
            border: '3px solid #F1C40F',
            color: '#F1C40F',
            fontSize: '24px',
            fontFamily: 'monospace'
          }}>
            🐝 Loading Swarm Visualization...
          </div>
        </Html>
      </group>
    );
  }
  
  if (error) {
    return (
      <group position={[60, 0, 0]}>
        <Html center>
          <div style={{
            background: 'rgba(0, 0, 0, 0.9)',
            padding: '20px',
            borderRadius: '10px',
            border: '3px solid #E74C3C',
            color: '#E74C3C',
            fontSize: '18px',
            fontFamily: 'monospace'
          }}>
            ⚠️ {error}
          </div>
        </Html>
      </group>
    );
  }
  
  // Position swarm graph to the right of main graph
  return (
    <group position={[60, 0, 0]}>
      {/* Status panel */}
      <Html position={[0, 25, 0]} center>
        <div style={{
          background: 'rgba(0, 0, 0, 0.8)',
          padding: '15px',
          borderRadius: '8px',
          border: '2px solid #F1C40F',
          color: '#fff',
          fontFamily: 'monospace',
          fontSize: '14px',
          minWidth: '250px'
        }}>
          <h3 style={{ margin: '0 0 10px 0', color: '#F1C40F' }}>
            🐝 Agent Swarm ({dataSource.toUpperCase()})
          </h3>
          <div>Agents: {swarmData.nodes.length}</div>
          <div>Communications: {swarmData.edges.length}</div>
          <div>Total Tokens: {swarmData.tokenUsage?.total || 0}</div>
          <div style={{ marginTop: '10px', fontSize: '12px', opacity: 0.8 }}>
            Gold = Coordinators | Green = Workers
          </div>
        </div>
      </Html>
      
      {/* Render nodes */}
      {swarmData.nodes.map((agent, index) => {
        const position = positionsRef.current.get(agent.id) || new THREE.Vector3(
          Math.cos(index / swarmData.nodes.length * Math.PI * 2) * 20,
          Math.sin(index / swarmData.nodes.length * Math.PI * 2) * 20,
          (Math.random() - 0.5) * 10
        );
        
        return (
          <SwarmNode
            key={agent.id}
            agent={agent}
            position={position}
            index={index}
          />
        );
      })}
      
      {/* Render edges */}
      {swarmData.edges.map(edge => {
        const sourcePos = positionsRef.current.get(edge.source);
        const targetPos = positionsRef.current.get(edge.target);
        
        if (sourcePos && targetPos) {
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
      
      {/* Ambient particles for "living hive" effect */}
      <points>
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={300}
            array={new Float32Array(900).map(() => (Math.random() - 0.5) * 80)}
            itemSize={3}
          />
        </bufferGeometry>
        <pointsMaterial
          size={0.08}
          color="#F1C40F"
          transparent
          opacity={0.4}
          blending={THREE.AdditiveBlending}
          sizeAttenuation={true}
        />
      </points>
      
      {/* Ground reference plane */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, -20, 0]}>
        <planeGeometry args={[100, 100, 20, 20]} />
        <meshBasicMaterial
          color="#F1C40F"
          wireframe
          transparent
          opacity={0.1}
        />
      </mesh>
    </group>
  );
};