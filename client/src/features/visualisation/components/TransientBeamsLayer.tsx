/**
 * TransientBeamsLayer
 *
 * Embodied render of agent actions (0x23 AGENT_ACTION). Each beam is a coloured
 * cylinder drawn from the acting agent node to its target KG node. Opacity
 * animates fade-in → hold → fade-out over the action's durationMs; the beam is
 * removed once its TTL elapses (the store prunes; this layer triggers the prune
 * every frame).
 *
 * Colour AND shape by action_type, both sourced from semanticEncoding (which
 * re-exports the decoder's AGENT_ACTION_COLORS):
 *   QUERY=blue UPDATE=yellow CREATE=green DELETE=red LINK=purple TRANSFORM=cyan
 * Shape embodies the verb: Create widens into the node, Delete narrows into it,
 * Query is a thin probe, Link a thick tie, Transform a rounder beam.
 *
 * Position resolution is injected via props so the layer stays decoupled from
 * graph internals. The caller (GraphManager) owns the agent registry and the
 * live node-position buffer and supplies two resolvers. A resolver returns
 * false when the id has no known world position — that beam is skipped
 * silently (never throws), exactly as the brief requires.
 *
 * Rendering follows the repo idioms: a stable per-beam cylinder mesh,
 * geometry/material built once and updated in-place inside useFrame with zero
 * per-frame allocations (cf. GlassEdges' matrix composition).
 */

import React, { useMemo, useRef, useEffect, useCallback } from 'react';
import * as THREE from 'three';
import { useFrame } from '@react-three/fiber';
import { agentActionColorHex, agentActionShape } from '../semanticEncoding';
import { useTransientBeams } from '../hooks/useTransientBeams';
import type { TransientBeam } from '@/store/transientBeamStore';

/**
 * Resolve a wire id to a world position. Writes into `out` and returns true on
 * success; returns false (leaving `out` untouched) when the id is unresolvable.
 */
export type BeamPositionResolver = (id: number, out: THREE.Vector3) => boolean;

interface TransientBeamsLayerProps {
  /** Resolve source_agent_id → agent node world position. */
  resolveAgentPosition: BeamPositionResolver;
  /** Resolve target_node_id → KG node world position. */
  resolveNodePosition: BeamPositionResolver;
  /** Cylinder radius in world units. */
  beamRadius?: number;
  /** Peak opacity during the hold phase. */
  maxOpacity?: number;
}

/** Fraction of lifetime spent fading in, and fading out (rest is hold). */
const FADE_IN_FRAC = 0.18;
const FADE_OUT_FRAC = 0.32;

/** A 0..1 opacity envelope: ramp up, hold at 1, ramp down. */
function opacityEnvelope(progress: number): number {
  if (progress <= 0) return 0;
  if (progress >= 1) return 0;
  if (progress < FADE_IN_FRAC) {
    return progress / FADE_IN_FRAC;
  }
  const fadeOutStart = 1 - FADE_OUT_FRAC;
  if (progress > fadeOutStart) {
    return (1 - progress) / FADE_OUT_FRAC;
  }
  return 1;
}

export const TransientBeamsLayer: React.FC<TransientBeamsLayerProps> = ({
  resolveAgentPosition,
  resolveNodePosition,
  beamRadius = 0.35,
  maxOpacity = 0.85,
}) => {
  const { beams, prune } = useTransientBeams();

  // Prune expired beams every frame, regardless of how many are alive.
  useFrame(() => {
    prune();
  });

  if (beams.length === 0) return null;

  return (
    <group name="transient-beams-layer">
      {beams.map(beam => (
        <TransientBeamMesh
          key={beam.id}
          beam={beam}
          resolveAgentPosition={resolveAgentPosition}
          resolveNodePosition={resolveNodePosition}
          beamRadius={beamRadius}
          maxOpacity={maxOpacity}
        />
      ))}
    </group>
  );
};

