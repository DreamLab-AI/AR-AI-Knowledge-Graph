import { useEffect, useRef } from 'react';
import { useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { useSpacePilot } from '../hooks/useSpacePilot';
import { useSettingsStore } from '../../../store/settingsStore';

/**
 * SpacePilot integration that enhances existing OrbitControls with 6DOF input
 * This component should be placed in the same Canvas as your OrbitControls
 */
export const SpacePilotIntegration: React.FC = () => {
  const { camera, controls } = useThree();
  const settings = useSettingsStore(state => state.settings);
  const spacePilotEnabled = settings?.visualisation?.spacePilot?.enabled !== false;

  // Initialize SpacePilot hook
  const spacePilot = useSpacePilot({
    enabled: spacePilotEnabled,
    onConnect: () => {
      console.log('[SpacePilot] Connected to 6DOF controller');
    },
    onDisconnect: () => {
      console.log('[SpacePilot] Disconnected from 6DOF controller');
    },
    onModeChange: (mode) => {
      console.log('[SpacePilot] Mode changed to:', mode);
    }
  });

  // Auto-connect if WebHID is supported and SpacePilot is enabled
  useEffect(() => {
    if (spacePilotEnabled && spacePilot.isSupported && !spacePilot.isConnected) {
      // Check if a device was previously connected
      const autoConnect = localStorage.getItem('spacepilot-auto-connect');
      if (autoConnect === 'true') {
        spacePilot.connect().catch(err => {
          console.warn('[SpacePilot] Auto-connect failed:', err);
          localStorage.removeItem('spacepilot-auto-connect');
        });
      }
    }
  }, [spacePilotEnabled, spacePilot.isSupported, spacePilot.isConnected]);

  // Store connection state for auto-reconnect
  useEffect(() => {
    if (spacePilot.isConnected) {
      localStorage.setItem('spacepilot-auto-connect', 'true');
    }
  }, [spacePilot.isConnected]);

  // The actual 6DOF control is handled by the SpacePilotController
  // which is initialized inside the useSpacePilot hook
  // It automatically integrates with the camera and OrbitControls

  // This component doesn't render anything visible
  return null;
};

export default SpacePilotIntegration;