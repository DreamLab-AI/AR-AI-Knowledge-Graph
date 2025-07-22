import React, { useState } from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls, Box, Grid, Environment } from '@react-three/drei';
import { useSpacePilot } from '../hooks/useSpacePilot';
import { SpacePilotStatus } from './SpacePilotStatus';
import { SpacePilotSettings } from './SpacePilotSettings';
import { SpacePilotButtonPanel } from './SpacePilotButtonPanel';
import { Button } from '../../design-system/components/Button';
import { Modal } from '../../design-system/components/Modal';
import { Settings } from 'lucide-react';
import * as THREE from 'three';

/**
 * Demo scene component that uses SpacePilot
 */
const SpacePilotScene: React.FC = () => {
  const [selectedObject, setSelectedObject] = useState<THREE.Object3D | null>(null);
  const [showSettings, setShowSettings] = useState(false);

  const spacePilot = useSpacePilot({
    onConnect: () => console.log('SpacePilot connected'),
    onDisconnect: () => console.log('SpacePilot disconnected'),
    onModeChange: (mode) => console.log('Mode changed to:', mode)
  });

  // Pass selected object to controller when in object mode
  React.useEffect(() => {
    if (spacePilot.currentMode === 'object' && selectedObject) {
      // The controller will handle object manipulation
      // This is where you'd integrate with your object selection system
    }
  }, [spacePilot.currentMode, selectedObject]);

  return (
    <>
      {/* 3D Scene */}
      <Canvas
        camera={{ position: [10, 10, 10], fov: 50 }}
        className="w-full h-full"
      >
        <ambientLight intensity={0.5} />
        <directionalLight position={[10, 10, 5]} intensity={1} />
        
        {/* Grid */}
        <Grid args={[20, 20]} />
        
        {/* Demo objects */}
        <Box
          position={[-3, 1, 0]}
          args={[2, 2, 2]}
          onClick={(e) => {
            e.stopPropagation();
            setSelectedObject(e.object);
          }}
        >
          <meshStandardMaterial 
            color={selectedObject?.uuid === e.object.uuid ? '#00ff00' : '#ff0000'} 
          />
        </Box>
        
        <Box
          position={[3, 1, 0]}
          args={[1.5, 3, 1.5]}
          onClick={(e) => {
            e.stopPropagation();
            setSelectedObject(e.object);
          }}
        >
          <meshStandardMaterial 
            color={selectedObject?.uuid === e.object.uuid ? '#00ff00' : '#0000ff'} 
          />
        </Box>
        
        <Box
          position={[0, 0.5, -3]}
          args={[1, 1, 1]}
          onClick={(e) => {
            e.stopPropagation();
            setSelectedObject(e.object);
          }}
        >
          <meshStandardMaterial 
            color={selectedObject?.uuid === e.object.uuid ? '#00ff00' : '#ffff00'} 
          />
        </Box>
        
        {/* Environment for reflections */}
        <Environment preset="sunset" />
        
        {/* OrbitControls - will be enhanced by SpacePilot */}
        <OrbitControls 
          enableDamping 
          dampingFactor={0.05}
          rotateSpeed={0.5}
        />
      </Canvas>

      {/* UI Overlay */}
      <div className="absolute top-4 left-4 right-4 flex justify-between items-start pointer-events-none">
        <div className="pointer-events-auto">
          <SpacePilotStatus
            connected={spacePilot.isConnected}
            mode={spacePilot.currentMode}
            sensitivity={spacePilot.config.translationSensitivity.x}
            onConnect={spacePilot.connect}
            onModeChange={spacePilot.setMode}
            onSettingsClick={() => setShowSettings(true)}
          />
        </div>
        
        {!spacePilot.isSupported && (
          <div className="bg-red-900/80 text-red-200 px-4 py-2 rounded-lg pointer-events-auto">
            WebHID is not supported in this browser
          </div>
        )}
      </div>

      {/* Button Panel - positioned on the right side */}
      {spacePilot.isConnected && (
        <div className="absolute top-4 right-4 pointer-events-auto">
          <SpacePilotButtonPanel
            onButtonPress={(buttonNumber) => {
              console.log(`Button ${buttonNumber} pressed`);
              // Handle button actions here
              if (buttonNumber === 1) {
                spacePilot.resetView();
              } else if (buttonNumber === 2) {
                const modes: Array<'camera' | 'object' | 'navigation'> = ['camera', 'object', 'navigation'];
                const currentIndex = modes.indexOf(spacePilot.currentMode);
                spacePilot.setMode(modes[(currentIndex + 1) % modes.length]);
              }
            }}
          />
        </div>
      )}

      {/* Settings Modal */}
      <Modal
        isOpen={showSettings}
        onClose={() => setShowSettings(false)}
        title="SpacePilot Settings"
        className="max-w-2xl"
      >
        <SpacePilotSettings
          config={spacePilot.config}
          onConfigChange={spacePilot.updateConfig}
          onCalibrate={spacePilot.calibrate}
          onResetDefaults={() => {
            // Reset to defaults
            spacePilot.updateConfig({
              translationSensitivity: { x: 1, y: 1, z: 1 },
              rotationSensitivity: { x: 1, y: 1, z: 1 },
              deadzone: 0.1,
              smoothing: 0.8
            });
          }}
        />
      </Modal>

      {/* Instructions */}
      <div className="absolute bottom-4 left-4 right-4 text-center text-sm text-gray-400 pointer-events-none">
        {!spacePilot.isConnected ? (
          <p>Click "Connect" to use your SpacePilot 3D mouse</p>
        ) : (
          <p>
            Mode: {spacePilot.currentMode} | 
            {spacePilot.currentMode === 'object' && ' Click on objects to select them |'}
            Press [1] to reset view | Press [2] to cycle modes
          </p>
        )}
      </div>
    </>
  );
};

/**
 * Main integration component that can be dropped into any page
 */
export const SpacePilotIntegration: React.FC = () => {
  return (
    <div className="relative w-full h-screen bg-gray-900">
      <SpacePilotScene />
    </div>
  );
};

/**
 * Minimal example for documentation
 */
export const SpacePilotExample: React.FC = () => {
  const spacePilot = useSpacePilot();

  if (!spacePilot.isSupported) {
    return <div>WebHID is not supported in this browser</div>;
  }

  return (
    <div>
      <Button onClick={spacePilot.connect}>
        {spacePilot.isConnected ? 'Connected' : 'Connect SpacePilot'}
      </Button>
      <p>Current Mode: {spacePilot.currentMode}</p>
    </div>
  );
};

export default SpacePilotIntegration;