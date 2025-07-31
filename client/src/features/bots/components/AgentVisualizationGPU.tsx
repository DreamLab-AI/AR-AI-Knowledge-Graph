import React, { useRef, useEffect, useState, useMemo, useCallback } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { Html, Text, Billboard, Sphere, Box, Cone, Cylinder, Torus } from '@react-three/drei';
import { EffectComposer, Bloom, ChromaticAberration, Vignette } from '@react-three/postprocessing';
import { BlendFunction } from 'postprocessing';
import { createLogger } from '../../../utils/logger';
import { useBotsWebSocketIntegration } from '../hooks/useBotsWebSocketIntegration';

const logger = createLogger('AgentVisualizationGPU');

// GPU Spring Solver Shader
const springShaderMaterial = {
  vertexShader: `
    uniform float time;
    uniform float springStrength;
    uniform float damping;
    uniform float nodeSize;
    
    attribute vec3 velocity;
    attribute vec3 targetPosition;
    attribute float agentType;
    attribute float health;
    attribute float activity;
    
    varying vec3 vColor;
    varying float vHealth;
    varying float vActivity;
    varying float vAgentType;
    
    // Agent type colors
    vec3 getAgentColor(float type) {
      if (type < 0.1) return vec3(0.2, 0.8, 1.0);      // coordinator - cyan
      else if (type < 0.2) return vec3(0.4, 1.0, 0.4); // coder - green
      else if (type < 0.3) return vec3(1.0, 0.6, 0.2); // architect - orange
      else if (type < 0.4) return vec3(0.8, 0.2, 0.8); // analyst - purple
      else if (type < 0.5) return vec3(1.0, 0.3, 0.3); // tester - red
      else if (type < 0.6) return vec3(1.0, 1.0, 0.3); // researcher - yellow
      else if (type < 0.7) return vec3(0.3, 0.6, 1.0); // reviewer - blue
      else if (type < 0.8) return vec3(0.5, 1.0, 0.8); // optimizer - mint
      else if (type < 0.9) return vec3(1.0, 0.5, 0.8); // documenter - pink
      else return vec3(0.7, 0.7, 0.7);                 // specialist - grey
    }
    
    void main() {
      // Spring physics calculation
      vec3 springForce = (targetPosition - position) * springStrength;
      vec3 dampingForce = -velocity * damping;
      vec3 totalForce = springForce + dampingForce;
      
      // Update position with spring physics
      vec3 newPosition = position + velocity * 0.016 + totalForce * 0.00026;
      
      // Add activity-based oscillation
      float activityPulse = sin(time * 3.0 + position.x * 0.5) * activity * 0.2;
      newPosition.y += activityPulse;
      
      // Size based on health and activity
      float size = nodeSize * (0.8 + health * 0.4 + activity * 0.3);
      
      vec4 mvPosition = modelViewMatrix * vec4(newPosition, 1.0);
      gl_Position = projectionMatrix * mvPosition;
      gl_PointSize = size * (300.0 / -mvPosition.z);
      
      // Pass data to fragment shader
      vColor = getAgentColor(agentType);
      vHealth = health;
      vActivity = activity;
      vAgentType = agentType;
    }
  `,
  
  fragmentShader: `
    uniform float time;
    uniform sampler2D agentTexture;
    
    varying vec3 vColor;
    varying float vHealth;
    varying float vActivity;
    varying float vAgentType;
    
    void main() {
      // Create circular point with glow
      vec2 uv = gl_PointCoord.xy;
      float dist = length(uv - 0.5);
      
      if (dist > 0.5) discard;
      
      // Core with health-based intensity
      float core = 1.0 - smoothstep(0.0, 0.3, dist);
      
      // Glow effect
      float glow = 1.0 - smoothstep(0.2, 0.5, dist);
      glow = pow(glow, 2.0) * vActivity;
      
      // Pulse effect
      float pulse = sin(time * 2.0 + vAgentType * 6.28) * 0.1 + 0.9;
      
      // Combine effects
      vec3 finalColor = vColor * core * pulse + vColor * glow * 0.5;
      
      // Health indicator ring
      float ring = smoothstep(0.35, 0.4, dist) - smoothstep(0.45, 0.5, dist);
      vec3 healthColor = mix(vec3(1.0, 0.2, 0.2), vec3(0.2, 1.0, 0.2), vHealth);
      finalColor += healthColor * ring * 0.8;
      
      float alpha = core + glow * 0.5;
      gl_FragColor = vec4(finalColor, alpha);
    }
  `
};

