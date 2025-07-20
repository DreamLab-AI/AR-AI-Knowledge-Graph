import React, { useRef, useMemo } from 'react';
import * as THREE from 'three';
import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';

// Test different THREE.js line rendering approaches
export const EdgeRenderingTest: React.FC = () => {
  // Test data: simple edge from (-2, 0, 0) to (2, 0, 0)
  const points = useMemo(() => {
    return new Float32Array([-2, 0, 0, 2, 0, 0]);
  }, []);

  // Create buffer geometry
  const geometry = useMemo(() => {
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.BufferAttribute(points, 3));
    return geo;
  }, [points]);

  // Different materials to test
  const materials = useMemo(() => ({
    basic: new THREE.LineBasicMaterial({
      color: 0xff0000,
      transparent: true,
      opacity: 0.8,
      linewidth: 1, // Note: linewidth doesn't work in WebGL
    }),
    dashed: new THREE.LineDashedMaterial({
      color: 0x00ff00,
      transparent: true,
      opacity: 0.8,
      dashSize: 0.1,
      gapSize: 0.1,
    }),
  }), []);

  return (
    <div style={{ width: '100%', height: '600px' }}>
      <Canvas camera={{ position: [0, 5, 5] }}>
        <ambientLight intensity={0.5} />
        <OrbitControls />
        
        {/* Test 1: LineSegments (current approach) */}
        <group position={[0, 2, 0]}>
          <lineSegments geometry={geometry} material={materials.basic} />
          <mesh position={[-2, 0, 0]}>
            <boxGeometry args={[0.2, 0.2, 0.2]} />
            <meshBasicMaterial color="white" />
          </mesh>
          <mesh position={[2, 0, 0]}>
            <boxGeometry args={[0.2, 0.2, 0.2]} />
            <meshBasicMaterial color="white" />
          </mesh>
        </group>

        {/* Test 2: Line (continuous line) */}
        <group position={[0, 0, 0]}>
          <line geometry={geometry} material={materials.basic} />
          <mesh position={[-2, 0, 0]}>
            <boxGeometry args={[0.2, 0.2, 0.2]} />
            <meshBasicMaterial color="blue" />
          </mesh>
          <mesh position={[2, 0, 0]}>
            <boxGeometry args={[0.2, 0.2, 0.2]} />
            <meshBasicMaterial color="blue" />
          </mesh>
        </group>

        {/* Test 3: Mesh-based line (cylinder) */}
        <group position={[0, -2, 0]}>
          <mesh rotation={[0, 0, Math.PI / 2]}>
            <cylinderGeometry args={[0.02, 0.02, 4, 8]} />
            <meshBasicMaterial color={0xff0000} transparent opacity={0.8} />
          </mesh>
          <mesh position={[-2, 0, 0]}>
            <boxGeometry args={[0.2, 0.2, 0.2]} />
            <meshBasicMaterial color="green" />
          </mesh>
          <mesh position={[2, 0, 0]}>
            <boxGeometry args={[0.2, 0.2, 0.2]} />
            <meshBasicMaterial color="green" />
          </mesh>
        </group>
      </Canvas>
    </div>
  );
};