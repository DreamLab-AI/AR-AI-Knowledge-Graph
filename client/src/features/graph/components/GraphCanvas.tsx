import { useRef } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import { OrbitControls, Stats } from '@react-three/drei';

// Components
 import GraphManager from './GraphManager';
import XRController from '../../xr/components/XRController';
import XRVisualisationConnector from '../../xr/components/XRVisualisationConnector';
import { SwarmVisualizationEnhanced } from '../../swarm/components/SwarmVisualizationEnhanced';
// import { SwarmVisualizationIntegrated } from '../../swarm/components/SwarmVisualizationIntegrated';
// import { SwarmVisualizationDebug } from '../../swarm/components/SwarmVisualizationDebug';

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
    console.log('[GRAPH CANVAS] Component rendering...');
    console.log('[GRAPH CANVAS] About to render SwarmVisualizationEnhanced');
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const { settings } = useSettingsStore();
    const showStats = settings?.system?.debug?.enablePerformanceDebug ?? false; // Use performance debug flag
    const xrEnabled = settings?.xr?.enabled !== false;
    const antialias = settings?.visualisation?.rendering?.enableAntialiasing !== false; // Correct property name

    // Removed the outer div wrapper
    return (
        <Canvas
            ref={canvasRef}
            className="r3f-canvas overflow-hidden" // Added overflow-hidden class here
            style={{
                width: '100%',
                height: '100%',
                minHeight: '0', // Ensure it can shrink
                display: 'block' // Revert to display: block
                // Removed flex properties from Canvas style
            }}
            gl={{
                antialias,
                alpha: true,
                powerPreference: 'high-performance',
                failIfMajorPerformanceCaveat: false
            }}
            camera={{
                fov: 75,
                near: 0.1,
                far: 2000, // Remove settings access, camera settings likely managed elsewhere
                position: [0, 10, 50]
            }}
            onCreated={({ gl }) => {
                if (debugState.isEnabled()) {
                    logger.debug('Canvas created with dimensions:', {
                        width: gl.domElement.width,
                        height: gl.domElement.height,
                        containerWidth: gl.domElement.parentElement?.clientWidth,
                        containerHeight: gl.domElement.parentElement?.clientHeight
                    });
                }
            }}
        >
            <SceneSetup />
            <GraphManager />
            <SwarmVisualizationEnhanced />
            {xrEnabled && <XRController />}
            {xrEnabled && <XRVisualisationConnector />}
            {showStats && <Stats />}
        </Canvas>
    );
};

export default GraphCanvas;