// Agent shape based on type and status
const AgentShape: React.FC<{ agent: any, size: number }> = ({ agent, size }) => {
  const meshRef = useRef<THREE.Mesh>(null);
  
  // Determine shape based on agent type
  const getShape = () => {
    switch (agent.type) {
      case 'coordinator':
        return <Sphere args={[size, 32, 16]} />;
      case 'coder':
        return <Box args={[size * 1.5, size * 1.5, size * 1.5]} />;
      case 'architect':
        return <Cone args={[size * 1.2, size * 2, 8]} />;
      case 'analyst':
        return <Cylinder args={[size * 0.8, size * 0.8, size * 2, 16]} />;
      case 'tester':
        return <Torus args={[size, size * 0.4, 16, 32]} />;
      default:
        return <Sphere args={[size, 16, 16]} />;
    }
  };
  
  // Rotation based on activity
  useFrame((state, delta) => {
    if (meshRef.current) {
      meshRef.current.rotation.x += delta * agent.activity * 0.5;
      meshRef.current.rotation.y += delta * agent.activity * 0.3;
    }
  });
  
  return (
    <mesh ref={meshRef}>
      {getShape()}
      <meshStandardMaterial
        color={agent.color}
        emissive={agent.color}
        emissiveIntensity={0.5 + agent.activity * 0.5}
        metalness={0.7}
        roughness={0.3}
        transparent
        opacity={0.8 + agent.health * 0.2}
      />
    </mesh>
  );
};

