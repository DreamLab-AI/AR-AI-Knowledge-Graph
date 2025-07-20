import React from 'react';
import * as THREE from 'three';
import { createLogger } from '../../../utils/logger';

const logger = createLogger('SwarmVisualizationDebug');

export const SwarmVisualizationDebug: React.FC = () => {
  logger.info('SwarmVisualizationDebug component mounted');
  
  // Simple colored box to test if anything renders
  return (
    <group position={[50, 0, 0]}>
      <mesh>
        <boxGeometry args={[10, 10, 10]} />
        <meshStandardMaterial color="#F1C40F" />
      </mesh>
      
      {/* Add a simple sphere too */}
      <mesh position={[15, 0, 0]}>
        <sphereGeometry args={[5, 32, 16]} />
        <meshStandardMaterial color="#2ECC71" />
      </mesh>
    </group>
  );
};