import React, { useRef, useEffect, useState, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { Html, Text, Billboard, Line as DreiLine } from '@react-three/drei';
import { BotsAgent, BotsEdge, BotsState } from '../types/botsTypes';
import { createLogger } from '../../../utils/logger';
import { useSettingsStore } from '../../../store/settingsStore';
import { apiService } from '../../../services/apiService';
import { mcpWebSocketService } from '../services/MCPWebSocketService';
import { botsWebSocketIntegration } from '../services/BotsWebSocketIntegration';
import { useBotsBinaryUpdates } from '../hooks/useBotsBinaryUpdates';
import { botsPhysicsWorker } from '../workers/BotsPhysicsWorker';
// BotsStatusIndicator merged into main VisionFlow panel
import { BotsDebugInfo } from './BotsVisualizationDebugInfo';
import { debugState } from '../../../utils/debugState';
import { useBotsData } from '../contexts/BotsDataContext';

const logger = createLogger('BotsVisualization');

// Get VisionFlow colors from settings or use defaults
const getVisionFlowColors = (settings: any) => {
  const visionflowSettings = settings?.visualisation?.graphs?.visionflow;
  const baseColor = visionflowSettings?.nodes?.baseColor || '#F1C40F';

  // Default gold and green color palette for bots
  return {
    // Primary agent types - Greens for roles (can be overridden by settings)
    coder: '#2ECC71',       // Emerald green
    tester: '#27AE60',      // Nephritis green
    researcher: '#1ABC9C',  // Turquoise
    reviewer: '#16A085',    // Green sea
    documenter: '#229954',  // Forest green
    specialist: '#239B56',  // Emerald dark

    // Meta roles - Golds for coordination (use base color from settings)
    coordinator: baseColor, // From settings
    analyst: '#F39C12',     // Orange gold
    architect: '#E67E22',   // Carrot gold
    optimizer: '#D68910',   // Dark gold
    monitor: '#D4AC0D',     // Bright gold
  };
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

interface BotsNodeProps {
  agent: BotsAgent;
  position: THREE.Vector3;
  index: number;
}

const BotsNode: React.FC<BotsNodeProps> = ({ agent, position, index }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);
  const borderRef = useRef<THREE.Mesh>(null);
  const [hover, setHover] = useState(false);
  const settings = useSettingsStore(state => state.settings);

  // Visual properties based on agent data and settings
  const visionflowColors = getVisionFlowColors(settings);
  const color = new THREE.Color(visionflowColors[agent.type] || '#CCCCCC');
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
          transparent
          opacity={0.5}
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
          opacity={0.1}
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

interface BotsEdgeProps {
  edge: BotsEdge;
  sourcePos: THREE.Vector3;
  targetPos: THREE.Vector3;
}

const BotsEdgeComponent: React.FC<BotsEdgeProps> = ({ edge, sourcePos, targetPos }) => {
  const particlesRef = useRef<THREE.Points>(null);
  const lineRef = useRef<THREE.Mesh>(null);
  const [isActive, setIsActive] = useState(false);
  const settings = useSettingsStore(state => state.settings);

  // Check if edge has recent activity
  useEffect(() => {
    const checkActivity = () => {
      const timeSinceLastMessage = Date.now() - edge.lastMessageTime;
      setIsActive(timeSinceLastMessage < 5000); // Active if communicated within 5 seconds
    };

    checkActivity();
    const interval = setInterval(checkActivity, 1000);
    return () => clearInterval(interval);
  }, [edge.lastMessageTime]);

  // Animate particles along edge only when active
  useFrame((state) => {
    if (!particlesRef.current || !isActive) return;

    const particlePositions = particlesRef.current.geometry.attributes.position;
    const animationSpeed = Math.min(edge.messageCount / 5, 3);
    const time = state.clock.elapsedTime * animationSpeed;

    // Animate 8 particles along the edge for pulse effect
    for (let i = 0; i < 8; i++) {
      const t = (time + i * 0.125) % 1;
      const x = sourcePos.x + (targetPos.x - sourcePos.x) * t;
      const y = sourcePos.y + (targetPos.y - sourcePos.y) * t;
      const z = sourcePos.z + (targetPos.z - sourcePos.z) * t;
      particlePositions.setXYZ(i, x, y, z);
    }
    particlePositions.needsUpdate = true;
  });

  // Create a cylindrical edge
  const edgeGeometry = useMemo(() => {
    const direction = new THREE.Vector3().subVectors(targetPos, sourcePos);
    const distance = direction.length();
    const geometry = new THREE.CylinderGeometry(0.05, 0.05, distance, 8);
    return geometry;
  }, [sourcePos, targetPos]);

  const edgePosition = useMemo(() => {
    return new THREE.Vector3(
      (sourcePos.x + targetPos.x) / 2,
      (sourcePos.y + targetPos.y) / 2,
      (sourcePos.z + targetPos.z) / 2
    );
  }, [sourcePos, targetPos]);

  const edgeRotation = useMemo(() => {
    const direction = new THREE.Vector3().subVectors(targetPos, sourcePos).normalize();
    const axis = new THREE.Vector3(0, 1, 0);
    const quaternion = new THREE.Quaternion().setFromUnitVectors(axis, direction);
    return quaternion;
  }, [sourcePos, targetPos]);

  return (
    <group>
      {/* Persistent edge line */}
      <mesh
        ref={lineRef}
        geometry={edgeGeometry}
        position={edgePosition}
        quaternion={edgeRotation}
      >
        <meshBasicMaterial
          color={settings?.visualisation?.graphs?.visionflow?.edges?.color || "#F1C40F"}
          transparent
          opacity={0.2}
        />
      </mesh>

      {/* Animated particles showing active data flow */}
      {isActive && (
        <points ref={particlesRef}>
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              count={8}
              array={new Float32Array(24)}
              itemSize={3}
            />
          </bufferGeometry>
          <pointsMaterial
            size={0.4}
            color="#FFD700"
            transparent
            opacity={0.95}
            blending={THREE.NormalBlending}
            sizeAttenuation={true}
          />
        </points>
      )}
    </group>
  );
};

interface BotsGraphData {
  nodes: BotsAgent[];
  edges: BotsEdge[];
  tokenUsage?: { total: number; byAgent: { [key: string]: number } };
  positions?: Float32Array;
}

export const BotsVisualization: React.FC = () => {
  // console.log('[VISIONFLOW] VisionFlow Enhanced component mounting...');

  const [botsData, setBotsData] = useState<BotsGraphData>({ nodes: [], edges: [] });
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [dataSource, setDataSource] = useState<'mcp' | 'api' | 'mock'>('mock'); // Default to mock until MCP orchestrator is available
  const [mcpConnected, setMcpConnected] = useState(false);
  const positionsRef = useRef<Map<string, THREE.Vector3>>(new Map());

  const edgeMapRef = useRef<Map<string, BotsEdge>>(new Map());
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const labelsRef = useRef<THREE.Group>(null);
  const settings = useSettingsStore(state => state.settings);
  const { updateBotsData } = useBotsData();

  // Position buffer from backend
  const positionBufferRef = useRef<Float32Array | null>(null);

  // Binary position updates from WebSocket
  const { positions: binaryPositions } = useBotsBinaryUpdates({
    enabled: !isLoading && botsData.nodes.length > 0,
    onPositionUpdate: (positions) => {
      // Update the positions ref when binary updates arrive
      positionBufferRef.current = positions;

      // Convert to Vector3 map
      if (positions && botsData.nodes.length > 0) {
        botsData.nodes.forEach((node, i) => {
          const i3 = i * 3;
          if (!positionsRef.current.has(node.id)) {
            positionsRef.current.set(node.id, new THREE.Vector3());
          }
          positionsRef.current.get(node.id)!.set(
            positions[i3],
            positions[i3 + 1],
            positions[i3 + 2]
          );
        });
      }
    }
  });


  // Initialize physics and data connection
  useEffect(() => {
    // console.log('[VISIONFLOW] useEffect - Starting initialization...');
    let cleanup = false;

    const initialize = async () => {
      try {
        // console.log('[VISIONFLOW] Initializing VisionFlow visualization...');
        setIsLoading(true);

        // Initialize physics worker
        botsPhysicsWorker.init();

        // Use integrated WebSocket service for both connections
        const connectionStatus = botsWebSocketIntegration.getConnectionStatus();
        logger.info('WebSocket connection status:', connectionStatus);

        // Set up real-time updates through integrated service
        botsWebSocketIntegration.on('bots-agents-update', (agents) => {
          logger.debug('Received bots agents update', agents);
          processAgentsUpdate(agents);
        });

        botsWebSocketIntegration.on('bots-edges-update', (edges) => {
          logger.debug('Received bots edges update', edges);
          processEdgesUpdate(edges);
        });

        botsWebSocketIntegration.on('bots-token-usage', (tokenUsage) => {
          logger.debug('Received token usage update', tokenUsage);
          setBotsData(prev => ({ ...prev, tokenUsage }));
        });

        botsWebSocketIntegration.on('mcp-connected', ({ connected }) => {
          setMcpConnected(connected);
          if (connected) {
            setDataSource('mcp');
            logger.info('MCP connection established through integration service');
          }
        });

        // Request initial data through integration service
        try {
          await botsWebSocketIntegration.requestInitialData();

          // If MCP is connected, also fetch directly
          if (connectionStatus.mcp) {
            await fetchMCPData();
          }
        } catch (error) {
          logger.warn('Failed to get initial data through integration:', error);

          // Try API fallback
          try {
            logger.info('[VISIONFLOW] Fetching bots data from API...');
            const data = await apiService.getBotsData();
            logger.info('[VISIONFLOW] API response:', data);

            if (data && data.nodes && data.nodes.length > 0) {
              logger.info('[VISIONFLOW] Got real data from API with', data.nodes.length, 'nodes');
              setDataSource('api');
              processBotsData(data);
            } else if (data && data._isMock) {
              logger.warn('[VISIONFLOW] API returned mock data');
              setDataSource('mock');
              processBotsData(data);
            } else {
              logger.warn('[VISIONFLOW] API returned empty or invalid data:', data);
              setError('No bots data available from server');
            }
          } catch (apiError) {
            logger.error('API also failed:', apiError);
            setError('Unable to connect to VisionFlow data source');
          }
        }

        setError(null);
      } catch (err) {
        logger.error('Initialization failed:', err);
        setError('Failed to initialize VisionFlow visualization');
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

        // Update edge map to maintain persistent edges between agent pairs
        communications.forEach(comm => {
          comm.receivers?.forEach(receiver => {
            const edgeKey = [comm.sender, receiver].sort().join('-');
            if (!edgeMapRef.current.has(edgeKey)) {
              edgeMapRef.current.set(edgeKey, {
                id: edgeKey,
                source: comm.sender,
                target: receiver,
                dataVolume: 0,
                messageCount: 0,
                lastMessageTime: Date.now()
              });
            }
            const edge = edgeMapRef.current.get(edgeKey)!;
            edge.dataVolume += comm.metadata?.size || 100;
            edge.messageCount += 1;
            edge.lastMessageTime = Date.now();
          });
        });

        // Clean up stale edges (no communication for 30 seconds)
        const now = Date.now();
        for (const [key, edge] of edgeMapRef.current.entries()) {
          if (now - edge.lastMessageTime > 30000) {
            edgeMapRef.current.delete(key);
          }
        }

        setBotsData({
          nodes: agents,
          edges: Array.from(edgeMapRef.current.values()),
          tokenUsage
        });

        // Update physics
        botsPhysicsWorker.updateAgents(agents);
        botsPhysicsWorker.updateEdges(Array.from(edgeMapRef.current.values()));
        if (tokenUsage) botsPhysicsWorker.updateTokenUsage(tokenUsage);
      } catch (error) {
        logger.error('Error fetching MCP data:', error);
      }
    };

    const handleMCPUpdate = (data: any) => {
      if (data.agents) fetchMCPData();
    };

    const processAgentsUpdate = (agents: BotsAgent[]) => {
      // Update bots data with new agents
      setBotsData(prev => ({ ...prev, nodes: agents }));

      // Update physics
      botsPhysicsWorker.updateAgents(agents);
    };

    const processEdgesUpdate = (edges: BotsEdge[]) => {
      // Update edge map
      edges.forEach(edge => {
        const edgeKey = [edge.source, edge.target].sort().join('-');
        edgeMapRef.current.set(edgeKey, edge);
      });

      // Clean up stale edges
      const now = Date.now();
      for (const [key, edge] of edgeMapRef.current.entries()) {
        if (now - edge.lastMessageTime > 30000) {
          edgeMapRef.current.delete(key);
        }
      }

      // Update bots data
      setBotsData(prev => ({
        ...prev,
        edges: Array.from(edgeMapRef.current.values())
      }));

      // Update physics
      botsPhysicsWorker.updateEdges(Array.from(edgeMapRef.current.values()));
    };

    const processBotsData = (data: any) => {
      // Convert backend format to BotsAgent format if needed
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

      // Update persistent edge map with backend edges
      data.edges?.forEach((edge: any) => {
        const edgeKey = [edge.source.toString(), edge.target.toString()].sort().join('-');
        edgeMapRef.current.set(edgeKey, {
          id: edge.id,
          source: edge.source.toString(),
          target: edge.target.toString(),
          dataVolume: edge.weight * 1024,
          messageCount: parseInt(edge.edge_type?.match(/\d+/) || '1'),
          lastMessageTime: Date.now()
        });
      });

      setBotsData({ nodes, edges: Array.from(edgeMapRef.current.values()) });

      // Update physics
      botsPhysicsWorker.updateAgents(nodes);
      botsPhysicsWorker.updateEdges(Array.from(edgeMapRef.current.values()));
    };

    // Remove mock data generation - only use real data

    if (!cleanup) {
      initialize();
    }

    // Poll for updates if using MCP
    const pollInterval = dataSource === 'mcp' ? setInterval(fetchMCPData, 3000) : null;

    return () => {
      cleanup = true;
      if (pollInterval) clearInterval(pollInterval);

      // Clean up integrated WebSocket listeners
      // Note: We don't disconnect the integration service as it's shared
      botsPhysicsWorker.cleanup();
    };
  }, []);

  // Update positions from physics simulation
  // Update context when bots data changes
  useEffect(() => {
    updateBotsData({
      nodeCount: botsData.nodes.length,
      edgeCount: botsData.edges.length,
      tokenCount: botsData.tokenUsage?.total || 0,
      mcpConnected,
      dataSource
    });
  }, [botsData, mcpConnected, dataSource, updateBotsData]);

  // Update physics configuration when settings change
  useEffect(() => {
    const physicsSettings = settings?.visualisation?.physics;
    if (physicsSettings) {
      // Extract physics configuration from settings
      const physicsConfig = {
        springStrength: physicsSettings.springStrength ?? 0.3,
        linkDistance: physicsSettings.linkDistance ?? 20,
        damping: physicsSettings.damping ?? 0.95,
        nodeRepulsion: physicsSettings.nodeRepulsion ?? 15,
        gravityStrength: physicsSettings.gravityStrength ?? 0.1,
        maxVelocity: physicsSettings.maxVelocity ?? 0.5
      };

      // Update the physics worker with new configuration
      botsPhysicsWorker.updateConfig(physicsConfig);
      logger.debug('Updated physics configuration:', physicsConfig);
    }
  }, [settings?.visualisation?.physics]);

  useFrame((state, delta) => {
    // Try to get positions from physics worker first
    const workerPositions = botsPhysicsWorker.getPositions();
    if (workerPositions && workerPositions.size > 0) {
      workerPositions.forEach((pos, id) => {
        if (!positionsRef.current.has(id)) {
          positionsRef.current.set(id, new THREE.Vector3());
        }
        positionsRef.current.get(id)!.set(pos.x, pos.y, pos.z);
      });
    } else if (botsData.nodes.length > 0 && settings?.visualisation?.physics?.enabled !== false) {
      // Fallback to simple physics simulation if worker not available
      const dt = Math.min(delta, 0.016);
      const physics = {
        damping: 0.95,
        nodeRepulsion: 15,
        linkDistance: 20,
        centerGravity: 0.01
      };

      // Simple physics for each node
      positionsRef.current.forEach((pos, id) => {
        const velocity = new THREE.Vector3();

        // Repulsion from other nodes
        positionsRef.current.forEach((otherPos, otherId) => {
          if (id !== otherId) {
            const diff = new THREE.Vector3().subVectors(pos, otherPos);
            const dist = diff.length();

            if (dist > 0 && dist < physics.nodeRepulsion) {
              const force = physics.nodeRepulsion / (dist * dist);
              diff.normalize().multiplyScalar(force * dt);
              velocity.add(diff);
            }
          }
        });

        // Attraction for edges
        botsData.edges.forEach(edge => {
          if (edge.source === id || edge.target === id) {
            const otherId = edge.source === id ? edge.target : edge.source;
            const otherPos = positionsRef.current.get(otherId);
            if (otherPos) {
              const diff = new THREE.Vector3().subVectors(otherPos, pos);
              const dist = diff.length();
              if (dist > 0) {
                const force = 0.1 * (dist - physics.linkDistance);
                diff.normalize().multiplyScalar(force * dt);
                velocity.add(diff);
              }
            }
          }
        });

        // Center gravity
        velocity.add(pos.clone().multiplyScalar(-physics.centerGravity * dt));

        // Apply damping and update position
        velocity.multiplyScalar(physics.damping);
        pos.add(velocity);

        // Limit position
        const maxDist = 30;
        if (pos.length() > maxDist) {
          pos.normalize().multiplyScalar(maxDist);
        }
      });
    }

    // Update instanced mesh if available
    if (meshRef.current && botsData.nodes.length > 0) {
      meshRef.current.count = botsData.nodes.length;
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  });

  if (isLoading) {
    // console.log('[VISIONFLOW] Rendering loading state...');
    return (
      <group position={[0, 0, 0]}>
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
            ⚡ Loading VisionFlow...
          </div>
        </Html>
      </group>
    );
  }

  if (error) {
    return (
      <group position={[0, 0, 0]}>
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

  // Position bots graph at origin to co-locate with knowledge graph
  return (
    <group position={[0, 0, 0]}>
      {/* Merged status indicator directly in VisionFlow */}

      {/* Debug info component */}
      {debugState.isEnabled() && (
        <BotsDebugInfo
          isLoading={isLoading}
          error={error}
          nodeCount={botsData.nodes.length}
          edgeCount={botsData.edges.length}
          mcpConnected={mcpConnected}
          dataSource={dataSource}
        />
      )}


      {/* Render nodes - use instanced mesh for large botss */}
      {botsData.nodes.length > 50 ? (
        <instancedMesh
          ref={meshRef}
          args={[undefined, undefined, botsData.nodes.length]}
          frustumCulled={false}
        >
          <sphereGeometry args={[0.5, 16, 16]} />
          <meshStandardMaterial
            color={settings?.visualisation?.graphs?.visionflow?.nodes?.baseColor || "#F1C40F"}
            emissive={settings?.visualisation?.graphs?.visionflow?.nodes?.baseColor || "#F1C40F"}
            emissiveIntensity={0.3}
            metalness={0.8}
            roughness={0.2}
            transparent
            opacity={0.9}
          />
        </instancedMesh>
      ) : (
        botsData.nodes.map((agent, index) => {
          const position = positionsRef.current.get(agent.id) || new THREE.Vector3(
            Math.cos(index / botsData.nodes.length * Math.PI * 2) * 20,
            Math.sin(index / botsData.nodes.length * Math.PI * 2) * 20,
            (Math.random() - 0.5) * 10
          );

          return (
            <BotsNode
              key={agent.id}
              agent={agent}
              position={position}
              index={index}
            />
          );
        })
      )}

      {/* Render edges */}
      {botsData.edges.map(edge => {
        const sourcePos = positionsRef.current.get(edge.source);
        const targetPos = positionsRef.current.get(edge.target);

        if (sourcePos && targetPos) {
          return (
            <BotsEdgeComponent
              key={edge.id}
              edge={edge}
              sourcePos={sourcePos}
              targetPos={targetPos}
            />
          );
        }
        return null;
      })}

      {/* Ambient particles for VisionFlow atmosphere - Fixed white squares bug */}
      <points>
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={200}
            array={new Float32Array(600).map(() => (Math.random() - 0.5) * 60)}
            itemSize={3}
          />
        </bufferGeometry>
        <pointsMaterial
          size={0.05}
          color={settings?.visualisation?.graphs?.visionflow?.nodes?.baseColor || "#F1C40F"}
          transparent
          opacity={0.3}
          blending={THREE.NormalBlending}
          sizeAttenuation={true}
          depthWrite={false} // Fix for white squares bug - prevents depth conflicts
        />
      </points>
    </group>
  );
};