// Rich metadata display
const AgentMetadata: React.FC<{ agent: any, position: THREE.Vector3 }> = ({ agent, position }) => {
  const [expanded, setExpanded] = useState(false);
  
  return (
    <Billboard position={[position.x, position.y + 2, position.z]}>
      <Html
        center
        occlude
        style={{
          transition: 'all 0.3s ease',
          transform: expanded ? 'scale(1.2)' : 'scale(1)',
        }}
      >
        <div
          onClick={() => setExpanded(!expanded)}
          style={{
            background: 'rgba(0, 0, 0, 0.85)',
            border: `2px solid ${agent.color}`,
            borderRadius: '8px',
            padding: expanded ? '12px' : '8px',
            color: '#fff',
            fontFamily: 'monospace',
            fontSize: '12px',
            cursor: 'pointer',
            minWidth: '120px',
            boxShadow: `0 0 20px ${agent.color}40`,
            backdropFilter: 'blur(10px)',
          }}
        >
          <div style={{ fontWeight: 'bold', marginBottom: '4px' }}>
            {agent.name || `${agent.type}-${agent.id.slice(0, 6)}`}
          </div>
          <div style={{ color: agent.healthColor, fontSize: '10px' }}>
            ❤️ {Math.round(agent.health * 100)}% | ⚡ {Math.round(agent.cpuUsage)}%
          </div>
          {expanded && (
            <>
              <hr style={{ margin: '8px 0', opacity: 0.3 }} />
              <div style={{ fontSize: '10px', lineHeight: '14px' }}>
                <div>Status: <span style={{ color: agent.statusColor }}>{agent.status}</span></div>
                <div>Tasks: {agent.activeTasks || 0} active</div>
                <div>Memory: {Math.round(agent.memoryUsage || 0)}%</div>
                <div>Tokens: {agent.tokenUsage || 0}</div>
                <div>Age: {Math.round(agent.age / 60000)}m</div>
                {agent.currentTask && (
                  <div style={{ marginTop: '4px', fontSize: '9px', opacity: 0.8 }}>
                    📋 {agent.currentTask}
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </Html>
    </Billboard>
  );
};

// Connection visualization with data flow
const DataFlowConnection: React.FC<{ 
  start: THREE.Vector3, 
  end: THREE.Vector3, 
  strength: number,
  dataVolume: number 
}> = ({ start, end, strength, dataVolume }) => {
  const particlesRef = useRef<THREE.Points>(null);
  const lineRef = useRef<THREE.Line>(null);
  
  // Animated particles showing data flow
  useFrame((state) => {
    if (particlesRef.current) {
      const positions = particlesRef.current.geometry.attributes.position;
      const time = state.clock.elapsedTime;
      
      for (let i = 0; i < 20; i++) {
        const t = ((time * strength * 0.3 + i * 0.05) % 1);
        const x = start.x + (end.x - start.x) * t;
        const y = start.y + (end.y - start.y) * t;
        const z = start.z + (end.z - start.z) * t;
        
        positions.setXYZ(i, x, y, z);
      }
      positions.needsUpdate = true;
    }
  });
  
  // Connection color based on data volume
  const connectionColor = useMemo(() => {
    const normalized = Math.min(dataVolume / 1000, 1);
    return new THREE.Color().setHSL(0.5 - normalized * 0.5, 0.8, 0.5);
  }, [dataVolume]);
  
  return (
    <group>
      {/* Connection line */}
      <line ref={lineRef}>
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={2}
            array={new Float32Array([start.x, start.y, start.z, end.x, end.y, end.z])}
            itemSize={3}
          />
        </bufferGeometry>
        <lineBasicMaterial color={connectionColor} opacity={0.3} transparent linewidth={2} />
      </line>
      
      {/* Data flow particles */}
      <points ref={particlesRef}>
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={20}
            array={new Float32Array(60)}
            itemSize={3}
          />
        </bufferGeometry>
        <pointsMaterial
          size={0.3}
          color={connectionColor}
          transparent
          opacity={0.8}
          sizeAttenuation
          blending={THREE.AdditiveBlending}
        />
      </points>
    </group>
  );
};

// Main GPU-accelerated agent visualization
export const AgentVisualizationGPU: React.FC = () => {
  const { gl } = useThree();
  const [agents, setAgents] = useState<any[]>([]);
  const [connections, setConnections] = useState<any[]>([]);
  const [error, setError] = useState<string | null>(null);
  const positionsRef = useRef<Map<string, THREE.Vector3>>(new Map());
  const velocitiesRef = useRef<Map<string, THREE.Vector3>>(new Map());
  const pointsRef = useRef<THREE.Points>(null);
  
  // Get real-time data from MCP
  const { 
    isConnected, 
    connectionState, 
    lastUpdate,
    requestAgentList,
    requestSwarmStatus,
    mcpData
  } = useBotsWebSocketIntegration();
  
  // Request data periodically
  useEffect(() => {
    if (isConnected) {
      const interval = setInterval(() => {
        requestAgentList();
        requestSwarmStatus();
      }, 2000);
      
      return () => clearInterval(interval);
    }
  }, [isConnected, requestAgentList, requestSwarmStatus]);
  
  // Process MCP data into visualization format
  useEffect(() => {
    if (mcpData?.agents) {
      const processedAgents = mcpData.agents.map((agent: any, index: number) => {
        const typeIndex = index / mcpData.agents.length;
        const color = new THREE.Color().setHSL(typeIndex, 0.7, 0.5);
        
        return {
          ...agent,
          color: `#${color.getHexString()}`,
          healthColor: agent.health > 80 ? '#00ff00' : agent.health > 50 ? '#ffff00' : '#ff0000',
          statusColor: agent.status === 'busy' ? '#00ff00' : agent.status === 'error' ? '#ff0000' : '#ffff00',
          activity: agent.cpuUsage / 100,
          size: 1 + (agent.workload || 0) * 2,
          tokenUsage: mcpData.tokenUsage?.byAgent[agent.type] || 0
        };
      });
      
      setAgents(processedAgents);
    }
    
    if (mcpData?.edges) {
      setConnections(mcpData.edges);
    }
  }, [mcpData]);
  
  // Initialize positions for new agents
  useEffect(() => {
    agents.forEach((agent, index) => {
      if (!positionsRef.current.has(agent.id)) {
        const angle = (index / agents.length) * Math.PI * 2;
        const radius = 15 + Math.random() * 10;
        const height = (Math.random() - 0.5) * 20;
        
        positionsRef.current.set(agent.id, new THREE.Vector3(
          Math.cos(angle) * radius,
          height,
          Math.sin(angle) * radius
        ));
        
        velocitiesRef.current.set(agent.id, new THREE.Vector3());
      }
    });
  }, [agents]);
  
  // GPU Spring physics simulation
  useFrame((state, delta) => {
    if (agents.length === 0) return;
    
    const dt = Math.min(delta, 0.016);
    const time = state.clock.elapsedTime;
    
    // Update spring physics
    agents.forEach((agent) => {
      const pos = positionsRef.current.get(agent.id);
      const vel = velocitiesRef.current.get(agent.id);
      
      if (!pos || !vel) return;
      
      // Spring forces between connected agents
      let force = new THREE.Vector3();
      
      connections.forEach(conn => {
        if (conn.source === agent.id || conn.target === agent.id) {
          const otherId = conn.source === agent.id ? conn.target : conn.source;
          const otherPos = positionsRef.current.get(otherId);
          
          if (otherPos) {
            const diff = new THREE.Vector3().subVectors(otherPos, pos);
            const dist = diff.length();
            const idealDist = 20;
            
            if (dist > 0) {
              const springForce = diff.normalize().multiplyScalar(
                (dist - idealDist) * 0.1 * conn.strength
              );
              force.add(springForce);
            }
          }
        }
      });
      
      // Repulsion between all agents
      agents.forEach(other => {
        if (other.id !== agent.id) {
          const otherPos = positionsRef.current.get(other.id);
          if (otherPos) {
            const diff = new THREE.Vector3().subVectors(pos, otherPos);
            const dist = diff.length();
            
            if (dist > 0 && dist < 30) {
              const repulsion = diff.normalize().multiplyScalar(100 / (dist * dist));
              force.add(repulsion);
            }
          }
        }
      });
      
      // Center gravity
      force.add(pos.clone().multiplyScalar(-0.01));
      
      // Apply forces
      vel.add(force.multiplyScalar(dt));
      vel.multiplyScalar(0.95); // Damping
      pos.add(vel.clone().multiplyScalar(dt));
      
      // Activity-based motion
      const activityMotion = new THREE.Vector3(
        Math.sin(time * agent.activity * 2) * 0.1,
        Math.cos(time * agent.activity * 3) * 0.1,
        Math.sin(time * agent.activity * 2.5) * 0.1
      );
      pos.add(activityMotion);
    });
    
    // Update GPU buffer if using points
    if (pointsRef.current && agents.length > 0) {
      const positions = pointsRef.current.geometry.attributes.position;
      const velocities = pointsRef.current.geometry.attributes.velocity;
      const healths = pointsRef.current.geometry.attributes.health;
      const activities = pointsRef.current.geometry.attributes.activity;
      
      agents.forEach((agent, i) => {
        const pos = positionsRef.current.get(agent.id);
        const vel = velocitiesRef.current.get(agent.id);
        
        if (pos && vel && i < positions.count) {
          positions.setXYZ(i, pos.x, pos.y, pos.z);
          velocities.setXYZ(i, vel.x, vel.y, vel.z);
          healths.setX(i, agent.health / 100);
          activities.setX(i, agent.activity);
        }
      });
      
      positions.needsUpdate = true;
      velocities.needsUpdate = true;
      healths.needsUpdate = true;
      activities.needsUpdate = true;
    }
  });
  
  // Create unified shader material
  const shaderMaterial = useMemo(() => {
    return new THREE.ShaderMaterial({
      uniforms: {
        time: { value: 0 },
        springStrength: { value: 0.02 },
        damping: { value: 0.95 },
        nodeSize: { value: 2.0 },
        agentTexture: { value: null }
      },
      vertexShader: springShaderMaterial.vertexShader,
      fragmentShader: springShaderMaterial.fragmentShader,
      transparent: true,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    });
  }, []);
  
  // Update shader uniforms
  useFrame((state) => {
    if (shaderMaterial) {
      shaderMaterial.uniforms.time.value = state.clock.elapsedTime;
    }
  });
  
  if (error) {
    return (
      <Html center>
        <div style={{ color: '#ff0000', background: 'rgba(0,0,0,0.8)', padding: '20px' }}>
          Error: {error}
        </div>
      </Html>
    );
  }
  
  return (
    <group>
      {/* Connection status indicator */}
      <Html position={[0, 30, 0]} center>
        <div style={{
          background: 'rgba(0, 0, 0, 0.9)',
          border: `2px solid ${isConnected ? '#00ff00' : '#ff0000'}`,
          borderRadius: '8px',
          padding: '10px',
          color: '#fff',
          fontFamily: 'monospace',
          fontSize: '14px',
        }}>
          <div>MCP: {isConnected ? '🟢 Connected' : '🔴 Disconnected'}</div>
          <div style={{ fontSize: '12px', opacity: 0.8 }}>
            Agents: {agents.length} | Connections: {connections.length}
          </div>
          {lastUpdate && (
            <div style={{ fontSize: '10px', opacity: 0.6 }}>
              Last update: {new Date(lastUpdate).toLocaleTimeString()}
            </div>
          )}
        </div>
      </Html>
      
      {/* GPU-accelerated points for large agent counts */}
      {agents.length > 50 ? (
        <points ref={pointsRef}>
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              count={agents.length}
              array={new Float32Array(agents.length * 3)}
              itemSize={3}
            />
            <bufferAttribute
              attach="attributes-velocity"
              count={agents.length}
              array={new Float32Array(agents.length * 3)}
              itemSize={3}
            />
            <bufferAttribute
              attach="attributes-agentType"
              count={agents.length}
              array={new Float32Array(agents.map((_, i) => i / agents.length))}
              itemSize={1}
            />
            <bufferAttribute
              attach="attributes-health"
              count={agents.length}
              array={new Float32Array(agents.map(a => a.health / 100))}
              itemSize={1}
            />
            <bufferAttribute
              attach="attributes-activity"
              count={agents.length}
              array={new Float32Array(agents.map(a => a.activity))}
              itemSize={1}
            />
          </bufferGeometry>
          <primitive object={shaderMaterial} attach="material" />
        </points>
      ) : (
        /* Individual agent meshes for smaller counts */
        agents.map((agent) => {
          const position = positionsRef.current.get(agent.id) || new THREE.Vector3();
          
          return (
            <group key={agent.id} position={position}>
              <AgentShape agent={agent} size={agent.size} />
              <AgentMetadata agent={agent} position={new THREE.Vector3()} />
            </group>
          );
        })
      )}
      
      {/* Render connections */}
      {connections.map((conn) => {
        const start = positionsRef.current.get(conn.source);
        const end = positionsRef.current.get(conn.target);
        
        if (start && end) {
          return (
            <DataFlowConnection
              key={conn.id}
              start={start}
              end={end}
              strength={conn.messageCount / 100}
              dataVolume={conn.dataVolume}
            />
          );
        }
        return null;
      })}
      
      {/* Ambient environment particles */}
      <points>
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            count={500}
            array={new Float32Array(1500).map(() => (Math.random() - 0.5) * 100)}
            itemSize={3}
          />
        </bufferGeometry>
        <pointsMaterial
          size={0.05}
          color="#446688"
          transparent
          opacity={0.2}
          sizeAttenuation
        />
      </points>
      
      {/* Post-processing effects */}
      <EffectComposer>
        <Bloom
          intensity={0.5}
          luminanceThreshold={0.8}
          luminanceSmoothing={0.9}
          blendFunction={BlendFunction.SCREEN}
        />
        <ChromaticAberration
          offset={[0.0005, 0.0005]}
          blendFunction={BlendFunction.NORMAL}
        />
        <Vignette
          darkness={0.3}
          offset={0.5}
          blendFunction={BlendFunction.NORMAL}
        />
      </EffectComposer>
    </group>
  );
};