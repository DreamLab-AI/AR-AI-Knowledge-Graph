import { useRef, useState, useEffect } from 'react';
import { Object3D, Group, Vector3 } from 'three';
import { useThree, useFrame } from '@react-three/fiber';
import { Interactive } from '@react-three/xr';
import { useSettingsStore } from '../../../../store/settingsStore';
import { createLogger } from '../../../../utils/logger';

const logger = createLogger('XRControlPanel');

interface XRControlPanelProps {
  /**
   * Distance from controller to position the panel
   * @default 0.3
   */
  distance?: number;
  
  /**
   * Size of the panel
   * @default {width: 0.3, height: 0.2}
   */
  size?: {
    width: number;
    height: number;
  };
  
  /**
   * Whether to show the panel
   * @default true
   */
  visible?: boolean;

  /**
   * Controller to attach the panel to (0 = right, 1 = left)
   * @default 0
   */
  controller?: number;

  /**
   * Whether the panel should face the user
   * @default true
   */
  faceUser?: boolean;
}

/**
 * XRControlPanel provides an interactive interface for controlling settings in XR mode.
 * It's attached to a controller and follows its movement while providing touch/pointer interaction.
 */
const XRControlPanel = ({
  distance = 0.3,
  size = { width: 0.3, height: 0.2 },
  visible = true,
  controller = 0,
  faceUser = true,
}: XRControlPanelProps) => {
  const groupRef = useRef<Group>(null);
  const { camera } = useThree();
  const [hovered, setHovered] = useState(false);
  const [selectedTab, setSelectedTab] = useState('visualisation');
  
  const { settings } = useSettingsStore();
  
  // Position the panel relative to the controller
  useFrame((state, delta) => {
    if (!groupRef.current) return;
    
    // If we have controller data, position the panel relative to it
    const controllers = (state as any).controllers;
    if (controllers && controllers[controller]) {
      const controllerObj = controllers[controller];
      
      // Get controller position and orientation
      const position = new Vector3();
      controllerObj.getWorldPosition(position);
      
      const quaternion = controllerObj.getWorldQuaternion();
      
      // Position the panel in front of the controller
      const forward = new Vector3(0, 0, -1).applyQuaternion(quaternion);
      position.addScaledVector(forward, distance);
      
      groupRef.current.position.copy(position);
      
      // Either match controller orientation or face the user
      if (faceUser) {
        // Make the panel face the user
        groupRef.current.lookAt(camera.position);
      } else {
        // Match controller orientation
        groupRef.current.quaternion.copy(quaternion);
      }
    }
  });
  
  // Handle tab selection
  const handleTabSelect = (tab: string) => {
    setSelectedTab(tab);
    logger.debug(`Selected XR panel tab: ${tab}`);
  };
  
  if (!visible) return null;
  
  return (
    <group ref={groupRef}>
      {/* Panel background */}
      <Interactive 
        onSelect={() => {}}
        onHover={() => setHovered(true)}
        onBlur={() => setHovered(false)}
      >
        <mesh position={[0, 0, 0]}>
          <planeGeometry args={[size.width, size.height]} />
          <meshStandardMaterial 
            color={hovered ? '#2a2a2a' : '#1a1a1a'} 
            transparent
            opacity={0.8}
          />
        </mesh>
      </Interactive>
      
      {/* Tab buttons at the top */}
      <group position={[0, size.height / 2 - 0.02, 0.001]}>
        {/* Visualisation tab */}
        <Interactive onSelect={() => handleTabSelect('visualisation')}>
          <mesh position={[-size.width / 4, 0, 0]}>
            <planeGeometry args={[size.width / 3.5, 0.03]} />
            <meshStandardMaterial 
              color={selectedTab === 'visualisation' ? '#4a86e8' : '#333333'} 
            />
          </mesh>
        </Interactive>
        
        {/* XR tab */}
        <Interactive onSelect={() => handleTabSelect('xr')}>
          <mesh position={[size.width / 4, 0, 0]}>
            <planeGeometry args={[size.width / 3.5, 0.03]} />
            <meshStandardMaterial 
              color={selectedTab === 'xr' ? '#4a86e8' : '#333333'} 
            />
          </mesh>
        </Interactive>
      </group>
      
      {/* Panel content - dynamically render based on selected tab */}
      <group position={[0, 0, 0.001]}>
        {selectedTab === 'visualisation' && (
          <group>
            {/* Simple visualisation controls */}
            <mesh position={[0, 0.05, 0]} scale={[0.9, 0.1, 1]}>
              <planeGeometry />
              <meshStandardMaterial color="#333333" />
            </mesh>
            <mesh position={[0, -0.05, 0]} scale={[0.9, 0.1, 1]}>
              <planeGeometry />
              <meshStandardMaterial color="#333333" />
            </mesh>
          </group>
        )}
        
        {selectedTab === 'xr' && (
          <group>
            {/* Simple XR controls */}
            <mesh position={[0, 0, 0]} scale={[0.9, 0.15, 1]}>
              <planeGeometry />
              <meshStandardMaterial color="#333333" />
            </mesh>
          </group>
        )}
      </group>
    </group>
  );
};

export default XRControlPanel;