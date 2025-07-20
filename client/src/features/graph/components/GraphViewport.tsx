import React, { Suspense, useEffect, useState, useCallback, useMemo } from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls, Stats } from '@react-three/drei';
import { EffectComposer, Bloom } from '@react-three/postprocessing';
import { graphDataManager } from '../managers/graphDataManager';
import EnhancedGraphManager from './EnhancedGraphManager';
import { VisualEnhancementToggle } from './VisualEnhancementToggle';
import CameraController from '../../visualisation/components/CameraController'; // Adjusted path
import { useSettingsStore } from '../../../store/settingsStore';
import { createLogger } from '../../../utils/logger';
import { SwarmVisualizationEnhanced } from '../../swarm/components/SwarmVisualizationEnhanced';

// Ensure Three.js types are properly loaded if not globally done
// import '../../../types/react-three-fiber.d.ts';

const logger = createLogger('GraphViewport');

const GraphViewport: React.FC = () => {
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [graphCenter, setGraphCenter] = useState<[number, number, number]>([0, 0, 0]);
  const [graphSize, setGraphSize] = useState(50); // Default size
  const [isNodeDragging, setIsNodeDragging] = useState(false); // <--- Add this state

  // Settings for camera and visuals
  const settings = useSettingsStore(state => state.settings);
  const cameraSettings = settings.visualisation.camera;
  const renderingSettings = settings.visualisation.rendering;
  const bloomSettingsStore = settings.visualisation.bloom;
  const debugSettings = settings.system.debug;

  const fov = cameraSettings?.fov ?? 75;
  const near = cameraSettings?.near ?? 0.1;
  const far = cameraSettings?.far ?? 2000;

  // Memoize cameraPosition to ensure stable reference unless underlying values change
  const cameraPosition = useMemo(() => (
    cameraSettings?.position
      ? [cameraSettings.position.x, cameraSettings.position.y, cameraSettings.position.z]
      : [0, 10, 50] // Default camera position
  ), [cameraSettings?.position]);

  const enableBloom = bloomSettingsStore?.enabled ?? true;
  // Using properties from BloomSettings in settings.ts
  const bloomStrength = bloomSettingsStore?.strength ?? 1.5;
  const bloomThreshold = bloomSettingsStore?.threshold ?? 0.2;
  const bloomRadius = bloomSettingsStore?.radius ?? 0.9;


  useEffect(() => {
    const initializeGraph = async () => {
      setIsLoading(true);
      setError(null);
      try {
        logger.debug('Fetching initial graph data...');
        await graphDataManager.fetchInitialData();
        logger.debug('Graph data fetched.');
        const data = await graphDataManager.getGraphData();

        if (!data || !data.nodes || data.nodes.length === 0) {
          logger.warn('No graph data or empty nodes received.');
          setGraphCenter([0,0,0]);
          setGraphSize(50); // Default size for empty graph
          setIsLoading(false);
          return;
        }

        let minX = Infinity, minY = Infinity, minZ = Infinity;
        let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;

        data.nodes.forEach((node) => {
          if (node.position) {
            minX = Math.min(minX, node.position.x);
            maxX = Math.max(maxX, node.position.x);
            minY = Math.min(minY, node.position.y);
            maxY = Math.max(maxY, node.position.y);
            minZ = Math.min(minZ, node.position.z);
            maxZ = Math.max(maxZ, node.position.z);
          }
        });

        const centerX = (minX === Infinity || maxX === -Infinity) ? 0 : (maxX + minX) / 2;
        const centerY = (minY === Infinity || maxY === -Infinity) ? 0 : (maxY + minY) / 2;
        const centerZ = (minZ === Infinity || maxZ === -Infinity) ? 0 : (maxZ + minZ) / 2;

        const width = (minX === Infinity || maxX === -Infinity) ? 0 : maxX - minX;
        const height = (minY === Infinity || maxY === -Infinity) ? 0 : maxY - minY;
        const depth = (minZ === Infinity || maxZ === -Infinity) ? 0 : maxZ - minZ;

        const maxDimension = Math.max(width, height, depth, 1); // Ensure maxDimension is at least 1

        setGraphCenter([centerX, centerY, centerZ]);
        setGraphSize(maxDimension > 0 ? maxDimension : 50);
        logger.debug('Graph initialized and centered.', { center: [centerX, centerY, centerZ], size: maxDimension });

      } catch (err) {
        logger.error('Failed to fetch initial graph data:', err);
        setError(err instanceof Error ? err.message : 'An unknown error occurred during data fetch.');
      } finally {
        setIsLoading(false);
      }
    };
    initializeGraph();
  }, []);

  if (isLoading) {
    return <div style={{ padding: '2rem', color: '#ccc', height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>Loading graph data...</div>;
  }

  if (error) {
    return <div style={{ padding: '2rem', color: 'red', height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>Error loading graph data: {error}</div>;
  }

  const backgroundColor = renderingSettings?.backgroundColor ?? '#000000';

  return (
    <div style={{ width: '100%', height: '100%', position: 'relative' }}>
      <Canvas
        style={{ display: 'block', width: '100%', height: '100%' }}
        camera={{
          fov: fov,
          near: near,
          far: far,
          position: cameraPosition as [number, number, number],
        }}
        gl={{ antialias: true, alpha: true, powerPreference: 'high-performance' }}
        dpr={[1, 2]} // Pixel ratio for sharpness
        shadows // Enable shadows
      >
        <color attach="background" args={[backgroundColor]} />
        <CameraController center={graphCenter} size={graphSize} />

        <ambientLight intensity={renderingSettings?.ambientLightIntensity ?? 0.6} />
        <directionalLight
          position={[10, 10, 5]} // Using hardcoded default as not in settings
          intensity={renderingSettings?.directionalLightIntensity ?? 1}
          castShadow
        />
        <pointLight
          position={[-10, -10, -5]} // Using hardcoded default as not in settings
          intensity={0.5} // Using hardcoded default as not in settings
        />

        <OrbitControls
          makeDefault
          enableDamping
          dampingFactor={0.05}
          minDistance={1}
          maxDistance={far / 2} // Max distance related to camera far plane
          target={graphCenter}
          enabled={!isNodeDragging} // <--- Control OrbitControls here
        />

        <Suspense fallback={null}>
          {/* Using EnhancedGraphManager for all graph rendering */}
          <EnhancedGraphManager />
          {/* Note: Enhanced features can be toggled via settings */}
          {/* HologramVisualisation could be added here if it's part of the core graph view */}
          {/* <HologramVisualisation standalone={false} position={[0, 0, 0]} size={20} /> */}
          
          {/* VisionFlow visualization re-enabled in same origin space */}
          <SwarmVisualizationEnhanced />
        </Suspense>

        {/* Removed showAxesHelper and showStats as they are not in DebugSettings type from settings.ts */}
        {/* {debugSettings?.showAxesHelper && <axesHelper args={[graphSize > 0 ? graphSize / 10 : 2]} />} */}
        {/* {debugSettings?.showStats && <Stats />} */}
        {debugSettings?.enabled && <Stats />} {/* Show Stats if general debug is enabled, as a fallback */}


        {enableBloom && (
          <EffectComposer>
            <Bloom
              intensity={bloomStrength} // Mapped from BloomSettings.strength
              luminanceThreshold={bloomThreshold} // Mapped from BloomSettings.threshold
              luminanceSmoothing={bloomRadius} // Mapped from BloomSettings.radius (best guess)
            />
          </EffectComposer>
        )}
      </Canvas>
    </div>
  );
};

export default GraphViewport;