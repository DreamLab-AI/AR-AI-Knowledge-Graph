/**
 * ActionConnectionsLayer
 *
 * Renders ephemeral animated connections between agent nodes and data nodes
 * showing real-time agent-to-data interactions.
 *
 * Visual Design:
 * - Bezier curve from agent to target
 * - Animated particle traveling along the path
 * - Color coded by action type (query=blue, update=yellow, create=green, delete=red, link=purple, transform=cyan)
 * - Impact burst effect at target
 *
 * Performance:
 * - VR mode uses simplified geometry for Quest 3 @ 72fps
 * - LOD: Reduces detail at distance
 * - Max 50 concurrent connections
 */

import React, { useMemo, useRef, useEffect } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { ActionConnection } from '../hooks/useActionConnections';
import { AgentActionType } from '@/services/BinaryWebSocketProtocol';

/** Pre-allocated temp objects for per-frame operations -- avoids GC churn. */
const _connColor = new THREE.Color();

interface ActionConnectionsLayerProps {
  connections: ActionConnection[];
  /** Enable VR-optimized rendering */
  vrMode?: boolean;
  /** Global opacity multiplier */
  opacity?: number;
  /** Line width for connections */
  lineWidth?: number;
}

/**
 * Phase timing boundaries (cumulative percentages of total 500ms duration)
 * - spawn:  0.0-0.2 (100ms) - Line fades in, particle grows at source
 * - travel: 0.2-0.8 (300ms) - Particle travels along bezier curve
 * - impact: 0.8-1.0 (100ms) - Burst effect at target, then fade out
 *
 * Note: Original spec had separate impact (50ms) and fade (50ms),
 * combined here for smoother visual transition.
 */
const PHASE_BOUNDS = {
  spawnEnd: 0.2,    // 100ms / 500ms = 0.2
  travelEnd: 0.8,   // 300ms / 500ms = 0.6, cumulative = 0.8
  impactEnd: 1.0,   // Combined impact+fade = 100ms, cumulative = 1.0
  fadeEnd: 1.0,     // Kept for compatibility
};

export const ActionConnectionsLayer: React.FC<ActionConnectionsLayerProps> = ({
  connections,
  vrMode = false,
  opacity = 1.0,
  lineWidth = 2,
}) => {
  if (connections.length === 0) return null;

  return (
    <group name="action-connections-layer">
      {connections.map((conn) => (
        <ActionConnectionLine
          key={conn.id}
          connection={conn}
          vrMode={vrMode}
          opacity={opacity}
          lineWidth={lineWidth}
        />
      ))}
    </group>
  );
};

