import React, { useRef, useEffect, useState, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { Html } from '@react-three/drei';
import { Line } from '@react-three/drei/core/Line';
import { SwarmAgent, SwarmEdge } from '../types/swarmTypes';
import { createLogger } from '../../../utils/logger';
import { debugState } from '../../../utils/debugState';
import { useSettingsStore } from '../../../store/settingsStore';
import { apiService } from '../../../services/api';
import { mcpWebSocketService } from '../services/MCPWebSocketService';
import { useSwarmBinaryUpdates } from '../hooks/useSwarmBinaryUpdates';
import { SwarmDebugInfo } from './SwarmVisualizationDebugInfo';

const logger = createLogger('SwarmVisualizationIntegrated');

// Gold and green color palette for swarm
const SWARM_COLORS = {
  coder: '#2ECC71',      // Green
  tester: '#27AE60',     // Dark Green
  coordinator: '#F1C40F', // Gold
  analyst: '#F39C12',    // Dark Gold
  researcher: '#1ABC9C', // Turquoise
  architect: '#E67E22',  // Orange
  reviewer: '#16A085',   // Teal
  optimizer: '#D68910',  // Dark Gold
  documenter: '#229954', // Forest Green
  monitor: '#D4AC0D',    // Bright Gold
  specialist: '#239B56'  // Emerald
};

interface SwarmGraphData {
  nodes: SwarmAgent[];
  edges: SwarmEdge[];
  positions?: Float32Array;
}

export const SwarmVisualizationIntegrated: React.FC = () => {
  // Immediate debug log
  console.log('[SWARM] Component mounting...');
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const labelsRef = useRef<THREE.Group>(null);
  const [swarmData, setSwarmData] = useState<SwarmGraphData>({ nodes: [], edges: [] });
  const [edgePoints, setEdgePoints] = useState<number[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [dataSource, setDataSource] = useState<'mcp' | 'api' | 'mock' | 'none'>('none');
  const [mcpConnected, setMcpConnected] = useState(false);
  const settings = useSettingsStore(state => state.settings);
  
  // Position buffer from backend
  const positionsRef = useRef<Float32Array | null>(null);
  
  // Binary position updates from WebSocket
  const { positions: binaryPositions } = useSwarmBinaryUpdates({
    enabled: !isLoading && swarmData.nodes.length > 0,
    onPositionUpdate: (positions) => {
      // Update the positions ref when binary updates arrive
      positionsRef.current = positions;
      
      // Trigger re-render by updating state
      setSwarmData(prev => ({
        ...prev,
        positions: positions
      }));
    }
  });

  // Try to connect to MCP server first, fallback to backend API
  useEffect(() => {
    let isConnected = false;
    let pollInterval: NodeJS.Timeout | null = null;

    const connectToMCP = async () => {
      try {
        setIsLoading(true);
        logger.info('Attempting to connect to MCP server...');
        console.log('[SWARM] Attempting MCP connection...');
        
        // Try to connect to MCP WebSocket
        await mcpWebSocketService.connect();
        isConnected = true;
        setMcpConnected(true);
        setDataSource('mcp');
        
        logger.info('Connected to MCP server, fetching agent data...');
        console.log('[SWARM] Connected to MCP, fetching data...');
        
        // Set up real-time updates
        mcpWebSocketService.on('update', handleMCPUpdate);
        
        // Fetch initial data
        await fetchMCPData();
        
        // Poll for communications more frequently
        pollInterval = setInterval(pollCommunications, 2000);
        
        setError(null);
      } catch (mcpError) {
        logger.warn('MCP connection failed, falling back to backend API:', mcpError);
        console.log('[SWARM] MCP failed, trying backend API...', mcpError);
        
        // Fallback to backend API
        try {
          setMcpConnected(false);
          logger.info('Trying backend API for swarm data...');
          console.log('[SWARM] Fetching from backend API...');
          const data = await apiService.getSwarmData();
          console.log('[SWARM] Backend API response:', data);
          
          // Convert backend data format if needed
          const mappedData = {
            nodes: data.nodes || [],
            edges: data.edges || [],
            positions: data.positions || null
          };
          
          // Check if we got mock or real data
          if (data._isMock) {
            setDataSource('mock');
            logger.info('Using mock swarm data');
            console.log('[SWARM] Using mock data');
          } else {
            setDataSource('api');
            logger.info('Using API swarm data');
            console.log('[SWARM] Using real API data');
          }
          
          // Convert nodes if they have the backend format
          if (mappedData.nodes.length > 0 && mappedData.nodes[0].data) {
            // Backend format detected, convert to SwarmAgent format
            mappedData.nodes = mappedData.nodes.map((node: any) => ({
              id: node.metadataId || node.id.toString(),
              type: node.type || 'specialist',
              status: node.metadata?.status || 'active',
              name: node.label || node.metadataId || `Agent ${node.id}`,
              cpuUsage: parseFloat(node.metadata?.cpu_usage || '50'),
              memoryUsage: parseFloat(node.metadata?.memory_usage || '50'),
              health: parseFloat(node.metadata?.health || '90'),
              workload: parseFloat(node.metadata?.workload || '0.5'),
              createdAt: new Date().toISOString(),
              age: 0,
              position: node.data?.position ? {
                x: node.data.position.x,
                y: node.data.position.y,
                z: node.data.position.z
              } : undefined
            }));
            console.log('[SWARM] Converted backend format nodes:', mappedData.nodes);
          }
          
          setSwarmData(mappedData);
          console.log('[SWARM] Set swarm data:', { nodes: mappedData.nodes.length, edges: mappedData.edges.length });
          
          if (mappedData.positions) {
            positionsRef.current = new Float32Array(mappedData.positions);
          } else if (mappedData.nodes.length > 0) {
            // Initialize positions from node data if available
            const positions = new Float32Array(mappedData.nodes.length * 3);
            mappedData.nodes.forEach((node: any, index: number) => {
              if (node.data && node.data.position) {
                positions[index * 3] = node.data.position.x || 0;
                positions[index * 3 + 1] = node.data.position.y || 0;
                positions[index * 3 + 2] = node.data.position.z || 0;
              } else {
                // Default circular layout
                const angle = (index / mappedData.nodes.length) * Math.PI * 2;
                const radius = 15;
                positions[index * 3] = Math.cos(angle) * radius;
                positions[index * 3 + 1] = Math.sin(angle) * radius;
                positions[index * 3 + 2] = (Math.random() - 0.5) * 10;
              }
            });
            positionsRef.current = positions;
            console.log('[SWARM] Initialized positions for', mappedData.nodes.length, 'nodes');
          }
          
          setError(null);
          logger.info(`Loaded swarm data from backend: ${mappedData.nodes.length} agents`);
        } catch (apiError) {
          logger.error('Both MCP and API failed:', apiError);
          console.error('[SWARM] API failed:', apiError);
          setError('Swarm visualization unavailable');
        }
      } finally {
        setIsLoading(false);
      }
    };

    const fetchMCPData = async () => {
      try {
        // Fetch agents
        const agents = await mcpWebSocketService.getAgents();
        
        // Fetch token usage
        const tokenUsage = await mcpWebSocketService.getTokenUsage();
        
        // Map MCP agent data to our format
        const mappedAgents: SwarmAgent[] = agents.map(agent => ({
          id: agent.id,
          type: agent.type || 'specialist',
          status: agent.status || 'active',
          name: agent.name || agent.id,
          cpuUsage: agent.metrics?.cpuUsage || Math.random() * 100,
          memoryUsage: agent.metrics?.memoryUsage || Math.random() * 100,
          health: agent.health || 90,
          workload: agent.workload || Math.random(),
          createdAt: new Date().toISOString(),
          age: 0
        }));

        // Initialize positions if needed
        if (!positionsRef.current || positionsRef.current.length !== mappedAgents.length * 3) {
          const positions = new Float32Array(mappedAgents.length * 3);
          mappedAgents.forEach((_, index) => {
            const angle = (index / mappedAgents.length) * Math.PI * 2;
            const radius = 15;
            positions[index * 3] = Math.cos(angle) * radius;
            positions[index * 3 + 1] = Math.sin(angle) * radius;
            positions[index * 3 + 2] = (Math.random() - 0.5) * 10;
          });
          positionsRef.current = positions;
        }

        setSwarmData(prev => ({
          ...prev,
          nodes: mappedAgents,
          positions: positionsRef.current
        }));
        
        logger.info(`Fetched ${mappedAgents.length} agents from MCP`);
      } catch (error) {
        logger.error('Error fetching MCP data:', error);
      }
    };

    const pollCommunications = async () => {
      if (!isConnected) return;
      
      try {
        const communications = await mcpWebSocketService.getCommunications();
        
        // Convert communications to edges
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
            edge.lastMessageTime = Date.now();
          });
        });

        setSwarmData(prev => ({
          ...prev,
          edges: Array.from(edgeMap.values())
        }));
      } catch (error) {
        logger.error('Error polling communications:', error);
      }
    };

    const handleMCPUpdate = (data: any) => {
      logger.debug('Received MCP update:', data);
      // Handle real-time updates from MCP
      if (data.agents) {
        fetchMCPData();
      }
    };

    connectToMCP();

    return () => {
      if (isConnected) {
        mcpWebSocketService.off('update', handleMCPUpdate);
        mcpWebSocketService.disconnect();
      }
      if (pollInterval) {
        clearInterval(pollInterval);
      }
    };
  }, []);

  // Update instanced mesh when data changes
  useEffect(() => {
    if (meshRef.current && swarmData.nodes.length > 0) {
      meshRef.current.count = swarmData.nodes.length;
      meshRef.current.instanceMatrix.needsUpdate = true;
      
      if (debugState.isEnabled()) {
        logger.debug(`Updated swarm mesh count to: ${swarmData.nodes.length}`);
      }
    }
  }, [swarmData.nodes.length]);

  // Main render loop - similar to GraphManager
  useFrame((state, delta) => {
    if (!meshRef.current || !labelsRef.current || swarmData.nodes.length === 0) {
      return;
    }

    const positions = positionsRef.current;
    if (!positions) return;

    // Update InstancedMesh for nodes
    const nodeSize = settings?.visualisation?.nodes?.nodeSize || 0.01;
    const BASE_SPHERE_RADIUS = 0.5;
    const baseScale = nodeSize / BASE_SPHERE_RADIUS;
    const tempMatrix = new THREE.Matrix4();

    swarmData.nodes.forEach((agent, i) => {
      const i3 = i * 3;
      
      // Scale based on agent workload/activity
      const scale = baseScale * (1 + agent.workload * 0.5);
      
      tempMatrix.makeScale(scale, scale, scale);
      tempMatrix.setPosition(positions[i3], positions[i3 + 1], positions[i3 + 2]);
      meshRef.current!.setMatrixAt(i, tempMatrix);
    });
    meshRef.current.instanceMatrix.needsUpdate = true;

    // Update edges
    const newEdgePoints: number[] = [];
    swarmData.edges.forEach(edge => {
      const sourceIndex = swarmData.nodes.findIndex(n => n.id === edge.source);
      const targetIndex = swarmData.nodes.findIndex(n => n.id === edge.target);
      
      if (sourceIndex !== -1 && targetIndex !== -1) {
        const i3s = sourceIndex * 3;
        const i3t = targetIndex * 3;
        newEdgePoints.push(positions[i3s], positions[i3s + 1], positions[i3s + 2]);
        newEdgePoints.push(positions[i3t], positions[i3t + 1], positions[i3t + 2]);
      }
    });
    setEdgePoints(newEdgePoints);

    // Update labels
    const labelSettings = settings?.visualisation?.labels;
    labelsRef.current.children.forEach((label, i) => {
      if (label instanceof THREE.Group && swarmData.nodes[i]) {
        const i3 = i * 3;
        label.position.set(
          positions[i3],
          positions[i3 + 1] + (labelSettings?.textPadding || 0.3),
          positions[i3 + 2]
        );
      }
    });
  });

  // Create geometry based on agent status
  const getAgentGeometry = (agent: SwarmAgent) => {
    const size = 0.5 + (agent.workload || 0) * 0.5;
    
    switch (agent.status) {
      case 'error':
      case 'terminating':
        return new THREE.TetrahedronGeometry(size);
      case 'initializing':
        return new THREE.BoxGeometry(size, size, size);
      default:
        return new THREE.SphereGeometry(size, 16, 16);
    }
  };

  // Node labels
  const NodeLabels = useMemo(() => {
    const labelSettings = settings?.visualisation?.labels;
    if (!labelSettings?.enableLabels) return null;

    return (
      <group ref={labelsRef}>
        {swarmData.nodes.map((agent, index) => {
          if (!agent.name) return null;
          
          const position = positionsRef.current ? [
            positionsRef.current[index * 3],
            positionsRef.current[index * 3 + 1] + 1,
            positionsRef.current[index * 3 + 2]
          ] : [0, 0, 0];

          return (
            <Html key={agent.id} position={position as [number, number, number]} center>
              <div style={{
                color: SWARM_COLORS[agent.type] || '#FFFFFF',
                fontSize: '12px',
                fontWeight: 'bold',
                textShadow: '0 0 4px rgba(0,0,0,0.8)',
                whiteSpace: 'nowrap',
                pointerEvents: 'none'
              }}>
                {agent.name}
                <div style={{ fontSize: '10px', opacity: 0.8 }}>
                  {Math.round(agent.cpuUsage)}% CPU | {Math.round(agent.health)}% Health
                </div>
              </div>
            </Html>
          );
        })}
      </group>
    );
  }, [swarmData.nodes, settings?.visualisation?.labels]);

  if (isLoading) {
    console.log('[SWARM] Still loading...');
    return (
      <group>
        <Html center>
          <div style={{ 
            color: '#F1C40F', 
            fontSize: '24px',
            background: 'rgba(0, 0, 0, 0.9)',
            padding: '20px',
            border: '3px solid #F1C40F',
            borderRadius: '10px'
          }}>
            🐝 Loading swarm visualization...
          </div>
        </Html>
      </group>
    );
  }

  if (error) {
    return (
      <group>
        <Html center>
          <div style={{
            background: 'rgba(0, 0, 0, 0.9)',
            padding: '20px',
            borderRadius: '10px',
            color: '#E74C3C',
            fontSize: '18px',
            border: '3px solid #E74C3C'
          }}>
            ⚠️ Swarm Error: {error}
          </div>
        </Html>
      </group>
    );
  }

  console.log('[SWARM] Rendering with', swarmData.nodes.length, 'nodes and', swarmData.edges.length, 'edges');
  
  // Add a visible test sphere to ensure the component is rendering
  if (swarmData.nodes.length === 0) {
    console.log('[SWARM] No nodes to render, showing placeholder');
    return (
      <group>
        <mesh position={[0, 10, 0]}>
          <sphereGeometry args={[2, 32, 32]} />
          <meshStandardMaterial color="#F1C40F" emissive="#F1C40F" emissiveIntensity={0.5} />
        </mesh>
        <Html position={[0, 15, 0]} center>
          <div style={{
            background: 'rgba(0, 0, 0, 0.9)',
            padding: '15px',
            borderRadius: '8px',
            color: '#F1C40F',
            fontSize: '16px',
            border: '3px solid #F1C40F'
          }}>
            ⚠️ Swarm: No agents loaded yet
          </div>
        </Html>
      </group>
    );
  }
  
  return (
    <group>
      {/* Debug info */}
      <SwarmDebugInfo
        isLoading={isLoading}
        error={error}
        nodeCount={swarmData.nodes.length}
        edgeCount={swarmData.edges.length}
        mcpConnected={mcpConnected}
        dataSource={dataSource}
      />
      
      {/* Swarm status indicator */}
      <Html position={[50, 20, 0]} center>
        <div style={{
          background: 'rgba(0, 0, 0, 0.8)',
          padding: '10px',
          borderRadius: '5px',
          color: '#fff',
          fontFamily: 'monospace',
          fontSize: '12px',
          minWidth: '200px',
          border: '2px solid #F1C40F'
        }}>
          <h3 style={{ margin: '0 0 10px 0', color: '#F1C40F' }}>🐝 Agent Swarm</h3>
          <div>Agents: {swarmData.nodes.length}</div>
          <div>Communications: {swarmData.edges.length}</div>
          <div>Status: Active</div>
        </div>
      </Html>

      {/* Swarm nodes */}
      <instancedMesh
        ref={meshRef}
        args={[undefined, undefined, swarmData.nodes.length]}
        frustumCulled={false}
      >
        <sphereGeometry args={[0.5, 16, 16]} />
        <meshStandardMaterial
          color="#F1C40F"
          emissive="#F1C40F"
          emissiveIntensity={0.5}
          metalness={0.8}
          roughness={0.2}
          transparent={true}
          opacity={0.9}
        />
      </instancedMesh>

      {/* Communication edges */}
      {edgePoints.length > 0 && (
        <Line
          points={edgePoints}
          color="#2ECC71"
          lineWidth={2}
          transparent
          opacity={0.6}
          dashed={false}
        />
      )}

      {/* Agent labels */}
      {NodeLabels}

      {/* Ambient particles for swarm atmosphere */}
      <points>
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={200}
            array={new Float32Array(600).map(() => (Math.random() - 0.5) * 100)}
            itemSize={3}
          />
        </bufferGeometry>
        <pointsMaterial
          size={0.1}
          color="#F1C40F"
          transparent
          opacity={0.3}
          blending={THREE.AdditiveBlending}
        />
      </points>
    </group>
  );
};