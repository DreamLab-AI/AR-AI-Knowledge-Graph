import React, { useEffect, useRef, useMemo } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { useSettingsStore } from '@/store/settingsStore';
import { Text, Html } from '@react-three/drei';
import { createLogger } from '../../../utils/loggerConfig';

const logger = createLogger('AgentNodesLayer');
import { isWebGPURenderer } from '../../../rendering/rendererFactory';
import { agentTrustKey, shortDid } from './agentIdentity';
import { AGENT_STATUS_COLORS, healthGlowColor, type AgentStatus, type HealthColorBands } from '../../bots/agentVisualConstants';



interface AgentNode {
  id: string;
  /**
   * Sovereign identity minted by agentbox at spawn (COM-14, ADR-125). When
   * present it is the trust key that supersedes `id` (the task_id); undefined
   * until agentbox attaches it and the server carries it through
   * `/api/bots/agents`. Wire key is snake_case `did_nostr`, matching the Rust
   * `Agent` serialisation.
   */
  did_nostr?: string;
  type: string;
  status: AgentStatus;
  health: number;
  cpuUsage: number;
  memoryUsage: number;
  workload: number;
  currentTask?: string;
  position?: { x: number; y: number; z: number };
  metadata?: Record<string, unknown>;
}

interface AgentConnection {
  source: string;
  target: string;
  type: 'communication' | 'coordination' | 'dependency';
  weight?: number;
}

interface AgentNodesLayerProps {
  agents: AgentNode[];
  connections?: AgentConnection[];
}

export const AgentNodesLayer: React.FC<AgentNodesLayerProps> = ({
  agents,
  connections = []
}) => {
  const groupRef = useRef<THREE.Group>(null);

  // Agent look-and-feel lives in the real persisted namespace
  // `visualisation.graphs.visionclaw.*` — the server resolves
  // "visionclaw"|"agent"|"bots" → graphs.visionclaw. These typed store reads
  // replace the former phantom `settings.agents.visualization.*` keys, which
  // existed neither in the client typed tree nor in Rust AppFullSettings and so
  // always resolved undefined. Now they are a control-centre Agents group.
  const nodeSize = useSettingsStore(s => s.get<number>('visualisation.graphs.visionclaw.nodes.nodeSize')) ?? 1.5;
  const baseColor = useSettingsStore(s => s.get<string>('visualisation.graphs.visionclaw.nodes.baseColor')) ?? '#ff8800';
  const connectionColor = useSettingsStore(s => s.get<string>('visualisation.graphs.visionclaw.edges.color')) ?? '#fbbf24';
  const connectionOpacity = useSettingsStore(s => s.get<number>('visualisation.graphs.visionclaw.edges.opacity')) ?? 0.4;
  // Activity breathing follows the global node-animation switch (typed).
  const animateActivity = useSettingsStore(s => s.get<boolean>('visualisation.animations.enableNodeAnimations')) ?? true;
  // Same multiplier semantics as GemNodes: defaults reproduce the layer's
  // historical rate (delta*2) and inhale depth (0.08) exactly.
  const breathingSpeed = useSettingsStore(s => s.get<number>('visualisation.graphTypeVisuals.agent.breathingSpeed')) ?? 1.5;
  const breathingAmplitude = useSettingsStore(s => s.get<number>('visualisation.graphTypeVisuals.agent.breathingAmplitude')) ?? 0.4;
  // Four configurable health→glow stops (control-centre Agents → Health). Absent
  // fields fall back to the canonical ramp inside healthGlowColor.
  const healthColors = useSettingsStore(s => s.get<HealthColorBands>('visualisation.graphTypeVisuals.agent.healthColors'));

  // Visibility authority is GraphManager's `nodeTypeVisibility.agent` gate (it
  // conditionally mounts this layer); here we only decide whether we have data.
  if (agents.length === 0) {
    return null;
  }

  return (
    <group ref={groupRef}>
      {}
      {agents.map((agent) => (
        <AgentNode
          key={agentTrustKey(agent)}
          agent={agent}
          nodeSize={nodeSize}
          baseColor={baseColor}
          animateActivity={animateActivity}
          breathingSpeed={breathingSpeed}
          breathingAmplitude={breathingAmplitude}
          healthColors={healthColors}
        />
      ))}

      {}
      {connections.map((connection, index) => (
        <AgentConnection
          key={`${connection.source}-${connection.target}-${index}`}
          connection={connection}
          agents={agents}
          color={connectionColor}
          baseOpacity={connectionOpacity}
        />
      ))}
    </group>
  );
};


