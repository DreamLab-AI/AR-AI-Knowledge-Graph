/**
 * EchoPulseLayer — the "Echo Pulse" signature flourish.
 *
 * An in-scene R3F layer that reacts to DOM setting commits (via
 * echoPulseBus) with an expanding emissive ring washing through the graph.
 * Mounted once inside the shared R3F scene root (see GraphCanvas.tsx).
 *
 * Perf contract (design-spec.md §4.4, §8 WP4 row):
 *   - Feature-flag off (`echoPulseEnabled === false`) -> render null, zero
 *     scene nodes, zero subscriptions.
 *   - Idle (flag on, no active pulses) -> pool meshes are `visible={false}`,
 *     so THREE issues zero draw calls; the only per-frame cost is a 3-slot
 *     boolean scan in tickPulsePool.
 *   - Active -> at most ECHO_PULSE_MAX_CONCURRENT (3) draw calls, one
 *     RingGeometry mesh per pulse, no per-node work, no React state writes
 *     per frame (all mutation goes through refs / THREE objects directly).
 *
 * Rings billboard to the camera every frame (mirrors the atmosphere-plane
 * billboarding pattern in WasmSceneEffects) so the wash reads correctly from
 * any orbit angle despite RingGeometry being planar.
 */

import React, { useEffect, useMemo, useRef } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { subscribeEchoPulse, type EchoPulseDetail } from './echoPulseBus';
import { useControlCenterUI } from '../state/useControlCenterUI';
import {
  createPulsePoolState,
  spawnPulse,
  tickPulsePool,
  pulseProgress,
  easeOutCubic,
  ECHO_PULSE_DURATION_S,
  ECHO_PULSE_MAX_RADIUS,
  type PulsePoolState,
} from './echoPulsePool';

/** How far in front of the camera 'camera-center' resolves to (world units). */
const CAMERA_FORWARD_DISTANCE = 600;
/** Ring is a thin annulus in normalised local space; scaled to world radius per-frame. */
const RING_INNER_RADIUS = 0.86;
const RING_OUTER_RADIUS = 1;
const RING_SEGMENTS = 48;
/** Peak opacity band — strength (0..1) maps into this range. */
const MIN_PEAK_OPACITY = 0.15;
const MAX_PEAK_OPACITY = 0.85;
const RING_SATURATION = 0.85;
const RING_LIGHTNESS = 0.6;

// Pre-allocated temp objects — avoids per-frame GC (mirrors WasmSceneEffects convention).
const _tmpDir = new THREE.Vector3();
const _tmpOrigin = new THREE.Vector3();
const _tmpColor = new THREE.Color();
const _tmpScale = new THREE.Vector3();

function resolveOriginInto(
  target: THREE.Vector3,
  origin: EchoPulseDetail['origin'],
  camera: THREE.Camera,
): THREE.Vector3 {
  if (origin === 'camera-center') {
    camera.getWorldDirection(_tmpDir);
    return target.copy(camera.position).addScaledVector(_tmpDir, CAMERA_FORWARD_DISTANCE);
  }
  return target.set(origin[0], origin[1], origin[2]);
}

const EchoPulseLayer: React.FC = () => {
  const echoPulseEnabled = useControlCenterUI((s) => s.echoPulseEnabled);
  const { camera } = useThree();

  const poolRef = useRef<PulsePoolState>(createPulsePoolState());
  const meshRefs = useRef<Array<THREE.Mesh | null>>([]);
  // Sampled once per useFrame tick; the bus callback fires outside the R3F
  // frame loop, so pulses spawn against the last-known R3F clock value
  // rather than a mismatched performance.now() basis.
  const clockRef = useRef(0);
  const wasActiveRef = useRef(false);

  const geometry = useMemo(
    () => new THREE.RingGeometry(RING_INNER_RADIUS, RING_OUTER_RADIUS, RING_SEGMENTS),
    [],
  );
  const materials = useMemo(
    () =>
      poolRef.current.slots.map(
        () =>
          new THREE.MeshBasicMaterial({
            transparent: true,
            opacity: 0,
            depthWrite: false,
            depthTest: true,
            blending: THREE.AdditiveBlending,
            side: THREE.DoubleSide,
            toneMapped: false,
          }),
      ),
    [],
  );

  useEffect(
    () => () => {
      geometry.dispose();
      materials.forEach((m) => m.dispose());
    },
    [geometry, materials],
  );

  useEffect(() => {
    if (!echoPulseEnabled) return;
    return subscribeEchoPulse((detail) => {
      spawnPulse(poolRef.current, detail, (o) => resolveOriginInto(_tmpOrigin, o, camera), clockRef.current);
    });
  }, [echoPulseEnabled, camera]);

  useFrame((state) => {
    clockRef.current = state.clock.elapsedTime;
    const pool = poolRef.current;
    const anyActive = tickPulsePool(pool, clockRef.current, ECHO_PULSE_DURATION_S);

    // Idle: nothing to animate this tick. Meshes were already hidden the
    // frame their pulse retired, so there is nothing further to do.
    if (!anyActive && !wasActiveRef.current) return;
    wasActiveRef.current = anyActive;

    for (let i = 0; i < pool.slots.length; i++) {
      const slot = pool.slots[i];
      const mesh = meshRefs.current[i];
      if (!mesh) continue;

      if (!slot.active) {
        mesh.visible = false;
        continue;
      }

      const t = pulseProgress(slot, clockRef.current, ECHO_PULSE_DURATION_S);
      const eased = easeOutCubic(t);
      const radius = Math.max(0.001, ECHO_PULSE_MAX_RADIUS * eased);
      const peakOpacity = MIN_PEAK_OPACITY + (MAX_PEAK_OPACITY - MIN_PEAK_OPACITY) * slot.strength;
      const opacity = peakOpacity * (1 - t);

      mesh.visible = true;
      mesh.position.copy(slot.origin);
      mesh.quaternion.copy(camera.quaternion);
      _tmpScale.set(radius, radius, radius);
      mesh.scale.copy(_tmpScale);

      const mat = materials[i];
      mat.opacity = opacity;
      _tmpColor.setHSL(slot.hue, RING_SATURATION, RING_LIGHTNESS);
      mat.color.copy(_tmpColor);
    }
  });

  if (!echoPulseEnabled) return null;

  return (
    <group name="echo-pulse-layer">
      {materials.map((material, i) => (
        <mesh
          key={i}
          ref={(m) => {
            meshRefs.current[i] = m;
          }}
          geometry={geometry}
          material={material}
          visible={false}
          frustumCulled={false}
          renderOrder={10}
        />
      ))}
    </group>
  );
};

export default EchoPulseLayer;
