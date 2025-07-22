import { useRef, useState } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import { OrbitControls, Stats } from '@react-three/drei';

// Components
import EnhancedGraphManager from './EnhancedGraphManager';
import { PostProcessingEffects } from './PostProcessingEffects';
import XRController from '../../xr/components/XRController';
import XRVisualisationConnector from '../../xr/components/XRVisualisationConnector';
import { SwarmVisualizationEnhanced } from '../../swarm/components/SwarmVisualizationEnhanced';
// import { DualVisualizationControls } from './DualVisualizationControls'; // Removed - both graphs now at origin

// SpacePilot Integration
import { SpacePilotSimpleIntegration } from '../../visualisation/components/SpacePilotSimpleIntegration';

// Store and utils
import { useSettingsStore } from '../../../store/settingsStore';
import { createLogger } from '../../../utils/logger';
import { debugState } from '../../../utils/debugState';

const logger = createLogger('GraphCanvas');

// Scene setup with lighting and background
const SceneSetup = () => {
    const { scene } = useThree();
    const settings = useSettingsStore(state => state.settings?.visualisation);

    // Render lights using JSX
    return (
        <>
            <color attach="background" args={[0, 0, 0.8]} /> {/* Medium blue background */}
            <ambientLight intensity={0.6} />
            <directionalLight
                intensity={0.8}
                position={[1, 1, 1]}
            />
        </>
    );
};

// Main GraphCanvas component
const GraphCanvas = () => {
    // console.log('[GRAPH CANVAS] Component rendering...');
    // console.log('[GRAPH CANVAS] About to render SwarmVisualizationEnhanced');
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const { settings } = useSettingsStore();
    const showStats = settings?.system?.debug?.enablePerformanceDebug ?? false; // Use performance debug flag
    const xrEnabled = settings?.xr?.enabled !== false;
    const antialias = settings?.visualisation?.rendering?.enableAntialiasing !== false; // Correct property name
    

    // Both visualizations now positioned at origin (0, 0, 0) for unified view

    // Wrapper to ensure proper canvas sizing
    return (
        <div style={{ width: '100%', height: '100%', backgroundColor: '#000033' }}>
            <Canvas
                ref={canvasRef}
                gl={{
                    antialias,
                    alpha: true,
                    powerPreference: 'high-performance',
                    failIfMajorPerformanceCaveat: false
                }}
                camera={{
                    fov: 75,
                    near: 0.1,
                    far: 2000,
                    position: [0, 20, 60] // Adjusted camera position for better view of unified graphs
                }}
                onCreated={({ gl }) => {
                    if (debugState.isEnabled()) {
                        logger.debug('Canvas created with dimensions:', {
                            width: gl.domElement.width,
                            height: gl.domElement.height
                        });
                    }
                }}
            >
                <SceneSetup />

                {/* Logseq Graph Visualization - positioned at origin */}
                <group position={[0, 0, 0]}>
                    <EnhancedGraphManager />
                </group>

                {/* VisionFlow Swarm Visualization - also positioned at origin for unified view */}
                <group position={[0, 0, 0]}>
                    <SwarmVisualizationEnhanced />
                </group>

                {/* Camera Controls with SpacePilot Integration */}
                <OrbitControls
                    enablePan={true}
                    enableZoom={true}
                    enableRotate={true}
                    zoomSpeed={0.8}
                    panSpeed={0.8}
                    rotateSpeed={0.8}
                />

                {xrEnabled && <XRController />}
                {xrEnabled && <XRVisualisationConnector />}
                {showStats && <Stats />}
                {settings?.visualisation?.bloom?.enabled && <PostProcessingEffects />}
            </Canvas>
        </div>
    );
};

export default GraphCanvas;
