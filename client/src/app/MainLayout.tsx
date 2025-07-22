import React, { useState, useRef, useEffect } from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls, Stats } from '@react-three/drei';
import GraphManager from '../features/graph/components/GraphManager';
import { BotsVisualization } from '../features/bots/components/BotsVisualization';
// import { DualVisualizationControls } from '../features/graph/components/DualVisualizationControls'; // Removed - both graphs now at origin
import { PostProcessingEffects } from '../features/graph/components/PostProcessingEffects';
import { SpacePilotSimpleIntegration } from '../features/visualisation/components/SpacePilotSimpleIntegration';
import { IntegratedControlPanel } from '../features/visualisation/components/IntegratedControlPanel';
import { HologramVisualisation } from '../features/visualisation/components/HologramVisualisation';
import { useSettingsStore } from '../store/settingsStore';
import { BotsDataProvider, useBotsData } from '../features/bots/contexts/BotsDataContext';
import { AuthGatedVoiceButton } from '../components/AuthGatedVoiceButton';
import { AuthGatedVoiceIndicator } from '../components/AuthGatedVoiceIndicator';
import { BrowserSupportWarning } from '../components/BrowserSupportWarning';
import { AudioInputService } from '../services/AudioInputService';

const MainLayoutContent: React.FC = () => {
  // Both visualizations now positioned at origin (0, 0, 0) for unified view
  const { settings } = useSettingsStore();
  const { botsData } = useBotsData();
  const showStats = settings?.system?.debug?.enablePerformanceDebug ?? false;
  const enableBloom = settings?.visualisation?.bloom?.enabled ?? false;
  const [orbitControlsEnabled, setOrbitControlsEnabled] = useState(true);
  const orbitControlsRef = useRef<any>(null);
  const [hasVoiceSupport, setHasVoiceSupport] = useState(true);

  const handleOrbitControlsToggle = (enabled: boolean) => {
    setOrbitControlsEnabled(enabled);
    if (orbitControlsRef.current) {
      orbitControlsRef.current.enabled = enabled;
    }
  };

  useEffect(() => {
    const support = AudioInputService.getBrowserSupport();
    const isSupported = support.getUserMedia && support.isHttps && support.audioContext && support.mediaRecorder;
    setHasVoiceSupport(isSupported);
  }, []);

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
          <GraphManager />
        </group>

        {/* VisionFlow Bots Visualization - also positioned at origin for unified view */}
        <group position={[0, 0, 0]}>
          <BotsVisualization />
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
      <IntegratedControlPanel
        showStats={showStats}
        enableBloom={enableBloom}
        onOrbitControlsToggle={handleOrbitControlsToggle}
        botsData={botsData ? {
          nodeCount: botsData.nodeCount,
          edgeCount: botsData.edgeCount,
          tokenCount: botsData.tokenCount,
          mcpConnected: botsData.mcpConnected,
          dataSource: botsData.dataSource
        } : undefined}
      />

      {/* Browser Support Warning - Only show when there's no voice support */}
      {!hasVoiceSupport && (
        <div className="fixed bottom-20 left-4 z-40 max-w-sm pointer-events-auto">
          <BrowserSupportWarning className="shadow-lg" />
        </div>
      )}

      {/* Voice Interaction Components - Only show when browser supports it */}
      {hasVoiceSupport && (
        <div className="fixed bottom-4 left-4 z-50 flex flex-col gap-1 items-start pointer-events-auto">
          <AuthGatedVoiceButton size="md" variant="primary" />
          <AuthGatedVoiceIndicator
            className="max-w-xs text-xs"
            showTranscription={true}
            showStatus={false}
          />
        </div>
      )}
    </div>
  );
};

const MainLayout: React.FC = () => {
  return (
    <BotsDataProvider>
      <MainLayoutContent />
    </BotsDataProvider>
  );
};

export default MainLayout;