const AgentNode: React.FC<{
  agent: AgentNode;
  nodeSize: number;
  baseColor: string;
  animateActivity: boolean;
  breathingSpeed: number;
  breathingAmplitude: number;
  healthColors?: HealthColorBands;
}> = ({ agent, nodeSize, baseColor, animateActivity, breathingSpeed, breathingAmplitude, healthColors }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);
  const nucleusRef = useRef<THREE.Mesh>(null);
  const pulseRef = useRef({ phase: 0 });
  // Independent lifecycle progress (0-1) so a materialise-in never contaminates a
  // later fade-out on the same node (and vice versa).
  const initProgressRef = useRef(0);
  const termProgressRef = useRef(0);

  const position: [number, number, number] = useMemo(() => {
    if (agent.position && (agent.position.x !== 0 || agent.position.y !== 0 || agent.position.z !== 0)) {
      return [agent.position.x, agent.position.y, agent.position.z];
    }
    // Deterministic fallback position from agent ID hash
    let hash = 0;
    for (let i = 0; i < agent.id.length; i++) {
      hash = ((hash << 5) - hash) + agent.id.charCodeAt(i);
      hash |= 0;
    }
    const pseudoRandom = (seed: number) => {
      const x = Math.sin(seed) * 10000;
      return x - Math.floor(x);
    };
    return [
      pseudoRandom(hash) * 20 - 10,
      pseudoRandom(hash + 1) * 20 - 10,
      pseudoRandom(hash + 2) * 20 - 10
    ];
  }, [agent.id, agent.position?.x, agent.position?.y, agent.position?.z]);

  const statusColor = AGENT_STATUS_COLORS[agent.status] ?? baseColor;
  const glowColor = useMemo(() => healthGlowColor(agent.health, healthColors), [agent.health, healthColors]);

  const scaledSize = nodeSize * (1 + agent.workload / 100);

  // Lifecycle transitions run over ~1.2s of frame time regardless of frame rate.
  const LIFECYCLE_DURATION = 1.2;

  useFrame((state, delta) => {
    if (!meshRef.current || !glowRef.current) return;
    const nucleusMat = nucleusRef.current?.material as THREE.MeshBasicMaterial | undefined;
    const status = agent.status;

    if (animateActivity && (status === 'active' || status === 'busy')) {
      // Active / busy — organic breathing: asymmetric inhale/exhale. Rate and
      // depth follow graphTypeVisuals.agent (defaults 1.5/0.4 ≡ old delta*2/0.08).
      pulseRef.current.phase += delta * breathingSpeed * (4 / 3);
      const breathCycle = Math.sin(pulseRef.current.phase);
      const breathScale = breathCycle > 0
        ? 1 + breathCycle * breathingAmplitude * 0.2
        : 1 + breathCycle * breathingAmplitude * 0.1;

      meshRef.current.scale.setScalar(scaledSize * breathScale);

      // Membrane breathes with slight delay
      const membraneBreath = 1.3 + Math.sin(pulseRef.current.phase - 0.3) * breathingAmplitude * 0.15;
      glowRef.current.scale.setScalar(membraneBreath);

      // Gentle rotation
      meshRef.current.rotation.y += delta * 0.5;

      // Nucleus glow pulse
      if (nucleusMat) {
        const nucleusPulse = Math.pow(Math.sin(pulseRef.current.phase * 0.6 + 0.5) * 0.5 + 0.5, 2);
        nucleusMat.opacity = 0.2 + nucleusPulse * 0.3;
      }
    } else if (status === 'error') {
      // Distress flicker
      pulseRef.current.phase += delta * 8;
      const distress = Math.sin(pulseRef.current.phase) * Math.sin(pulseRef.current.phase * 0.66) * 0.15;
      meshRef.current.scale.setScalar(scaledSize * (1 + Math.abs(distress)));
      glowRef.current.scale.setScalar(1.3 + Math.abs(distress) * 0.5);
    } else if (status === 'initializing') {
      // Opacity ramp-in: the node materialises from a point, membrane glowing up.
      initProgressRef.current = Math.min(1, initProgressRef.current + delta / LIFECYCLE_DURATION);
      const p = initProgressRef.current * initProgressRef.current; // ease-in
      meshRef.current.scale.setScalar(scaledSize * p);
      glowRef.current.scale.setScalar(1.3 * (0.4 + p * 0.6));
      if (nucleusMat) nucleusMat.opacity = 0.05 + p * 0.25;
    } else if (status === 'terminating') {
      // Fade-out: the node dematerialises toward a point.
      termProgressRef.current = Math.min(1, termProgressRef.current + delta / LIFECYCLE_DURATION);
      const fade = 1 - termProgressRef.current;
      meshRef.current.scale.setScalar(scaledSize * fade);
      glowRef.current.scale.setScalar(1.3 * fade);
      if (nucleusMat) nucleusMat.opacity = 0.2 * fade;
    } else if (status === 'offline') {
      // Desaturated static: no pulse, dim core, membrane pulled in.
      meshRef.current.scale.setScalar(scaledSize);
      glowRef.current.scale.setScalar(1.05);
      if (nucleusMat) nucleusMat.opacity = 0.05;
    } else {
      // Idle: base scale and a very subtle life sign.
      meshRef.current.scale.setScalar(scaledSize);
      glowRef.current.scale.setScalar(1.3);
      if (nucleusMat) {
        pulseRef.current.phase += delta * 0.5;
        nucleusMat.opacity = 0.1 + Math.sin(pulseRef.current.phase) * 0.05;
      }
    }
  });

  // Unit-size geometry keyed only on agent.type -- scaledSize applied via mesh scale
  const geometry = useMemo(() => {
    switch (agent.type) {
      case 'researcher':
        return new THREE.OctahedronGeometry(1.0, 0);
      case 'coder':
        return new THREE.BoxGeometry(1.5, 1.5, 1.5);
      case 'analyzer':
        return new THREE.TetrahedronGeometry(1.0, 0);
      case 'tester':
        return new THREE.ConeGeometry(1.0, 2.0, 6);
      case 'optimizer':
        return new THREE.TorusGeometry(0.8, 0.3, 8, 12);
      case 'coordinator':
        return new THREE.IcosahedronGeometry(1.0, 0);
      default:
        return new THREE.SphereGeometry(1.0, 10, 8);
    }
  }, [agent.type]);

  useEffect(() => {
    return () => { geometry?.dispose(); };
  }, [geometry]);

  return (
    <group position={position}>
      {/* Outer membrane (bioluminescent) */}
      <mesh ref={glowRef} scale={[1.3, 1.3, 1.3]}>
        <sphereGeometry args={[scaledSize * 0.75, 10, 8]} />
        <meshStandardMaterial
          color={glowColor}
          transparent
          opacity={0.08}
          side={THREE.BackSide}
          depthWrite={false}
          emissive={glowColor}
          emissiveIntensity={0.3}
        />
      </mesh>

      {/* Inner nucleus glow */}
      <mesh ref={nucleusRef} scale={[0.4, 0.4, 0.4]}>
        <sphereGeometry args={[scaledSize * 0.8, 12, 12]} />
        <meshBasicMaterial
          color={statusColor}
          transparent
          opacity={0.25}
          blending={THREE.AdditiveBlending}
          depthWrite={false}
        />
      </mesh>

      {/* Main body */}
      <mesh ref={meshRef} geometry={geometry}>
        <meshStandardMaterial
          color={statusColor}
          emissive={glowColor}
          emissiveIntensity={agent.status === 'active' || agent.status === 'busy' ? 0.5 : 0.2}
          metalness={0.3}
          roughness={0.7}
        />
      </mesh>

      {/* Agent type label — Html on WebGPU (troika Text Line2 geometry triggers drawIndexed(Infinity); troika limitation, not version-specific) */}
      {isWebGPURenderer ? (
        <Html position={[0, scaledSize + 1.5, 0]} center style={{ pointerEvents: 'none', whiteSpace: 'nowrap' }}>
          <div style={{ color: statusColor, fontSize: '12px', fontWeight: 'bold', textShadow: '0 0 4px black' }}>
            {agent.type.toUpperCase()}
          </div>
          <div style={{ color: '#fff', fontSize: '10px', textShadow: '0 0 3px black' }}>
            {agent.status} | {agent.health}%
          </div>
          {agent.did_nostr && (
            <div style={{ color: '#7dd3fc', fontSize: '9px', fontFamily: 'monospace', textShadow: '0 0 3px black' }}>
              {shortDid(agent.did_nostr)}
            </div>
          )}
          {agent.currentTask && (
            <div style={{ color: '#aaa', fontSize: '9px', maxWidth: '120px', overflow: 'hidden', textOverflow: 'ellipsis' }}>
              {agent.currentTask}
            </div>
          )}
        </Html>
      ) : (
        <>
          <Text
            position={[0, scaledSize + 1, 0]}
            fontSize={0.5}
            color={statusColor}
            anchorX="center"
            anchorY="bottom"
            outlineWidth={0.03}
            outlineColor="black"
          >
            {agent.type.toUpperCase()}
          </Text>
          <Text
            position={[0, scaledSize + 1.5, 0]}
            fontSize={0.3}
            color="#ffffff"
            anchorX="center"
            anchorY="bottom"
            outlineWidth={0.02}
            outlineColor="black"
          >
            {agent.status} | {agent.health}%
          </Text>
          {agent.did_nostr && (
            <Text
              position={[0, scaledSize + 2.0, 0]}
              fontSize={0.22}
              color="#7dd3fc"
              anchorX="center"
              anchorY="bottom"
              outlineWidth={0.015}
              outlineColor="black"
            >
              {shortDid(agent.did_nostr)}
            </Text>
          )}
          {agent.currentTask && (
            <Text
              position={[0, -(scaledSize + 1), 0]}
              fontSize={0.25}
              color="#aaaaaa"
              anchorX="center"
              anchorY="top"
              maxWidth={10}
              outlineWidth={0.01}
              outlineColor="black"
            >
              {agent.currentTask}
            </Text>
          )}
        </>
      )}

      {/* Health bar with gradient glow */}
      <group position={[0, -(scaledSize + 0.5), 0]}>
        {/* Background track */}
        <mesh position={[0, 0, 0]}>
          <planeGeometry args={[2, 0.15]} />
          <meshBasicMaterial color="#1a1a1a" transparent opacity={0.6} />
        </mesh>
        {/* Health fill with bioluminescent color */}
        <mesh position={[-(1 - agent.health / 100), 0, 0.01]}>
          <planeGeometry args={[(agent.health / 100) * 2, 0.15]} />
          <meshBasicMaterial
            color={glowColor}
            transparent
            opacity={0.9}
          />
        </mesh>
        {/* Glow overlay on health bar */}
        <mesh position={[-(1 - agent.health / 100), 0, 0.02]}>
          <planeGeometry args={[(agent.health / 100) * 2, 0.25]} />
          <meshBasicMaterial
            color={glowColor}
            transparent
            opacity={0.15}
            blending={THREE.AdditiveBlending}
            depthWrite={false}
          />
        </mesh>
      </group>

      {/* Workload ring */}
      {(agent.status === 'active' || agent.status === 'busy') && agent.workload > 0 && (
        <mesh rotation={[Math.PI / 2, 0, 0]}>
          <torusGeometry args={[scaledSize * 1.8, 0.05, 8, 32, (agent.workload / 100) * Math.PI * 2]} />
          <meshBasicMaterial
            color={glowColor}
            transparent
            opacity={0.6}
          />
        </mesh>
      )}
    </group>
  );
};


