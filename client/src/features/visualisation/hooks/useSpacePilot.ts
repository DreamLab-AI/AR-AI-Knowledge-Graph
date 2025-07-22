import { useEffect, useState, useRef, useCallback } from 'react';
import { useThree } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { SpacePilotController, SpacePilotConfig, defaultSpacePilotConfig } from '../controls/SpacePilotController';
import { useSettingsStore } from '../../../store/settingsStore';

import { SpaceDriver } from '../../../services/SpaceDriverService';

export interface SpacePilotOptions {
  enabled?: boolean;
  config?: Partial<SpacePilotConfig>;
  onConnect?: () => void;
  onDisconnect?: () => void;
  onModeChange?: (mode: 'camera' | 'object' | 'navigation') => void;
}

export interface SpacePilotHookReturn {
  isConnected: boolean;
  isSupported: boolean;
  currentMode: 'camera' | 'object' | 'navigation';
  setMode: (mode: 'camera' | 'object' | 'navigation') => void;
  connect: () => Promise<void>;
  disconnect: () => void;
  calibrate: () => void;
  resetView: () => void;
  updateConfig: (config: Partial<SpacePilotConfig>) => void;
  config: SpacePilotConfig;
}

/**
 * React hook for SpacePilot integration with Three.js
 */
export function useSpacePilot(options: SpacePilotOptions = {}): SpacePilotHookReturn {
  const {
    enabled = true,
    config: userConfig = {},
    onConnect,
    onDisconnect,
    onModeChange
  } = options;

  const { camera, scene, gl, controls } = useThree();
  const [isConnected, setIsConnected] = useState(false);
  const [isSupported, setIsSupported] = useState(false);
  const [currentMode, setCurrentMode] = useState<'camera' | 'object' | 'navigation'>('camera');
  
  const controllerRef = useRef<SpacePilotController | null>(null);
  const configRef = useRef<SpacePilotConfig>({ ...defaultSpacePilotConfig, ...userConfig });
  
  // Load settings from store
  const settings = useSettingsStore(state => state.settings);
  const spacePilotSettings = settings?.visualisation?.spacePilot;

  // Check WebHID support
  useEffect(() => {
    setIsSupported('hid' in navigator);
  }, []);

  // Initialize controller
  useEffect(() => {
    if (!enabled || !camera || !isSupported) return;

    // Merge settings from store with user config
    const mergedConfig = {
      ...defaultSpacePilotConfig,
      ...spacePilotSettings,
      ...userConfig
    };
    configRef.current = mergedConfig;

    // Create controller instance
    const controller = new SpacePilotController(
      camera,
      mergedConfig,
      controls as OrbitControls | undefined
    );
    controllerRef.current = controller;

    // Set initial mode
    setCurrentMode(mergedConfig.mode);

    return () => {
      controller.stop();
      controllerRef.current = null;
    };
  }, [enabled, camera, controls, isSupported, spacePilotSettings, userConfig]);

  // Handle SpaceDriver events
  useEffect(() => {
    if (!enabled || !isSupported || !controllerRef.current) return;

    const handleTranslate = (event: CustomEvent) => {
      controllerRef.current?.handleTranslation(event.detail);
    };

    const handleRotate = (event: CustomEvent) => {
      controllerRef.current?.handleRotation(event.detail);
    };

    const handleButtons = (event: CustomEvent) => {
      controllerRef.current?.handleButtons(event.detail);
    };

    const handleConnect = (event: CustomEvent) => {
      setIsConnected(true);
      controllerRef.current?.start();
      onConnect?.();
    };

    const handleDisconnect = () => {
      setIsConnected(false);
      controllerRef.current?.stop();
      onDisconnect?.();
    };

    // Add event listeners
    SpaceDriver.addEventListener('translate', handleTranslate);
    SpaceDriver.addEventListener('rotate', handleRotate);
    SpaceDriver.addEventListener('buttons', handleButtons);
    SpaceDriver.addEventListener('connect', handleConnect);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);

    // Check if already connected
    // Note: SpaceDriver doesn't expose device property, so we can't check initial state
    // The connect event will fire if a device is already connected

    return () => {
      SpaceDriver.removeEventListener('translate', handleTranslate);
      SpaceDriver.removeEventListener('rotate', handleRotate);
      SpaceDriver.removeEventListener('buttons', handleButtons);
      SpaceDriver.removeEventListener('connect', handleConnect);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
    };
  }, [enabled, isSupported, onConnect, onDisconnect]);

  // Connect to device
  const connect = useCallback(async () => {
    if (!isSupported) {
      console.error('WebHID is not supported in this browser');
      return;
    }

    try {
      await SpaceDriver.scan();
    } catch (error) {
      console.error('Failed to connect to SpacePilot:', error);
    }
  }, [isSupported]);

  // Disconnect from device
  const disconnect = useCallback(() => {
    SpaceDriver.disconnect();
  }, []);

  // Calibrate (reset smoothing buffers)
  const calibrate = useCallback(() => {
    controllerRef.current?.stop();
    controllerRef.current?.start();
  }, []);

  // Reset view
  const resetView = useCallback(() => {
    if (controls && 'reset' in controls) {
      (controls as OrbitControls).reset();
    } else if (camera) {
      camera.position.set(0, 10, 20);
      camera.lookAt(0, 0, 0);
    }
  }, [camera, controls]);

  // Set control mode
  const setMode = useCallback((mode: 'camera' | 'object' | 'navigation') => {
    setCurrentMode(mode);
    controllerRef.current?.setMode(mode);
    onModeChange?.(mode);
    
    // Save to settings store
    const updateSettings = useSettingsStore.getState().updateSettings;
    updateSettings({
      visualisation: {
        spacePilot: {
          mode
        }
      }
    });
  }, [onModeChange]);

  // Update configuration
  const updateConfig = useCallback((newConfig: Partial<SpacePilotConfig>) => {
    const mergedConfig = { ...configRef.current, ...newConfig };
    configRef.current = mergedConfig;
    controllerRef.current?.updateConfig(mergedConfig);
    
    // Save to settings store
    const updateSettings = useSettingsStore.getState().updateSettings;
    updateSettings({
      visualisation: {
        spacePilot: mergedConfig
      }
    });
  }, []);

  return {
    isConnected,
    isSupported,
    currentMode,
    setMode,
    connect,
    disconnect,
    calibrate,
    resetView,
    updateConfig,
    config: configRef.current
  };
}