/** Single animated action connection */
const ActionConnectionLine: React.FC<{
  connection: ActionConnection;
  vrMode: boolean;
  opacity: number;
  lineWidth: number;
}> = ({ connection, vrMode, opacity, lineWidth }) => {
  const lineRef = useRef<THREE.Line>(null);
  const particleRef = useRef<THREE.Mesh>(null);
  const glowRef = useRef<THREE.Mesh>(null);
  const impactRef = useRef<THREE.Mesh>(null);

  // Number of curve sample points -- stable across lifetime
  const numPoints = vrMode ? 21 : 41;

  // Pre-allocated temp vectors for per-frame bezier computation
  const _src = useRef(new THREE.Vector3());
  const _tgt = useRef(new THREE.Vector3());
  const _mid = useRef(new THREE.Vector3());
  const _dir = useRef(new THREE.Vector3());
  const _perp = useRef(new THREE.Vector3());
  const _curvePoint = useRef(new THREE.Vector3());
  const _up = useRef(new THREE.Vector3(0, 1, 0));
  const _curve = useRef(new THREE.QuadraticBezierCurve3(
    new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()
  ));

  // Create the line geometry ONCE and reuse it -- never recreated per frame
  const lineGeometry = useRef<THREE.BufferGeometry | null>(null);
  if (!lineGeometry.current) {
    const positions = new Float32Array(numPoints * 3);
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    lineGeometry.current = geom;
  }

  // Stable material -- created once
  const lineMaterial = useMemo(() => {
    _connColor.set(connection.color);
    return new THREE.LineBasicMaterial({
      color: _connColor.getHex(),
      transparent: true,
      opacity: opacity,
      linewidth: lineWidth,
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Stable line object -- created once
  const lineObject = useMemo(() => {
    return new THREE.Line(lineGeometry.current!, lineMaterial);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Dispose geometry and material on unmount
  useEffect(() => {
    const geom = lineGeometry.current;
    const mat = lineMaterial;
    return () => {
      geom?.dispose();
      mat.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Resolve source/target with deterministic fallback (no allocation)
  const resolveSource = (conn: ActionConnection, out: THREE.Vector3): THREE.Vector3 => {
    if (conn.sourcePosition) {
      return out.set(conn.sourcePosition.x, conn.sourcePosition.y, conn.sourcePosition.z);
    }
    const hash = conn.sourceAgentId * 1337;
    return out.set(Math.sin(hash) * 15, Math.cos(hash * 0.7) * 10, Math.sin(hash * 0.3) * 15);
  };

  const resolveTarget = (conn: ActionConnection, out: THREE.Vector3): THREE.Vector3 => {
    if (conn.targetPosition) {
      return out.set(conn.targetPosition.x, conn.targetPosition.y, conn.targetPosition.z);
    }
    const hash = conn.targetNodeId * 2749;
    return out.set(Math.sin(hash) * 20, Math.cos(hash * 0.5) * 15, Math.sin(hash * 0.8) * 20);
  };

  // All animation and geometry updates happen in useFrame -- zero allocations per frame
  useFrame(() => {
    const conn = connection;
    const src = resolveSource(conn, _src.current);
    const tgt = resolveTarget(conn, _tgt.current);

    // Recompute bezier control point
    const mid = _mid.current.addVectors(src, tgt).multiplyScalar(0.5);
    const distance = src.distanceTo(tgt);
    const dir = _dir.current.subVectors(tgt, src).normalize();
    const perp = _perp.current.crossVectors(dir, _up.current).normalize();
    const actionTypeIndex = conn._actionTypeEnum ??
      (['query', 'update', 'create', 'delete', 'link', 'transform'].indexOf(conn.actionType as string) || 0);
    const offsetAmount = distance * 0.3 * (1 + (actionTypeIndex * 0.1));
    mid.add(perp.multiplyScalar(offsetAmount));
    mid.y += distance * 0.15;

    // Update curve endpoints
    const curve = _curve.current;
    curve.v0.copy(src);
    curve.v1.copy(mid);
    curve.v2.copy(tgt);

    // Write bezier points directly into the existing buffer attribute
    const geom = lineGeometry.current!;
    const posAttr = geom.getAttribute('position') as THREE.BufferAttribute;
    const arr = posAttr.array as Float32Array;
    for (let i = 0; i < numPoints; i++) {
      const t = i / (numPoints - 1);
      curve.getPoint(t, _curvePoint.current);
      arr[i * 3] = _curvePoint.current.x;
      arr[i * 3 + 1] = _curvePoint.current.y;
      arr[i * 3 + 2] = _curvePoint.current.z;
    }
    posAttr.needsUpdate = true;

    // Compute visual properties
    const { progress, phase, color } = conn;
    let lineOpacity = 1.0;
    let particleScale = 1.0;
    let impactScale = 0;

    switch (phase) {
      case 'spawn':
        lineOpacity = progress / PHASE_BOUNDS.spawnEnd;
        particleScale = progress / PHASE_BOUNDS.spawnEnd;
        break;
      case 'travel':
        lineOpacity = 1.0;
        particleScale = 1.0;
        break;
      case 'impact':
      case 'fade': {
        const impactProgress = (progress - PHASE_BOUNDS.travelEnd) / (PHASE_BOUNDS.impactEnd - PHASE_BOUNDS.travelEnd);
        if (impactProgress < 0.5) {
          lineOpacity = 1.0;
          particleScale = 0.5;
          impactScale = impactProgress * 2;
        } else {
          const fadeProgress = (impactProgress - 0.5) * 2;
          lineOpacity = 1 - fadeProgress;
          particleScale = 0.5 * (1 - fadeProgress);
          impactScale = 1 - fadeProgress;
        }
        break;
      }
    }

    _connColor.set(color);
    const colorHex = _connColor.getHex();

    // Update line material
    if (lineRef.current) {
      const mat = lineRef.current.material as THREE.LineBasicMaterial;
      mat.opacity = lineOpacity * opacity;
      mat.color.setHex(colorHex);
    }

    // Compute particle position along curve
    let particleT = 0;
    if (phase === 'spawn') {
      particleT = 0;
    } else if (phase === 'travel') {
      particleT = (progress - PHASE_BOUNDS.spawnEnd) / (PHASE_BOUNDS.travelEnd - PHASE_BOUNDS.spawnEnd);
    } else {
      particleT = 1;
    }
    curve.getPoint(particleT, _curvePoint.current);

    if (particleRef.current) {
      particleRef.current.position.copy(_curvePoint.current);
      particleRef.current.scale.setScalar(particleScale * (vrMode ? 0.3 : 0.5));
    }
    if (glowRef.current) {
      glowRef.current.position.copy(_curvePoint.current);
      const glowMat = glowRef.current.material as THREE.MeshBasicMaterial;
      glowMat.opacity = 0.3 * opacity * particleScale;
    }
    if (impactRef.current) {
      impactRef.current.position.copy(tgt);
      impactRef.current.scale.setScalar(impactScale > 0 ? impactScale * 2 : 0.001);
      impactRef.current.visible = impactScale > 0;
      const impactMat = impactRef.current.material as THREE.MeshBasicMaterial;
      impactMat.opacity = impactScale * 0.5 * opacity;
    }
  });

  // Resolve initial color for declarative JSX props
  _connColor.set(connection.color);
  const initialColorHex = _connColor.getHex();

  return (
    <group>
      {/* Connection line -- stable object, geometry buffer updated in-place per frame */}
      <primitive ref={lineRef} object={lineObject} />

      {/* Traveling particle */}
      <mesh ref={particleRef}>
        <sphereGeometry args={[vrMode ? 0.2 : 0.4, 8, 6]} />
        <meshBasicMaterial
          color={initialColorHex}
          transparent
          opacity={0.9 * opacity}
        />
      </mesh>

      {/* Glow around particle */}
      {!vrMode && (
        <mesh ref={glowRef}>
          <sphereGeometry args={[0.8, 12, 12]} />
          <meshBasicMaterial
            color={initialColorHex}
            transparent
            opacity={0.3 * opacity}
            side={THREE.BackSide}
          />
        </mesh>
      )}

      {/* Impact burst at target */}
      <mesh ref={impactRef} visible={false}>
        <ringGeometry args={[0.5, 2, vrMode ? 16 : 32]} />
        <meshBasicMaterial
          color={initialColorHex}
          transparent
          opacity={0}
          side={THREE.DoubleSide}
        />
      </mesh>
    </group>
  );
};

/** Statistics component for debugging */
export const ActionConnectionsStats: React.FC<{
  connections: ActionConnection[];
}> = ({ connections }) => {
  const stats = useMemo(() => {
    const byType: Record<string, number> = {};
    for (const conn of connections) {
      // Support both new string type and legacy enum
      const typeName = typeof conn.actionType === 'string'
        ? conn.actionType
        : AgentActionType[conn._actionTypeEnum ?? conn.actionType] || 'Unknown';
      byType[typeName] = (byType[typeName] || 0) + 1;
    }
    return byType;
  }, [connections]);

  // This component lives INSIDE the R3F <Canvas> scene graph (it is rendered by
  // AgentActionVisualization alongside the 3D ActionConnectionsLayer). A raw
  // DOM <div> reconciled by react-three-fiber throws
  // "R3F: Div is not part of the THREE namespace!" and takes down the canvas
  // via CanvasErrorBoundary. drei's <Html fullscreen> portals the overlay out
  // of the WebGL scene into a screen-space container, so the debug panel is
  // valid inside the Canvas. `pointerEvents: none` keeps OrbitControls drivable
  // through the full-screen wrapper; the inner panel pins to the lower-left.
  return (
    <Html fullscreen zIndexRange={[100, 0]} style={{ pointerEvents: 'none' }}>
      <div style={{
        position: 'absolute',
        bottom: 10,
        left: 10,
        background: 'rgba(0,0,0,0.7)',
        color: 'white',
        padding: '8px 12px',
        borderRadius: 4,
        fontSize: 12,
        fontFamily: 'monospace',
      }}>
        <div>Active Actions: {connections.length}</div>
        {Object.entries(stats).map(([type, count]) => (
          <div key={type} style={{ opacity: 0.8 }}>
            {type}: {count}
          </div>
        ))}
      </div>
    </Html>
  );
};

export default ActionConnectionsLayer;
