import React, { useRef, useEffect, useState, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { Html } from '@react-three/drei';
import { SwarmAgent, SwarmEdge, SwarmState, SwarmVisualConfig } from '../types/swarmTypes';
import { MockSwarmDataProvider } from '../services/MockSwarmDataProvider';
import { createLogger } from '../../../utils/logger';

const logger = createLogger('SwarmVisualizationSimple');

// Use mock provider
const swarmDataProvider = new MockSwarmDataProvider();

// Gold and green color palette
const VISUAL_CONFIG: SwarmVisualConfig = {
  colors: {
    coder: '#2ECC71',
    tester: '#27AE60',
    coordinator: '#F1C40F',
    analyst: '#F39C12',
    researcher: '#1ABC9C',
    architect: '#E67E22',
    reviewer: '#16A085',
    optimizer: '#D68910',
    documenter: '#229954',
    monitor: '#D4AC0D',
    specialist: '#239B56'
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

interface NodeVisualsProps {
  agent: SwarmAgent;
  position: THREE.Vector3;
}

const NodeVisuals: React.FC<NodeVisualsProps> = ({ agent, position }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);

  const color = new THREE.Color(VISUAL_CONFIG.colors[agent.type] || '#CCCCCC');
  const size = 0.5 + (agent.workload || agent.cpuUsage / 100) * 1.5;
  const healthColor = new THREE.Color().lerpColors(
    new THREE.Color('#E74C3C'),
    new THREE.Color('#2ECC71'),
    agent.health / 100
  );

  useFrame((state) => {
    if (meshRef.current && glowRef.current) {
      const pulseSpeed = 1 + agent.cpuUsage / 20;
      const pulseScale = 1 + Math.sin(state.clock.elapsedTime * pulseSpeed) * 0.1;
      glowRef.current.scale.setScalar(pulseScale);

      meshRef.current.position.copy(position);
      glowRef.current.position.copy(position);
    }
  });

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

  return (
    <group>
      <mesh ref={meshRef} geometry={geometry}>
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={0.3}
          metalness={0.7}
          roughness={0.3}
        />
      </mesh>

      <mesh position={position.toArray()}>
        <ringGeometry args={[size * 1.2, size * 1.4, 32]} />
        <meshBasicMaterial color={healthColor} transparent opacity={0.8} />
      </mesh>

      <mesh ref={glowRef} position={position.toArray()}>
        <sphereGeometry args={[size * 1.5, 16, 16]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0.2 + agent.cpuUsage / 200}
          side={THREE.BackSide}
        />
      </mesh>
    </group>
  );
};

export const SwarmVisualizationSimple: React.FC = () => {
  const [swarmState, setSwarmState] = useState<SwarmState>({
    agents: new Map(),
    edges: new Map(),
    communications: [],
    tokenUsage: { total: 0, byAgent: {} },
    lastUpdate: Date.now()
  });
  const [connected, setConnected] = useState(false);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [positions, setPositions] = useState<Map<string, THREE.Vector3>>(new Map());
  const [velocities, setVelocities] = useState<Map<string, THREE.Vector3>>(new Map());

  useEffect(() => {
    // Connect to mock provider
    swarmDataProvider.connect().then(() => {
      logger.info('Connected to swarm data provider');
      setConnected(true);
      setConnectionError(null);

      swarmDataProvider.on('update', handleUpdate);
      fetchSwarmData();
    }).catch(error => {
      logger.error('Failed to connect:', error);
      setConnectionError('Swarm visualization loading...');
      setConnected(false);
    });

    const pollInterval = setInterval(pollCommunications, 3000);

    return () => {
      swarmDataProvider.disconnect();
      clearInterval(pollInterval);
    };
  }, []);

  const handleUpdate = async (data: any) => {
    if (data.agents) {
      const agents = new Map<string, SwarmAgent>();
      data.agents.forEach((agent: SwarmAgent) => {
        agents.set(agent.id, agent);

        // Initialize positions if needed
        if (!positions.has(agent.id)) {
          positions.set(agent.id, new THREE.Vector3(
            (Math.random() - 0.5) * 30,
            (Math.random() - 0.5) * 30,
            (Math.random() - 0.5) * 30
          ));
          velocities.set(agent.id, new THREE.Vector3(0, 0, 0));
        }
      });

      setSwarmState(prev => ({
        ...prev,
        agents,
        lastUpdate: Date.now()
      }));
    }
  };

  const fetchSwarmData = async () => {
    try {
      const [agents, tokenUsage] = await Promise.all([
        swarmDataProvider.getAgents(),
        swarmDataProvider.getTokenUsage()
      ]);

      const agentMap = new Map<string, SwarmAgent>();
      agents.forEach(agent => {
        agentMap.set(agent.id, agent);

        if (!positions.has(agent.id)) {
          positions.set(agent.id, new THREE.Vector3(
            (Math.random() - 0.5) * 30,
            (Math.random() - 0.5) * 30,
            (Math.random() - 0.5) * 30
          ));
          velocities.set(agent.id, new THREE.Vector3(0, 0, 0));
        }
      });

      setSwarmState(prev => ({
        ...prev,
        agents: agentMap,
        tokenUsage,
        lastUpdate: Date.now()
      }));
    } catch (error) {
      logger.error('Error fetching swarm data:', error);
    }
  };

  const pollCommunications = async () => {
    try {
      const communications = await swarmDataProvider.getCommunications();
      const edgeMap = new Map<string, SwarmEdge>();

      communications.forEach(comm => {
        const edgeKey = [comm.sender, ...comm.receivers].sort().join('-');

        if (!edgeMap.has(edgeKey)) {
          edgeMap.set(edgeKey, {
            id: edgeKey,
            source: comm.sender,
            target: comm.receivers[0],
            dataVolume: 0,
            messageCount: 0,
            lastMessageTime: Date.now()
          });
        }

        const edge = edgeMap.get(edgeKey)!;
        edge.dataVolume += comm.metadata.size;
        edge.messageCount += 1;
        edge.lastMessageTime = Date.now();
      });

      setSwarmState(prev => ({
        ...prev,
        edges: edgeMap,
        communications: communications.slice(-100)
      }));
    } catch (error) {
      logger.error('Error polling communications:', error);
    }
  };

  // Simple physics simulation
  useFrame((state, delta) => {
    const dt = Math.min(delta, 0.016);

    // Update positions with simple physics
    positions.forEach((pos, id) => {
      const vel = velocities.get(id)!;
      const agent = swarmState.agents.get(id);

      if (!agent) return;

      // Apply damping
      vel.multiplyScalar(0.95);

      // Repulsion from other nodes
      positions.forEach((otherPos, otherId) => {
        if (id !== otherId) {
          const diff = new THREE.Vector3().subVectors(pos, otherPos);
          const dist = diff.length();

          if (dist > 0 && dist < 10) {
            const force = 3 / (dist * dist);
            diff.normalize().multiplyScalar(force * dt);
            vel.add(diff);
          }
        }
      });

      // Attraction for edges
      swarmState.edges.forEach(edge => {
        if (edge.source === id || edge.target === id) {
          const otherId = edge.source === id ? edge.target : edge.source;
          const otherPos = positions.get(otherId);
          if (otherPos) {
            const diff = new THREE.Vector3().subVectors(otherPos, pos);
            const dist = diff.length();
            if (dist > 0) {
              const idealDist = 15;
              const force = 0.1 * (dist - idealDist);
              diff.normalize().multiplyScalar(force * dt);
              vel.add(diff);
            }
          }
        }
      });

      // Center gravity
      const centerForce = -0.01;
      vel.add(pos.clone().multiplyScalar(centerForce * dt));

      // Update position
      pos.add(vel);

      // Limit position
      const maxDist = 30;
      if (pos.length() > maxDist) {
        pos.normalize().multiplyScalar(maxDist);
      }
    });

    // Force re-render
    setPositions(new Map(positions));
  });

  return (
    <group position={[0, 0, 0]}>
      {/* Status display */}
      <Html position={[0, 20, 0]} center>
        <div style={{
          background: 'rgba(0, 0, 0, 0.8)',
          padding: '10px',
          borderRadius: '5px',
          color: '#fff',
          fontFamily: 'monospace',
          fontSize: '12px',
          minWidth: '200px',
          border: `2px solid ${connected ? '#2ECC71' : '#F1C40F'}`
        }}>
          <h3 style={{ margin: '0 0 10px 0', color: '#F1C40F' }}>🐝 Swarm Visualization</h3>
          <div>Agents: {swarmState.agents.size}</div>
          <div>Communications: {swarmState.edges.size}</div>
          <div>Status: {connected ? 'Active' : 'Loading...'}</div>
        </div>
      </Html>

      {/* Render nodes */}
      {Array.from(swarmState.agents.values()).map(agent => {
        const position = positions.get(agent.id) || new THREE.Vector3();
        return <NodeVisuals key={agent.id} agent={agent} position={position} />;
      })}

      {/* Render edges as simple lines */}
      {Array.from(swarmState.edges.values()).map(edge => {
        const sourcePos = positions.get(edge.source);
        const targetPos = positions.get(edge.target);

        if (sourcePos && targetPos) {
          const points = [sourcePos, targetPos];
          const geometry = new THREE.BufferGeometry().setFromPoints(points);

          return (
            <line key={edge.id} geometry={geometry}>
              <lineBasicMaterial
                color="#F1C40F"
                transparent
                opacity={0.3 + Math.min(edge.messageCount / 10, 0.7)}
                linewidth={1}
              />
            </line>
          );
        }
        return null;
      })}

      {/* Ambient particles */}
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