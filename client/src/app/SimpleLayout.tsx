import React, { useState } from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls, Stats } from '@react-three/drei';
import EnhancedGraphManager from '../features/graph/components/EnhancedGraphManager';
import { SwarmVisualizationEnhanced } from '../features/swarm/components/SwarmVisualizationEnhanced';
import { DualVisualizationControls } from '../features/graph/components/DualVisualizationControls';
import { PostProcessingEffects } from '../features/graph/components/PostProcessingEffects';
import { useSettingsStore } from '../store/settingsStore';

const SimpleLayout: React.FC = () => {
  const [separationDistance, setSeparationDistance] = useState(20);
  const { settings } = useSettingsStore();
  const showStats = settings?.system?.debug?.enablePerformanceDebug ?? false;
  const enableBloom = settings?.visualisation?.bloom?.enabled ?? false;

  return (
    <div style={{ 
      position: 'fixed',
      top: 0,
      left: 0,
      width: '100vw',
      height: '100vh',
      backgroundColor: '#000022'
    }}>
      <Canvas
        camera={{ 
          position: [0, 10, 50], 
          fov: 75,
          near: 0.1,
          far: 2000
        }}
        style={{ width: '100%', height: '100%' }}
        gl={{
          antialias: true,
          alpha: true,
          powerPreference: 'high-performance'
        }}
      >
        {/* Scene lighting */}
        <color attach="background" args={[0, 0, 0.05]} />
        <ambientLight intensity={0.6} />
        <directionalLight intensity={0.8} position={[1, 1, 1]} />

        {/* Logseq Graph Visualization - positioned on the left */}
        <group position={[-separationDistance, 0, 0]}>
          <EnhancedGraphManager />
        </group>

        {/* VisionFlow Swarm Visualization - positioned on the right */}
        <group position={[separationDistance, 0, 0]}>
          <SwarmVisualizationEnhanced />
        </group>

        {/* Dual Visualization Controls */}
        <DualVisualizationControls
          separationDistance={separationDistance}
          setSeparationDistance={setSeparationDistance}
        />

        {/* Camera Controls */}
        <OrbitControls 
          enablePan={true} 
          enableZoom={true} 
          enableRotate={true}
          zoomSpeed={0.8}
          panSpeed={0.8}
          rotateSpeed={0.8}
        />

        {/* Performance stats */}
        {showStats && <Stats />}
        
        {/* Post-processing effects */}
        {enableBloom && <PostProcessingEffects />}
      </Canvas>
      
      {/* Debug info */}
      <div style={{
        position: 'absolute',
        top: 10,
        left: 10,
        color: 'white',
        fontFamily: 'monospace',
        fontSize: '12px',
        backgroundColor: 'rgba(0,0,0,0.7)',
        padding: '10px',
        borderRadius: '5px'
      }}>
        <div>Simple Layout - Dual Graph View</div>
        <div>Separation: {separationDistance}</div>
        <div>Stats: {showStats ? 'ON' : 'OFF'}</div>
        <div>Bloom: {enableBloom ? 'ON' : 'OFF'}</div>
      </div>
    </div>
  );
};

export default SimpleLayout;