/** Pre-allocated temps shared across all beam instances (single render thread). */
const _src = new THREE.Vector3();
const _tgt = new THREE.Vector3();
const _mid = new THREE.Vector3();
const _dir = new THREE.Vector3();
const _up = new THREE.Vector3(0, 1, 0);
const _quat = new THREE.Quaternion();
const _scale = new THREE.Vector3();
const _color = new THREE.Color();

/** Single animated beam: a unit-Y cylinder stretched between two world points. */
const TransientBeamMesh: React.FC<{
  beam: TransientBeam;
  resolveAgentPosition: BeamPositionResolver;
  resolveNodePosition: BeamPositionResolver;
  beamRadius: number;
  maxOpacity: number;
}> = ({
  beam,
  resolveAgentPosition,
  resolveNodePosition,
  beamRadius,
  maxOpacity,
}) => {
  const meshRef = useRef<THREE.Mesh>(null);

  const colorHex = useMemo(() => {
    _color.set(agentActionColorHex(beam.actionType));
    return _color.getHex();
  }, [beam.actionType]);

  // Unit-Y cylinder built once; instance is stretched/positioned per frame. The
  // per-action shape embodies the verb — Create widens into the node (+Y/top),
  // Delete narrows into it, Query is a thin probe, Link a thick tie, etc.
  const geometry = useMemo(() => {
    const shape = agentActionShape(beam.actionType);
    return new THREE.CylinderGeometry(
      beamRadius * shape.radiusTop,
      beamRadius * shape.radiusBottom,
      1,
      shape.radialSegments,
      1,
      true,
    );
  }, [beamRadius, beam.actionType]);

  const material = useMemo(() => {
    const mat = new THREE.MeshBasicMaterial({
      color: colorHex,
      transparent: true,
      opacity: 0,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
      side: THREE.DoubleSide,
      toneMapped: false,
    });
    return mat;
  }, [colorHex]);

  useEffect(() => {
    return () => {
      geometry.dispose();
      material.dispose();
    };
  }, [geometry, material]);

  const updateBeam = useCallback(() => {
    const mesh = meshRef.current;
    if (!mesh) return;

    // ID-space resolution: agent (source) and KG node (target). Either failing
    // hides the beam this frame without throwing.
    const haveSrc = resolveAgentPosition(beam.sourceAgentId, _src);
    const haveTgt = resolveNodePosition(beam.targetNodeId, _tgt);
    if (!haveSrc || !haveTgt) {
      mesh.visible = false;
      return;
    }

    _dir.subVectors(_tgt, _src);
    const length = _dir.length();
    if (length < 1e-4) {
      mesh.visible = false;
      return;
    }

    const now = performance.now();
    const progress = (now - beam.startTime) / beam.durationMs;
    const opacity = opacityEnvelope(progress) * maxOpacity;
    if (opacity <= 0) {
      mesh.visible = false;
      return;
    }

    mesh.visible = true;
    (mesh.material as THREE.MeshBasicMaterial).opacity = opacity;

    // Compose: translate to midpoint, rotate unit-Y to the beam direction,
    // scale Y to the full source→target distance.
    _dir.normalize();
    _mid.addVectors(_src, _tgt).multiplyScalar(0.5);
    const dot = _up.dot(_dir);
    if (dot < -0.9999) {
      _quat.set(1, 0, 0, 0);
    } else {
      _quat.setFromUnitVectors(_up, _dir);
    }
    _scale.set(1, length, 1);
    mesh.position.copy(_mid);
    mesh.quaternion.copy(_quat);
    mesh.scale.copy(_scale);
  }, [
    beam.sourceAgentId,
    beam.targetNodeId,
    beam.startTime,
    beam.durationMs,
    maxOpacity,
    resolveAgentPosition,
    resolveNodePosition,
  ]);

  useFrame(updateBeam);

  return (
    <mesh
      ref={meshRef}
      geometry={geometry}
      material={material}
      visible={false}
      frustumCulled={false}
    />
  );
};

export default TransientBeamsLayer;