const AgentConnection: React.FC<{
  connection: AgentConnection;
  agents: AgentNode[];
  color: string;
  baseOpacity: number;
}> = ({ connection, agents, color, baseOpacity }) => {
  const lineRef = useRef<THREE.Line>(null);

  
  const sourceAgent = agents.find(a => a.id === connection.source);
  const targetAgent = agents.find(a => a.id === connection.target);

  if (!sourceAgent || !targetAgent || !sourceAgent.position || !targetAgent.position) {
    return null;
  }

  // Safe: early return above guarantees position is defined
  const sourcePos = useMemo(() => new THREE.Vector3(
    sourceAgent.position!.x, sourceAgent.position!.y, sourceAgent.position!.z
  ), [sourceAgent.position?.x, sourceAgent.position?.y, sourceAgent.position?.z]);

  const targetPos = useMemo(() => new THREE.Vector3(
    targetAgent.position!.x, targetAgent.position!.y, targetAgent.position!.z
  ), [targetAgent.position?.x, targetAgent.position?.y, targetAgent.position?.z]);

  
  const points = useMemo(() => {
    const midPoint = new THREE.Vector3()
      .addVectors(sourcePos, targetPos)
      .multiplyScalar(0.5);

    
    const direction = new THREE.Vector3().subVectors(targetPos, sourcePos);
    const perpendicular = new THREE.Vector3(-direction.y, direction.x, 0).normalize();
    midPoint.add(perpendicular.multiplyScalar(2));

    const curve = new THREE.QuadraticBezierCurve3(sourcePos, midPoint, targetPos);
    return curve.getPoints(50);
  }, [sourcePos, targetPos]);

  const geometry = useMemo(() => {
    return new THREE.BufferGeometry().setFromPoints(points);
  }, [points]);

  useEffect(() => {
    return () => { geometry?.dispose(); };
  }, [geometry]);


  useFrame((state) => {
    if (lineRef.current) {
      const material = lineRef.current.material as THREE.LineBasicMaterial;
      // Pulse around the configured base opacity (visionclaw edges.opacity).
      material.opacity = Math.max(0, baseOpacity * (0.75 + Math.sin(state.clock.elapsedTime * 2) * 0.25));
    }
  });


  const lineWidth = connection.weight ? connection.weight * 2 : 2;
  // Communication links read slightly brighter than coordination/dependency ones,
  // scaled off the user-configured base opacity.
  const opacity = baseOpacity * (connection.type === 'communication' ? 1.0 : 0.7);

  const lineMaterial = useMemo(() => new THREE.LineBasicMaterial({
    color,
    linewidth: lineWidth,
    transparent: true,
    opacity
  }), [color, lineWidth, opacity]);

  const lineObject = useMemo(() => {
    if (!geometry) return null;
    return new THREE.Line(geometry, lineMaterial);
  }, [geometry, lineMaterial]);

  useEffect(() => {
    return () => {
      lineMaterial?.dispose();
    };
  }, [lineMaterial]);

  return (
    <>
      {lineObject && <primitive object={lineObject} ref={lineRef} />}
    </>
  );
};


