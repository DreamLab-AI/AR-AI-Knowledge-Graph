import { useEffect } from 'react';
import { OrbitControls } from '@react-three/drei';
import { SpaceDriver } from '../services/SpaceDriverService';

/**
 * Hook to ensure mouse controls work properly, especially when SpaceMouse is unavailable
 */
export function useMouseControls(orbitControlsRef: React.RefObject<OrbitControls>) {
  useEffect(() => {
    // Function to ensure OrbitControls are enabled
    const ensureControlsEnabled = () => {
      if (orbitControlsRef.current && 'enabled' in orbitControlsRef.current) {
        orbitControlsRef.current.enabled = true;
        console.log('[useMouseControls] OrbitControls enabled');
      }
    };

    // Check WebHID availability on mount
    const hasWebHID = 'hid' in navigator;
    const isSecureContext = window.isSecureContext;

    if (!hasWebHID || !isSecureContext) {
      // If WebHID is not available, ensure OrbitControls are always enabled
      ensureControlsEnabled();
      console.log('[useMouseControls] WebHID not available or insecure context - OrbitControls permanently enabled');
    }

    // Listen for WebHID unavailable event
    const handleWebHIDUnavailable = () => {
      ensureControlsEnabled();
    };

    // Listen for SpaceMouse disconnect
    const handleDisconnect = () => {
      ensureControlsEnabled();
    };

    SpaceDriver.addEventListener('webhid-unavailable', handleWebHIDUnavailable);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);

    // Ensure controls are enabled on mount
    setTimeout(ensureControlsEnabled, 100);

    return () => {
      SpaceDriver.removeEventListener('webhid-unavailable', handleWebHIDUnavailable);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
    };
  }, [orbitControlsRef]);
}