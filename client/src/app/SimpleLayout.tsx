import React, { useState, useRef } from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls, Stats } from '@react-three/drei';
import EnhancedGraphManager from '../features/graph/components/EnhancedGraphManager';
import { SwarmVisualizationEnhanced } from '../features/swarm/components/SwarmVisualizationEnhanced';
// import { DualVisualizationControls } from '../features/graph/components/DualVisualizationControls'; // Removed - both graphs now at origin
import { PostProcessingEffects } from '../features/graph/components/PostProcessingEffects';
import { SpacePilotSimpleIntegration } from '../features/visualisation/components/SpacePilotSimpleIntegration';
import { IntegratedControlPanelEnhanced } from '../features/visualisation/components/IntegratedControlPanelEnhanced';
import { useSettingsStore } from '../store/settingsStore';
import { SwarmDataProvider, useSwarmData } from '../features/swarm/contexts/SwarmDataContext';

const SimpleLayoutContent: React.FC = () => {
  // Both visualizations now positioned at origin (0, 0, 0) for unified view
  const { settings } = useSettingsStore();
  const { swarmData } = useSwarmData();
  const showStats = settings?.system?.debug?.enablePerformanceDebug ?? false;
  const enableBloom = settings?.visualisation?.bloom?.enabled ?? false;
  const [orbitControlsEnabled, setOrbitControlsEnabled] = useState(true);
  const orbitControlsRef = useRef<any>(null);

  const handleOrbitControlsToggle = (enabled: boolean) => {
    setOrbitControlsEnabled(enabled);
    if (orbitControlsRef.current) {
      orbitControlsRef.current.enabled = enabled;
    }
  };

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
          position: [0, 20, 60], // Adjusted camera position for better view of unified graphs
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

        {/* Logseq Graph Visualization - positioned at origin */}
        <group position={[0, 0, 0]}>
          <EnhancedGraphManager />
        </group>

        {/* VisionFlow Swarm Visualization - also positioned at origin for unified view */}
        <group position={[0, 0, 0]}>
          <SwarmVisualizationEnhanced />
        </group>

        {/* Camera Controls */}
        <OrbitControls 
          ref={orbitControlsRef}
          enabled={orbitControlsEnabled}
          enablePan={true} 
          enableZoom={true} 
          enableRotate={true}
          zoomSpeed={0.8}
          panSpeed={0.8}
          rotateSpeed={0.8}
        />
        
        {/* SpacePilot 6DOF Controller */}
        <SpacePilotSimpleIntegration />

        {/* Performance stats */}
        {showStats && <Stats />}
        
        {/* Post-processing effects */}
        {enableBloom && <PostProcessingEffects />}
      </Canvas>
      
      {/* Integrated Control Panel */}
      <IntegratedControlPanelEnhanced
        showStats={showStats}
        enableBloom={enableBloom}
        onOrbitControlsToggle={handleOrbitControlsToggle}
        swarmData={swarmData ? {
          nodeCount: swarmData.nodeCount,
          edgeCount: swarmData.edgeCount,
          tokenCount: swarmData.tokenCount,
          mcpConnected: swarmData.mcpConnected,
          dataSource: swarmData.dataSource
        } : undefined}
      />
    </div>
  );
};

const SimpleLayout: React.FC = () => {
  return (
    <SwarmDataProvider>
      <SimpleLayoutContent />
    </SwarmDataProvider>
  );
};

export default SimpleLayout;