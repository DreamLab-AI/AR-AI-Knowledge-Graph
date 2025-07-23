import React, { useRef, useEffect, useState, useMemo, useCallback } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { Html, Text, Billboard, Line as DreiLine } from '@react-three/drei';
import { BotsAgent, BotsEdge, BotsState } from '../types/BotsTypes';
import { createLogger } from '../../../utils/logger';
import { useSettingsStore } from '../../../store/settingsStore';
import { botsPhysicsWorker } from '../workers/BotsPhysicsWorker';
import { BotsDebugInfo } from './BotsVisualizationDebugInfo';
import { BotsControlPanel } from './BotsControlPanel';
import { debugState } from '../../../utils/debugState';
import { useBotsData } from '../contexts/BotsDataContext';
import { mockDataGenerator } from '../services/MockDataGenerator';
import { configurationMapper, VisualizationConfig } from '../services/ConfigurationMapper';

const logger = createLogger('BotsVisualizationEnhanced');

// Enhanced Node Component with dynamic configuration
interface BotsNodeProps {
  agent: BotsAgent;
  position: THREE.Vector3;
  index: number;
  config: VisualizationConfig;
}

const BotsNodeEnhanced: React.FC<BotsNodeProps> = ({ agent, position, index, config }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);
  const borderRef = useRef<THREE.Mesh>(null);
  const [hover, setHover] = useState(false);

  // Visual properties from configuration
  const color = new THREE.Color(config.colors.agents[agent.type] || '#CCCCCC');
  const size = config.sizes.nodeBaseSize + (agent.workload || agent.cpuUsage / 100) *
    (config.sizes.nodeMaxSize - config.sizes.nodeBaseSize);

  // Health color from configuration
  const healthColor = useMemo(() => {
    if (agent.health > 80) return new THREE.Color(config.colors.health.excellent);
    if (agent.health > 50) return new THREE.Color(config.colors.health.good);
    if (agent.health > 30) return new THREE.Color(config.colors.health.warning);
    return new THREE.Color(config.colors.health.critical);
  }, [agent.health, config.colors.health]);

  // Pulse animation with configurable speed and amplitude
  useFrame((state) => {
    if (!meshRef.current || !glowRef.current) return;

    const pulseSpeed = config.animation.pulseSpeed + agent.cpuUsage / 20;
    const pulseScale = 1 + Math.sin(state.clock.elapsedTime * pulseSpeed) * config.animation.pulseAmplitude;

    // Pulse glow
    if (glowRef.current) {
      glowRef.current.scale.setScalar(pulseScale * config.sizes.glowScale);
      (glowRef.current.material as THREE.MeshBasicMaterial).opacity =
        config.rendering.glowOpacity + agent.cpuUsage / 200 + (hover ? 0.2 : 0);
    }

    // Update position
    meshRef.current.position.copy(position);
    if (glowRef.current) glowRef.current.position.copy(position);
    if (borderRef.current) borderRef.current.position.copy(position);
  });

  // Shape based on status
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
          emissiveIntensity={config.rendering.emissiveIntensity}
          metalness={config.rendering.metalness}
          roughness={config.rendering.roughness}
          transparent
          opacity={config.rendering.nodeOpacity}
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
        <sphereGeometry args={[size * config.sizes.glowScale, 16, 16]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={config.rendering.glowOpacity}
          side={THREE.BackSide}
        />
      </mesh>

      {/* Label with details */}
      <Billboard position={[position.x, position.y + size * 2, position.z]}>
        <Text
          fontSize={hover ? config.sizes.labelFontSizeHover : config.sizes.labelFontSize}
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

// Enhanced Edge Component with dynamic configuration
interface BotsEdgeProps {
  edge: BotsEdge;
  sourcePos: THREE.Vector3;
  targetPos: THREE.Vector3;
  config: VisualizationConfig;
}

const BotsEdgeEnhanced: React.FC<BotsEdgeProps> = ({ edge, sourcePos, targetPos, config }) => {
  const particlesRef = useRef<THREE.Points>(null);
  const lineRef = useRef<THREE.Mesh>(null);
  const [isActive, setIsActive] = useState(false);

  // Check if edge has recent activity based on configuration
  useEffect(() => {
    const checkActivity = () => {
      const timeSinceLastMessage = Date.now() - edge.lastMessageTime;
      setIsActive(timeSinceLastMessage < config.animation.edgeActivityThreshold);
    };

    checkActivity();
    const interval = setInterval(checkActivity, 1000);
    return () => clearInterval(interval);
  }, [edge.lastMessageTime, config.animation.edgeActivityThreshold]);

  // Animate particles with configurable speed and count
  useFrame((state) => {
    if (!particlesRef.current || !isActive) return;

    const particlePositions = particlesRef.current.geometry.attributes.position;
    const animationSpeed = config.animation.particleSpeed * Math.min(edge.messageCount / 5, 3);
    const time = state.clock.elapsedTime * animationSpeed;

    // Animate particles along the edge
    for (let i = 0; i < config.animation.particleCount; i++) {
      const t = (time + i * (1 / config.animation.particleCount)) % 1;
      const x = sourcePos.x + (targetPos.x - sourcePos.x) * t;
      const y = sourcePos.y + (targetPos.y - sourcePos.y) * t;
      const z = sourcePos.z + (targetPos.z - sourcePos.z) * t;
      particlePositions.setXYZ(i, x, y, z);
    }
    particlePositions.needsUpdate = true;
  });

  // Create cylindrical edge geometry
  const edgeGeometry = useMemo(() => {
    const direction = new THREE.Vector3().subVectors(targetPos, sourcePos);
    const distance = direction.length();
    const geometry = new THREE.CylinderGeometry(
      config.sizes.edgeWidth,
      config.sizes.edgeWidth,
      distance,
      8
    );
    return geometry;
  }, [sourcePos, targetPos, config.sizes.edgeWidth]);

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
          color={isActive ? config.colors.edges.active : config.colors.edges.inactive}
          transparent
          opacity={config.rendering.edgeOpacity}
        />
      </mesh>

      {/* Animated particles showing active data flow */}
      {isActive && (
        <points ref={particlesRef}>
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              count={config.animation.particleCount}
              array={new Float32Array(config.animation.particleCount * 3)}
              itemSize={3}
            />
          </bufferGeometry>
          <pointsMaterial
            size={0.4}
            color={config.colors.edges.particle}
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

// Main Enhanced Visualization Component
export const BotsVisualizationEnhanced: React.FC = () => {
  const [botsData, setBotsData] = useState<{
    nodes: BotsAgent[];
    edges: BotsEdge[];
    tokenUsage?: { total: number; byAgent: { [key: string]: number } };
  }>({ nodes: [], edges: [] });

  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showControlPanel, setShowControlPanel] = useState(true);
  const [config, setConfig] = useState<VisualizationConfig>(configurationMapper.getConfig());

  const positionsRef = useRef<Map<string, THREE.Vector3>>(new Map());
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const settings = useSettingsStore(state => state.settings);
  const { updateBotsData } = useBotsData();

  // Initialize mock data and configuration
  useEffect(() => {
    logger.info('Initializing enhanced visualization with mock data...');

    // Initialize mock data generator
    mockDataGenerator.initialize(12);

    // Initialize physics worker
    botsPhysicsWorker.init();

    // Subscribe to configuration changes
    const configId = 'enhanced-viz';
    configurationMapper.subscribe(configId, (newConfig) => {
      setConfig(newConfig);

      // Update physics configuration
      botsPhysicsWorker.updateConfig(newConfig.physics);
    });

    // Set up data update interval
    const updateData = () => {
      const nodes = mockDataGenerator.getAgents();
      const edges = mockDataGenerator.getEdges();
      const tokenUsage = mockDataGenerator.getTokenUsage();

      setBotsData({ nodes, edges, tokenUsage });

      // Update physics
      botsPhysicsWorker.updateAgents(nodes);
      botsPhysicsWorker.updateEdges(edges);
      if (tokenUsage) botsPhysicsWorker.updateTokenUsage(tokenUsage);
    };

    updateData();
    const interval = setInterval(updateData, 1000);

    setIsLoading(false);
    setError(null);

    return () => {
      clearInterval(interval);
      configurationMapper.unsubscribe(configId);
      mockDataGenerator.destroy();
      botsPhysicsWorker.cleanup();
    };
  }, []);

  // Update context when data changes
  useEffect(() => {
    updateBotsData({
      nodeCount: botsData.nodes.length,
      edgeCount: botsData.edges.length,
      tokenCount: botsData.tokenUsage?.total || 0,
      mcpConnected: false,
      dataSource: 'mock'
    });
  }, [botsData, updateBotsData]);

  // Physics simulation
  useFrame((state, delta) => {
    // Get positions from physics worker
    const workerPositions = botsPhysicsWorker.getPositions();
    if (workerPositions && workerPositions.size > 0) {
      workerPositions.forEach((pos, id) => {
        if (!positionsRef.current.has(id)) {
          positionsRef.current.set(id, new THREE.Vector3());
        }
        positionsRef.current.get(id)!.set(pos.x, pos.y, pos.z);
      });
    } else if (botsData.nodes.length > 0) {
      // Fallback simple physics if worker not available
      const dt = Math.min(delta, 0.016);

      // Initialize positions for new nodes
      botsData.nodes.forEach((node, index) => {
        if (!positionsRef.current.has(node.id)) {
          const angle = (index / botsData.nodes.length) * Math.PI * 2;
          const radius = 20;
          positionsRef.current.set(node.id, new THREE.Vector3(
            Math.cos(angle) * radius,
            Math.sin(angle) * radius,
            (Math.random() - 0.5) * 10
          ));
        }
      });

      // Simple physics simulation
      positionsRef.current.forEach((pos, id) => {
        const velocity = new THREE.Vector3();

        // Repulsion from other nodes
        positionsRef.current.forEach((otherPos, otherId) => {
          if (id !== otherId) {
            const diff = new THREE.Vector3().subVectors(pos, otherPos);
            const dist = diff.length();

            if (dist > 0 && dist < config.physics.nodeRepulsion) {
              const force = config.physics.nodeRepulsion / (dist * dist);
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
                const force = config.physics.springStrength * (dist - config.physics.linkDistance);
                diff.normalize().multiplyScalar(force * dt);
                velocity.add(diff);
              }
            }
          }
        });

        // Center gravity
        velocity.add(pos.clone().multiplyScalar(-config.physics.gravityStrength * dt));

        // Apply damping and update position
        velocity.multiplyScalar(config.physics.damping);
        velocity.clampLength(0, config.physics.maxVelocity);
        pos.add(velocity);

        // Limit position
        const maxDist = 30;
        if (pos.length() > maxDist) {
          pos.normalize().multiplyScalar(maxDist);
        }
      });
    }

    // Update instanced mesh if using it
    if (meshRef.current && botsData.nodes.length > 0) {
      meshRef.current.count = botsData.nodes.length;
      meshRef.current.instanceMatrix.needsUpdate = true;
    }
  });

  const handleConfigChange = useCallback((newConfig: VisualizationConfig) => {
    logger.info('Configuration changed:', newConfig);
  }, []);

  if (isLoading) {
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
            ⚡ Loading Enhanced VisionFlow...
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

  return (
    <group position={[0, 0, 0]}>
      {/* Control Panel */}
      {showControlPanel && (
        <BotsControlPanel
          position={[30, 10, 0]}
          onConfigChange={handleConfigChange}
        />
      )}

      {/* Debug info */}
      {debugState.isEnabled() && (
        <BotsDebugInfo
          isLoading={isLoading}
          error={error}
          nodeCount={botsData.nodes.length}
          edgeCount={botsData.edges.length}
          mcpConnected={false}
          dataSource={'mock'}
        />
      )}

      {/* Toggle control panel button */}
      <Html position={[30, 20, 0]}>
        <button
          onClick={() => setShowControlPanel(!showControlPanel)}
          style={{
            background: 'rgba(0, 0, 0, 0.8)',
            border: '2px solid #F1C40F',
            color: '#F1C40F',
            borderRadius: '5px',
            padding: '5px 10px',
            cursor: 'pointer',
            fontFamily: 'monospace',
            fontSize: '12px',
          }}
        >
          {showControlPanel ? 'Hide' : 'Show'} Control Panel
        </button>
      </Html>

      {/* Render nodes */}
      {botsData.nodes.length > 50 ? (
        // Use instanced mesh for large numbers
        <instancedMesh
          ref={meshRef}
          args={[undefined, undefined, botsData.nodes.length]}
          frustumCulled={false}
        >
          <sphereGeometry args={[config.sizes.nodeBaseSize, 16, 16]} />
          <meshStandardMaterial
            color={config.colors.agents.specialist}
            emissive={config.colors.agents.specialist}
            emissiveIntensity={config.rendering.emissiveIntensity}
            metalness={config.rendering.metalness}
            roughness={config.rendering.roughness}
            transparent
            opacity={config.rendering.nodeOpacity}
          />
        </instancedMesh>
      ) : (
        // Use individual nodes for smaller numbers
        botsData.nodes.map((agent, index) => {
          const position = positionsRef.current.get(agent.id) || new THREE.Vector3(
            Math.cos(index / botsData.nodes.length * Math.PI * 2) * 20,
            Math.sin(index / botsData.nodes.length * Math.PI * 2) * 20,
            (Math.random() - 0.5) * 10
          );

          return (
            <BotsNodeEnhanced
              key={agent.id}
              agent={agent}
              position={position}
              index={index}
              config={config}
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
            <BotsEdgeEnhanced
              key={edge.id}
              edge={edge}
              sourcePos={sourcePos}
              targetPos={targetPos}
              config={config}
            />
          );
        }
        return null;
      })}

      {/* Ambient particles for atmosphere */}
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
          color={config.colors.background.ambientParticles}
          transparent
          opacity={0.3}
          blending={THREE.NormalBlending}
          sizeAttenuation={true}
          depthWrite={false}
        />
      </points>
    </group>
  );
};