// Secondary telemetry poll cadence (seconds) for this standalone agent layer.
// The typed settings tree exposes no polling-interval field — AgentPollingService
// owns the primary `/graph/data?graph_type=agent` poll — so this module constant
// is the single source for the legacy `/api/bots/*` fallback poll below. Replaces
// the phantom `settings.agents.monitoring.telemetry_poll_interval` read.
const AGENT_TELEMETRY_POLL_SECONDS = 5;

export const useAgentNodes = () => {
  const [agents, setAgents] = React.useState<AgentNode[]>([]);
  const [connections, setConnections] = React.useState<AgentConnection[]>([]);

  useEffect(() => {
    const pollAgents = async () => {
      try {
        const response = await fetch('/api/bots/agents');
        if (response.ok) {
          const data = await response.json();
          setAgents(data.agents || []);
        }
      } catch (error) {
        logger.error('Failed to fetch agent telemetry:', error);
      }
    };

    const pollConnections = async () => {
      try {
        const response = await fetch('/api/bots/data');
        if (response.ok) {
          const data = await response.json();
          setConnections(data.edges || []);
        }
      } catch (error) {
        logger.error('Failed to fetch agent connections:', error);
      }
    };

    // Poll at the fixed fallback cadence
    const interval = AGENT_TELEMETRY_POLL_SECONDS * 1000;

    const timer = setInterval(() => {
      pollAgents();
      pollConnections();
    }, interval);

    pollAgents();
    pollConnections();

    return () => clearInterval(timer);
  }, []);

  return { agents, connections };
};

export default AgentNodesLayer;
