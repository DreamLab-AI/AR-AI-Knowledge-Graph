/**
 * BotsNode.tsx
 * Per-agent 3-D node with:
 *   - Dynamic geometry (shape encodes status/type)
 *   - Organic breathing/metabolic useFrame animation
 *   - Bioluminescent membrane + nucleus glow
 *   - Queen corona ring
 *   - High-token-rate vibration / float / memory-pressure shake
 *   - Billboard label with 5 display modes (click to cycle)
 *   - AgentStatusBadges HTML overlay on hover / active
 *
 * Zero-alloc contract: all THREE objects used inside useFrame are refs
 * (currentPositionRef, targetPositionRef, lastPositionRef) — never allocated
 * per frame.
 */
import React, { useRef, useEffect, useState, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';
import { Html, Text, Billboard } from '@react-three/drei';
import { BotsAgent } from '../types/BotsTypes';
import { useTelemetry, useThreeJSTelemetry } from '../../../telemetry/useTelemetry';
import { useSettingsStore } from '../../../store/settingsStore';
import {
  lerpVector3,
  formatProcessingLogs,
  applySwarmTint,
  ADDITIVE_BLENDING,
  BACK_SIDE,
} from './BotsShared';
import { healthGlowColor, agentStatusColor } from '../agentVisualConstants';
import { AgentStatusBadges } from './AgentStatusBadges';

export interface BotsNodeProps {
  agent: BotsAgent;
  position: THREE.Vector3;
  index: number;
  color: string;
  /**
   * When true (default) the body colour is hue-rotated by a stable per-swarmId
   * offset so multi-swarm scenes read as families. Pass false to render every
   * swarm on the flat type colour.
   */
  swarmTint?: boolean;
}

export const BotsNode: React.FC<BotsNodeProps> = ({ agent, position, index, color, swarmTint = true }) => {
  const groupRef   = useRef<THREE.Group>(null);
  const meshRef    = useRef<THREE.Mesh>(null);
  const glowRef    = useRef<THREE.Mesh>(null);
  const nucleusRef = useRef<THREE.Mesh>(null);
  const coronaRef  = useRef<THREE.Mesh>(null);
  // declaredIntent pre-action flash: clock time the current flash began (-1 idle).
  const flashStartRef = useRef(-1);
  const prevIntentRef = useRef<string | undefined>(agent.declaredIntent);
  const [hover, setHover] = useState(false);
  const [displayMode, setDisplayMode] = useState<
    'overview' | 'performance' | 'tasks' | 'network' | 'resources'
  >('overview');
  const telemetry      = useTelemetry(`BotsNode-${agent.id}`);
  const threeJSTelemetry = useThreeJSTelemetry(agent.id);
  const lastPositionRef    = useRef<THREE.Vector3 | undefined>(undefined);
  const currentPositionRef = useRef<THREE.Vector3>(position.clone());
  const targetPositionRef  = useRef<THREE.Vector3>(position.clone());
  const elapsedTimeRef     = useRef(0);
  const settings = useSettingsStore(state => state.settings);

  const healthColors = settings?.visualisation?.graphTypeVisuals?.agent?.healthColors;
  const glowColor = useMemo(
    () => healthGlowColor(agent.health || 0, healthColors),
    [agent.health, healthColors],
  );

  const isQueen = agent.type === 'queen';

  const statusColor = useMemo(() => agentStatusColor(agent.status), [agent.status]);

  // Swarm partition tint: stable per-swarmId hue rotation of the base body colour
  // so agents from the same swarm read as a family (default ON via swarmTint).
  const bodyColor = useMemo(
    () => (swarmTint ? applySwarmTint(color, agent.swarmId) : color),
    [swarmTint, color, agent.swarmId],
  );

  const baseSize      = 1.0;
  const cpuScale      = agent.cpuUsage    ? (agent.cpuUsage / 100) * 0.8 : 0;
  const workloadScale = agent.workload    ? agent.workload * 0.6           : 0;
  const activityScale = agent.activity   ? agent.activity * 0.4           : 0;
  const tokenScale    = agent.tokenRate  ? Math.min(agent.tokenRate / 50, 0.5) : 0;
  const clampedSize   = Math.max(0.5, Math.min(
    baseSize + cpuScale + workloadScale + activityScale + tokenScale,
    3.0,
  ));

  const geometry = useMemo(() => {
    const r = clampedSize;
    switch (agent.status) {
      case 'error':        return new THREE.TetrahedronGeometry(r * 1.2);
      case 'terminating':  return new THREE.OctahedronGeometry(r);
      case 'initializing': return new THREE.BoxGeometry(r, r, r);
      case 'idle':         return new THREE.SphereGeometry(r * 0.8, 8, 6);
      case 'offline':      return new THREE.CylinderGeometry(r * 0.5, r * 0.5, r);
      case 'busy':
        switch (agent.type) {
          case 'queen':       return new THREE.IcosahedronGeometry(r * 1.3, 1);
          case 'coordinator': return new THREE.DodecahedronGeometry(r * 1.1);
          case 'architect':   return new THREE.ConeGeometry(r, r * 1.5, 8);
          default:            return new THREE.SphereGeometry(r, 10, 8);
        }
      case 'active':
      default:
        return new THREE.SphereGeometry(r, 10, 8);
    }
  }, [agent.status, agent.type, clampedSize]);

  useEffect(() => {
    return () => { geometry?.dispose(); };
  }, [geometry]);

  useFrame((state) => {
    if (!groupRef.current || !meshRef.current || !glowRef.current) return;

    telemetry.startRender();

    if (!lastPositionRef.current || !lastPositionRef.current.equals(position)) {
      threeJSTelemetry.logPositionUpdate(
        { x: position.x, y: position.y, z: position.z },
        { agentType: agent.type, agentStatus: agent.status },
      );
      if (!lastPositionRef.current) {
        lastPositionRef.current = position.clone();
      } else {
        lastPositionRef.current.copy(position);
      }
    }

    targetPositionRef.current.copy(position);
    lerpVector3(currentPositionRef.current, targetPositionRef.current, 0.15);
    groupRef.current.position.copy(currentPositionRef.current);

    const elapsedTime = state.clock.elapsedTime;
    elapsedTimeRef.current = elapsedTime;

    // declaredIntent pre-action flash: a new non-empty declared intent starts a
    // brief (~600ms) aura spike — the "about to act" cue before the agent moves.
    const intent = agent.declaredIntent;
    if (intent && intent !== prevIntentRef.current) {
      flashStartRef.current = elapsedTime;
    }
    prevIntentRef.current = intent;
    let flashEnv = 0;
    if (flashStartRef.current >= 0) {
      const t = (elapsedTime - flashStartRef.current) / 0.6;
      if (t >= 1) flashStartRef.current = -1;
      else flashEnv = 1 - t; // linear decay from the intent instant
    }

    const activity   = agent.activity ?? 0;
    const healthPulse = agent.health ? (agent.health / 100) : 0.5;
    const tokenGlow  = agent.tokenRate ? Math.min(agent.tokenRate / 20, 2) : 0;

    // Organic breathing & metabolic pulse
    if (agent.status === 'active' || agent.status === 'busy') {
      const tokenMultiplier  = agent.tokenRate ? Math.min(agent.tokenRate / 10, 3) : 1;
      const healthMultiplier = agent.health    ? Math.max(0.3, agent.health / 100)  : 1;
      const pulseSpeed       = 2 * tokenMultiplier * healthMultiplier;

      const breathCycle = Math.sin(elapsedTime * pulseSpeed * 0.8 + index);
      const breathScale = breathCycle > 0
        ? 1 + breathCycle * 0.08   // gentle inhale
        : 1 + breathCycle * 0.04;  // slower exhale
      meshRef.current.scale.setScalar(breathScale * clampedSize);

      const membraneScale  = isQueen ? 1.5 : 1.3;
      const membraneBreath = 0.08 + healthPulse * 0.04;
      const glowBreathScale = membraneScale
        + Math.sin(elapsedTime * pulseSpeed * 0.7 + index + 0.3) * membraneBreath;
      const statusGlow    = agent.status === 'busy' ? 1.5 : 1.0;
      const glowIntensity = (tokenGlow > 0 ? tokenGlow : 1) * healthPulse * statusGlow;
      glowRef.current.scale.setScalar(glowBreathScale * glowIntensity);

      if (nucleusRef.current) {
        const nucleusGlow = Math.pow(Math.sin(elapsedTime * 1.2 + 0.5 + index) * 0.5 + 0.5, 2);
        const nucleusMat  = nucleusRef.current.material as THREE.MeshBasicMaterial;
        if (nucleusMat) {
          nucleusMat.opacity = 0.3 + activity * 0.3 + nucleusGlow * 0.2;
        }
        nucleusRef.current.scale.setScalar(0.4 + nucleusGlow * 0.05);
      }

      const glowMat = glowRef.current.material as THREE.MeshStandardMaterial;
      if (glowMat && glowMat.opacity !== undefined) {
        glowMat.emissiveIntensity = 0.3 + tokenGlow * 0.2;
      }
    } else if (agent.status === 'error') {
      const distress  = Math.sin(elapsedTime * 8 + index) * Math.sin(elapsedTime * 5.3 + index) * 0.2;
      const errorPulse = 1 + Math.abs(distress) + Math.sin(elapsedTime * 8 + index) * 0.15;
      meshRef.current.scale.setScalar(errorPulse * clampedSize);
      glowRef.current.scale.setScalar(errorPulse * 2.0);
      if (nucleusRef.current) {
        const flickerMat = nucleusRef.current.material as THREE.MeshBasicMaterial;
        if (flickerMat) {
          flickerMat.opacity = 0.2 + Math.abs(Math.sin(elapsedTime * 12 + index)) * 0.5;
        }
      }
    } else {
      if (nucleusRef.current) {
        const idleMat = nucleusRef.current.material as THREE.MeshBasicMaterial;
        if (idleMat) {
          idleMat.opacity = 0.15 + Math.sin(elapsedTime * 0.5 + index) * 0.05;
        }
      }
    }

    // Busy cytoplasm churn
    if (agent.status === 'busy') {
      if (isQueen) {
        meshRef.current.rotation.y += 0.005;
      } else {
        const rotationSpeed = agent.tokenRate ? 0.01 * (1 + agent.tokenRate / 50) : 0.01;
        meshRef.current.rotation.y += rotationSpeed;
      }
      groupRef.current.rotation.x += Math.sin(elapsedTime * 0.7 + index) * 0.02 * 0.1;
      groupRef.current.rotation.z += Math.cos(elapsedTime * 0.5 + index * 0.7) * 0.02 * 0.1;
    }

    // Queen corona
    if (isQueen && coronaRef.current) {
      coronaRef.current.rotation.y -= 0.003;
      coronaRef.current.rotation.z  = Math.sin(elapsedTime * 0.4) * 0.05;
      const coronaMat = coronaRef.current.material as THREE.MeshBasicMaterial;
      if (coronaMat) {
        coronaMat.opacity = 0.12 + Math.sin(elapsedTime * 0.8) * 0.04;
      }
    }

    // High token-rate vibration + float
    if (agent.tokenRate && agent.tokenRate > 30) {
      meshRef.current.position.y += Math.sin(elapsedTime * 15 + index) * 0.03
        + Math.cos(elapsedTime * 3 + index) * 0.1;
    }

    // Memory pressure shake
    if (agent.memoryUsage && agent.memoryUsage > 80) {
      const shake = Math.sin(elapsedTime * 25) * 0.01;
      meshRef.current.position.x += shake;
      meshRef.current.position.z += shake * 0.7;
    }

    // Critical health alarm pulse
    if (agent.health && agent.health < 25) {
      meshRef.current.scale.multiplyScalar(Math.sin(elapsedTime * 12) * 0.5 + 1);
    }

    // declaredIntent flash: brief whole-node swell + core brighten. Group scale is
    // set absolutely (identity when idle) so the spike never accumulates; the
    // nucleus opacity was set absolutely in the status branch above, so adding the
    // spike here is safe and resets next frame.
    groupRef.current.scale.setScalar(1 + flashEnv * 0.18);
    if (flashEnv > 0 && nucleusRef.current) {
      const flashMat = nucleusRef.current.material as THREE.MeshBasicMaterial;
      if (flashMat) flashMat.opacity = Math.min(1, flashMat.opacity + flashEnv * 0.6);
    }

    telemetry.endRender();
  });

  const processingLogs = formatProcessingLogs(agent.processingLogs);

  return (
    <group ref={groupRef}>
      {/* Outer membrane */}
      <mesh ref={glowRef} scale={[isQueen ? 1.5 : 1.3, isQueen ? 1.5 : 1.3, isQueen ? 1.5 : 1.3]}>
        <sphereGeometry args={[clampedSize * 0.75, 10, 8]} />
        <meshStandardMaterial
          color={isQueen ? '#FFD700' : glowColor}
          transparent
          opacity={0.08 + (hover ? 0.06 : 0)
            + (agent.tokenRate ? Math.min(agent.tokenRate / 100, 0.12) : 0)}
          side={BACK_SIDE}
          depthWrite={false}
          emissive={isQueen ? '#FFD700' : glowColor}
          emissiveIntensity={0.3 + (agent.tokenRate ? Math.min(agent.tokenRate / 20, 2) * 0.2 : 0)}
        />
      </mesh>

      {/* Inner nucleus glow */}
      <mesh ref={nucleusRef} scale={[0.4, 0.4, 0.4]}>
        <sphereGeometry args={[clampedSize * 0.8, 12, 12]} />
        <meshBasicMaterial
          color={isQueen ? '#FFD700' : statusColor}
          transparent
          opacity={0.3 + (agent.activity ?? 0) * 0.3}
          blending={ADDITIVE_BLENDING}
          depthWrite={false}
        />
      </mesh>

      {/* Queen golden corona ring */}
      {isQueen && (
        <mesh ref={coronaRef} rotation={[Math.PI / 2, 0, 0]}>
          <torusGeometry args={[clampedSize * 1.8, clampedSize * 0.08, 16, 48]} />
          <meshBasicMaterial
            color="#FFD700"
            transparent
            opacity={0.14}
            blending={ADDITIVE_BLENDING}
            depthWrite={false}
          />
        </mesh>
      )}

      {/* Main agent body */}
      <mesh
        ref={meshRef}
        geometry={geometry}
        onPointerOver={() => {
          setHover(true);
          telemetry.logInteraction('hover_start', {
            agentId: agent.id, agentType: agent.type,
            health: agent.health, cpuUsage: agent.cpuUsage,
            tokenRate: agent.tokenRate, status: agent.status, nodeSize: clampedSize,
          });
        }}
        onPointerOut={() => {
          setHover(false);
          telemetry.logInteraction('hover_end', { agentId: agent.id, agentType: agent.type, hoverDuration: 'hover_ended' });
        }}
        onClick={() => {
          const modes: Array<'overview' | 'performance' | 'tasks' | 'network' | 'resources'> =
            ['overview', 'performance', 'tasks', 'network', 'resources'];
          const nextMode = modes[(modes.indexOf(displayMode) + 1) % modes.length];
          setDisplayMode(nextMode);
          telemetry.logInteraction('click', {
            agentId: agent.id, agentType: agent.type, displayMode: nextMode,
            position: { x: position.x, y: position.y, z: position.z },
            health: agent.health, status: agent.status, currentTask: agent.currentTask,
            capabilities: agent.capabilities?.slice(0, 3),
          });
        }}
      >
        <meshStandardMaterial
          color={bodyColor}
          emissive={glowColor}
          emissiveIntensity={(() => {
            const glowSettings = settings?.visualisation?.glow;
            const baseIntensity = glowSettings?.nodeGlowStrength ?? 0.7;
            return (agent.status === 'active' || agent.status === 'busy')
              ? baseIntensity * 0.7
              : baseIntensity * 0.3;
          })()}
          metalness={0.3}
          roughness={0.7}
          transparent={agent.status === 'error' || agent.status === 'terminating'}
          opacity={agent.status === 'error' || agent.status === 'terminating' ? 0.7 : 1.0}
        />
      </mesh>

      {/* HTML overlay */}
      {(hover || agent.status === 'active' || agent.status === 'busy') && (
        <Html
          center
          distanceFactor={8}
          style={{
            transition: 'all 0.3s ease-in-out',
            opacity: hover ? 1 : 0.85,
            pointerEvents: 'none',
            position: 'absolute',
            top: `${-clampedSize * 25}px`,
            left: '0',
            transform: hover ? 'scale(1.05)' : 'scale(1)',
            filter: hover ? 'drop-shadow(0 4px 8px rgba(0,0,0,0.3))' : 'none',
          }}
        >
          <AgentStatusBadges agent={agent} logs={processingLogs} />
        </Html>
      )}

      {/* High-activity ring + token particles */}
      {((agent.tokenRate ?? 0) > 30 || agent.cpuUsage > 80) && (
        <group>
          <mesh rotation={[Math.PI / 2, 0, 0]} position={[0, clampedSize + 0.2, 0]}>
            <ringGeometry args={[clampedSize * 1.1, clampedSize * 1.3, 16]} />
            <meshBasicMaterial
              color={agent.cpuUsage > 90 ? '#E74C3C' : agent.cpuUsage > 70 ? '#F39C12' : '#2ECC71'}
              transparent opacity={0.6} side={THREE.DoubleSide}
            />
          </mesh>

          {(agent.tokenRate ?? 0) > 50 && [
            ...Array(Math.min(Math.floor((agent.tokenRate ?? 0) / 10), 8))
          ].map((_, i) => {
            const angle  = (i / 8) * Math.PI * 2;
            const radius = clampedSize * 2;
            const x = Math.cos(angle + elapsedTimeRef.current) * radius;
            const z = Math.sin(angle + elapsedTimeRef.current) * radius;
            return (
              <mesh key={i} position={[x, 0, z]}>
                <sphereGeometry args={[0.03, 6, 6]} />
                <meshBasicMaterial color="#F39C12" transparent opacity={0.8} />
              </mesh>
            );
          })}
        </group>
      )}

      {/* Billboard labels */}
      <Billboard follow lockX={false} lockY={false} lockZ={false}>
        <Text position={[0, clampedSize + 0.8, 0]} fontSize={0.18} color="#3498DB"
          anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
          [{displayMode.toUpperCase()}]
        </Text>
        <Text position={[0, -clampedSize - 0.7, 0]} fontSize={0.4} color="white"
          anchorX="center" anchorY="middle" outlineWidth={0.05} outlineColor="black">
          {agent.name || String(agent.id).slice(0, 8)}
        </Text>

        {displayMode === 'overview' && (<>
          <Text position={[0, -clampedSize - 1.1, 0]} fontSize={0.25} color={color}
            anchorX="center" anchorY="middle" outlineWidth={0.03} outlineColor="black">
            {agent.type.toUpperCase()}
          </Text>
          <Text position={[0, -clampedSize - 1.4, 0]} fontSize={0.2} color={glowColor}
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Health: {agent.health ? `${agent.health.toFixed(0)}%` : 'N/A'}
          </Text>
          <Text position={[0, -clampedSize - 1.7, 0]} fontSize={0.15} color="#95A5A6"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Status: {agent.status}
          </Text>
        </>)}

        {displayMode === 'performance' && (<>
          <Text position={[0, -clampedSize - 1.1, 0]} fontSize={0.2}
            color={agent.cpuUsage > 80 ? '#E74C3C' : agent.cpuUsage > 50 ? '#F39C12' : '#2ECC71'}
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            CPU: {agent.cpuUsage?.toFixed(0) || 0}%
          </Text>
          <Text position={[0, -clampedSize - 1.4, 0]} fontSize={0.2} color="#9B59B6"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            MEM: {agent.memoryUsage?.toFixed(0) || 0}%
          </Text>
          <Text position={[0, -clampedSize - 1.7, 0]} fontSize={0.18}
            color={(agent.tokenRate ?? 0) > 20 ? '#E67E22' : '#3498DB'}
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Tokens: {agent.tokenRate?.toFixed(1) || 0}/min
          </Text>
          <Text position={[0, -clampedSize - 2.0, 0]} fontSize={0.15} color="#F39C12"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Total: {agent.tokens?.toLocaleString() || 0}
          </Text>
        </>)}

        {displayMode === 'tasks' && (<>
          <Text position={[0, -clampedSize - 1.1, 0]} fontSize={0.2} color="#2ECC71"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Active: {agent.tasksActive || 0}
          </Text>
          <Text position={[0, -clampedSize - 1.4, 0]} fontSize={0.2} color="#3498DB"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Done: {agent.tasksCompleted || 0}
          </Text>
          <Text position={[0, -clampedSize - 1.7, 0]} fontSize={0.15} color="#95A5A6"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            {agent.currentTask ? agent.currentTask.substring(0, 20) + '...' : 'Idle'}
          </Text>
          {agent.successRate !== undefined && (
            <Text position={[0, -clampedSize - 2.0, 0]} fontSize={0.15}
              color={agent.successRate > 0.8 ? '#27AE60' : agent.successRate > 0.6 ? '#F39C12' : '#E74C3C'}
              anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
              Success: {(agent.successRate * 100).toFixed(0)}%
            </Text>
          )}
        </>)}

        {displayMode === 'network' && (<>
          <Text position={[0, -clampedSize - 1.1, 0]} fontSize={0.18} color="#E67E22"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Swarm: {agent.swarmId?.substring(0, 8) || 'None'}
          </Text>
          <Text position={[0, -clampedSize - 1.4, 0]} fontSize={0.18} color="#F39C12"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Mode: {agent.agentMode || 'Default'}
          </Text>
          {agent.parentQueenId && (
            <Text position={[0, -clampedSize - 1.7, 0]} fontSize={0.15} color="#FFD700"
              anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
              Queen: {agent.parentQueenId.substring(0, 8)}
            </Text>
          )}
          <Text position={[0, -clampedSize - 2.0, 0]} fontSize={0.15} color="#95A5A6"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Age: {agent.age ? Math.floor(agent.age / 1000 / 60) : 0}m
          </Text>
        </>)}

        {displayMode === 'resources' && (<>
          <Text position={[0, -clampedSize - 1.1, 0]} fontSize={0.18} color="#3498DB"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Workload: {((agent.workload ?? 0) * 100).toFixed(0)}%
          </Text>
          <Text position={[0, -clampedSize - 1.4, 0]} fontSize={0.18} color="#2ECC71"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            Activity: {((agent.activity ?? 0) * 100).toFixed(0)}%
          </Text>
          {agent.capabilities && agent.capabilities.length > 0 && (
            <Text position={[0, -clampedSize - 1.7, 0]} fontSize={0.15} color="#9B59B6"
              anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
              Caps: {agent.capabilities.length} total
            </Text>
          )}
          <Text position={[0, -clampedSize - 2.0, 0]} fontSize={0.13} color="#95A5A6"
            anchorX="center" anchorY="middle" outlineWidth={0.02} outlineColor="black">
            {agent.capabilities?.[0]?.replace(/_/g, ' ') || 'None'}
          </Text>
        </>)}
      </Billboard>
    </group>
  );
};
