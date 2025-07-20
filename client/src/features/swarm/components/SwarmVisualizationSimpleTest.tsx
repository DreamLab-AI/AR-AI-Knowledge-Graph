import React from 'react';
import * as THREE from 'three';

export const SwarmVisualizationSimpleTest: React.FC = () => {
  console.log('[SWARM TEST] Simple test component rendering...');
  
  return (
    <group position={[60, 0, 0]}>
      {/* Gold box */}
      <mesh position={[0, 0, 0]}>
        <boxGeometry args={[10, 10, 10]} />
        <meshStandardMaterial color="#F1C40F" />
      </mesh>
      
      {/* Green sphere */}
      <mesh position={[15, 0, 0]}>
        <sphereGeometry args={[5, 32, 16]} />
        <meshStandardMaterial color="#2ECC71" />
      </mesh>
      
      {/* Test text */}
      <mesh position={[0, 15, 0]}>
        <planeGeometry args={[20, 5]} />
        <meshBasicMaterial color="#000000" />
      </mesh>
    </group>
  );
};