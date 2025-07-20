import React, { createContext, useContext, useEffect, useState, ReactNode, useCallback, useRef } from 'react';
import { useThree, useFrame } from '@react-three/fiber';
import { XRSessionManager } from '../managers/xrSessionManager';
import { useSettingsStore } from '../../../store/settingsStore';
import { createLogger } from '../../../utils/logger';
import { debugState } from '../../../utils/debugState';
import * as THREE from 'three';
import { XRSettings } from '../types/xr';
import { XRControllerEvent } from '../managers/xrSessionManager';

const logger = createLogger('XRCoreProvider');

// Enhanced XR Context interface with complete session state and interactions
interface XRCoreContextProps {
  isXRCapable: boolean;
  isXRSupported: boolean;
  isSessionActive: boolean;
  sessionType: XRSessionMode | null;
  isPresenting: boolean;
  controllers: THREE.XRTargetRaySpace[];
  controllerGrips: THREE.Object3D[];
  handsVisible: boolean;
  handTrackingEnabled: boolean;
  sessionManager: XRSessionManager | null;
  // Teleportation state
  isTeleporting: boolean;
  teleportPosition: THREE.Vector3 | null;
  // Session management methods
  startSession: (mode?: XRSessionMode) => Promise<void>;
  endSession: () => Promise<void>;
  // Event subscription methods
  onSessionStart: (callback: (session: XRSession) => void) => () => void;
  onSessionEnd: (callback: () => void) => () => void;
  onControllerConnect: (callback: (controller: THREE.XRTargetRaySpace) => void) => () => void;
  onControllerDisconnect: (callback: (controller: THREE.XRTargetRaySpace) => void) => () => void;
  // XR settings
  updateXRSettings: (settings: XRSettings) => void;
}

const XRCoreContext = createContext<XRCoreContextProps>({
  isXRCapable: false,
  isXRSupported: false,
  isSessionActive: false,
  sessionType: null,
  isPresenting: false,
  controllers: [],
  controllerGrips: [],
  handsVisible: false,
  handTrackingEnabled: false,
  sessionManager: null,
  isTeleporting: false,
  teleportPosition: null,
  startSession: async () => {},
  endSession: async () => {},
  onSessionStart: () => () => {},
  onSessionEnd: () => () => {},
  onControllerConnect: () => () => {},
  onControllerDisconnect: () => () => {},
  updateXRSettings: () => {},
});

export const useXRCore = () => useContext(XRCoreContext);

interface XRCoreProviderProps {
  children: ReactNode;
  renderer?: THREE.WebGLRenderer;
}

// Hook for XR interactions (teleportation, floor, etc.)
export const useXRInteractions = () => {
  const { scene, camera } = useThree();
  const { isTeleporting, teleportPosition, sessionManager } = useXRCore();
  const { settings } = useSettingsStore();
  const xrSettings = settings?.xr;
  
  const floorPlaneRef = useRef<THREE.Mesh | null>(null);
  const teleportMarkerRef = useRef<THREE.Mesh | null>(null);
  const raycasterRef = useRef(new THREE.Raycaster());
  const controllerIntersectionsRef = useRef<Map<THREE.XRTargetRaySpace, THREE.Intersection[]>>(new Map());
  
  // Create floor plane
  useEffect(() => {
    if (!xrSettings?.showFloor) {
      if (floorPlaneRef.current) {
        scene.remove(floorPlaneRef.current);
        floorPlaneRef.current.geometry.dispose();
        (floorPlaneRef.current.material as THREE.Material).dispose();
        floorPlaneRef.current = null;
      }
      return;
    }
    
    if (!floorPlaneRef.current) {
      const geometry = new THREE.PlaneGeometry(20, 20);
      const material = new THREE.MeshBasicMaterial({
        color: 0x808080,
        transparent: true,
        opacity: 0.2,
        side: THREE.DoubleSide
      });
      
      floorPlaneRef.current = new THREE.Mesh(geometry, material);
      floorPlaneRef.current.rotation.x = -Math.PI / 2;
      floorPlaneRef.current.position.y = 0;
      floorPlaneRef.current.receiveShadow = true;
      floorPlaneRef.current.name = 'xr-floor';
      
      scene.add(floorPlaneRef.current);
      
      if (debugState.isEnabled()) {
        logger.info('XR floor plane created');
      }
    }
  }, [scene, xrSettings?.showFloor]);
  
  // Create teleport marker
  useEffect(() => {
    if (!xrSettings?.teleportEnabled) {
      if (teleportMarkerRef.current) {
        scene.remove(teleportMarkerRef.current);
        teleportMarkerRef.current.geometry.dispose();
        (teleportMarkerRef.current.material as THREE.Material).dispose();
        teleportMarkerRef.current = null;
      }
      return;
    }
    
    if (!teleportMarkerRef.current) {
      const geometry = new THREE.RingGeometry(0.15, 0.2, 32);
      const material = new THREE.MeshBasicMaterial({
        color: 0x00ff00,
        transparent: true,
        opacity: 0.6,
        side: THREE.DoubleSide
      });
      
      teleportMarkerRef.current = new THREE.Mesh(geometry, material);
      teleportMarkerRef.current.rotation.x = -Math.PI / 2;
      teleportMarkerRef.current.visible = false;
      teleportMarkerRef.current.name = 'teleport-marker';
      
      scene.add(teleportMarkerRef.current);
      
      if (debugState.isEnabled()) {
        logger.info('Teleport marker created');
      }
    }
  }, [scene, xrSettings?.teleportEnabled]);
  
  // Update controller interactions
  useFrame(() => {
    if (!sessionManager?.isSessionActive()) return;
    
    const controllers = sessionManager.getControllers();
    
    controllers.forEach(controller => {
      // Initialize raycaster from controller
      const tempMatrix = new THREE.Matrix4();
      tempMatrix.identity().extractRotation(controller.matrixWorld);
      
      raycasterRef.current.ray.origin.setFromMatrixPosition(controller.matrixWorld);
      raycasterRef.current.ray.direction.set(0, 0, -1).applyMatrix4(tempMatrix);
      
      // Store intersections for this controller
      const intersections: THREE.Intersection[] = [];
      
      // Check for floor intersection if teleport is enabled
      if (floorPlaneRef.current && xrSettings?.teleportEnabled) {
        const floorIntersects = raycasterRef.current.intersectObject(floorPlaneRef.current);
        
        if (floorIntersects.length > 0) {
          intersections.push(...floorIntersects);
          
          // Update teleport marker position if currently teleporting
          if (isTeleporting && teleportMarkerRef.current) {
            teleportMarkerRef.current.position.copy(floorIntersects[0].point);
            teleportMarkerRef.current.visible = true;
          }
        } else if (isTeleporting && teleportMarkerRef.current) {
          // Hide marker if not pointing at floor
          teleportMarkerRef.current.visible = false;
        }
      }
      
      // Store intersections for this controller
      controllerIntersectionsRef.current.set(controller, intersections);
    });
  });
  
  // Handle teleportation completion
  useEffect(() => {
    if (!isTeleporting && teleportPosition && teleportMarkerRef.current?.visible) {
      // Get camera position but keep y-height the same
      const cameraPosition = new THREE.Vector3();
      cameraPosition.setFromMatrixPosition(camera.matrixWorld);
      
      // Calculate teleport offset (where we want camera to end up)
      const offsetX = teleportPosition.x - cameraPosition.x;
      const offsetZ = teleportPosition.z - cameraPosition.z;
      
      // Find camera rig/offset parent - in WebXR the camera is often a child of a rig
      let cameraRig = camera.parent;
      if (cameraRig) {
        // Apply offset to camera rig's position
        cameraRig.position.x += offsetX;
        cameraRig.position.z += offsetZ;
      } else {
        // Fallback to moving camera directly if no rig
        camera.position.x += offsetX;
        camera.position.z += offsetZ;
      }
      
      // Hide teleport marker
      if (teleportMarkerRef.current) {
        teleportMarkerRef.current.visible = false;
      }
      
      if (debugState.isDataDebugEnabled()) {
        logger.debug('Teleported to', { x: teleportPosition.x, z: teleportPosition.z });
      }
    }
  }, [isTeleporting, teleportPosition, camera]);
  
  return {
    floorPlane: floorPlaneRef.current,
    teleportMarker: teleportMarkerRef.current,
    controllerIntersections: controllerIntersectionsRef.current,
  };
};

const XRCoreProvider: React.FC<XRCoreProviderProps> = ({ 
  children, 
  renderer: externalRenderer 
}) => {
  // Basic XR capability state
  const [isXRCapable, setIsXRCapable] = useState(false);
  const [isXRSupported, setIsXRSupported] = useState(false);
  
  // Session state
  const [isSessionActive, setIsSessionActive] = useState(false);
  const [sessionType, setSessionType] = useState<XRSessionMode | null>(null);
  const [isPresenting, setIsPresenting] = useState(false);
  
  // Controller and hand tracking state
  const [controllers, setControllers] = useState<THREE.XRTargetRaySpace[]>([]);
  const [controllerGrips, setControllerGrips] = useState<THREE.Object3D[]>([]);
  const [handsVisible, setHandsVisible] = useState(false);
  const [handTrackingEnabled, setHandTrackingEnabled] = useState(false);
  
  // Teleportation state
  const [isTeleporting, setIsTeleporting] = useState(false);
  const [teleportPosition, setTeleportPosition] = useState<THREE.Vector3 | null>(null);
  
  // Session manager and event handlers
  const sessionManagerRef = useRef<XRSessionManager | null>(null);
  const sessionStartCallbacksRef = useRef<Set<(session: XRSession) => void>>(new Set());
  const sessionEndCallbacksRef = useRef<Set<() => void>>(new Set());
  const controllerConnectCallbacksRef = useRef<Set<(controller: THREE.XRTargetRaySpace) => void>>(new Set());
  const controllerDisconnectCallbacksRef = useRef<Set<(controller: THREE.XRTargetRaySpace) => void>>(new Set());
  
  // Controller event handlers
  const controllerSelectStartUnsubscribeRef = useRef<(() => void) | null>(null);
  const controllerSelectEndUnsubscribeRef = useRef<(() => void) | null>(null);
  const controllerSqueezeStartUnsubscribeRef = useRef<(() => void) | null>(null);
  const controllerSqueezeEndUnsubscribeRef = useRef<(() => void) | null>(null);
  
  // Cleanup tracking
  const cleanupFunctionsRef = useRef<Set<() => void>>(new Set());
  
  const { settings } = useSettingsStore();
  const xrSettings = settings?.xr;

  // Initialize XR capability detection (Quest 3 AR focused)
  useEffect(() => {
    const checkXRSupport = async () => {
      try {
        if ('xr' in navigator) {
          // Prioritize AR support for Quest 3
          const arSupported = await (navigator.xr as any).isSessionSupported('immersive-ar');
          
          setIsXRSupported(arSupported);
          setIsXRCapable(true);
          
          if (arSupported) {
            if (settings?.system?.debug?.enabled) {
              logger.info('Quest 3 AR mode detected and supported');
            }
          } else {
            logger.warn('Quest 3 AR mode not supported - immersive-ar session required');
          }
        } else {
          setIsXRCapable(false);
          setIsXRSupported(false);
          logger.warn('WebXR not available - Quest 3 browser required');
        }
      } catch (error) {
        setIsXRCapable(false);
        setIsXRSupported(false);
        logger.error('Error checking XR support:', error);
      }
    };

    checkXRSupport();
  }, [settings?.system?.debug?.enabled]);

  // Handle controller select start event (trigger press)
  const handleControllerSelectStart = useCallback((event: XRControllerEvent) => {
    const { controller } = event;
    
    // Start teleportation if enabled
    if (xrSettings?.teleportEnabled) {
      setIsTeleporting(true);
      
      // We'll let the useXRInteractions hook handle the actual teleport logic
      if (debugState.isDataDebugEnabled()) {
        logger.debug('Started teleportation');
      }
    }
  }, [xrSettings?.teleportEnabled]);
  
  // Handle controller select end event (trigger release)
  const handleControllerSelectEnd = useCallback((event: XRControllerEvent) => {
    // Complete teleportation if in progress
    if (isTeleporting) {
      // The actual teleportation is handled by useXRInteractions hook
      setIsTeleporting(false);
      
      if (debugState.isDataDebugEnabled()) {
        logger.debug('Ended teleportation');
      }
    }
  }, [isTeleporting]);
  
  // Handle controller squeeze start event (grip press)
  const handleControllerSqueezeStart = useCallback((event: XRControllerEvent) => {
    // Placeholder for future interactions
    // Could be used for grabbing objects, scaling the environment, etc.
    if (debugState.isDataDebugEnabled()) {
      logger.debug('Controller squeeze start');
    }
  }, []);
  
  // Handle controller squeeze end event (grip release)
  const handleControllerSqueezeEnd = useCallback((event: XRControllerEvent) => {
    // Placeholder for future interactions
    if (debugState.isDataDebugEnabled()) {
      logger.debug('Controller squeeze end');
    }
  }, []);

  // Initialize session manager when XR is supported and dependencies are available
  useEffect(() => {
    if (!isXRSupported || sessionManagerRef.current) return;

    try {
      // Create default scene and camera if not provided
      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(75, window.innerWidth / window.innerHeight, 0.1, 1000);
      camera.position.z = 5;

      // Initialize XR session manager
      const sessionManager = XRSessionManager.getInstance(externalRenderer, scene, camera);
      sessionManager.initialize(settings);
      sessionManagerRef.current = sessionManager;

      // Set up controller event handlers
      controllerSelectStartUnsubscribeRef.current = sessionManager.onSelectStart(handleControllerSelectStart);
      controllerSelectEndUnsubscribeRef.current = sessionManager.onSelectEnd(handleControllerSelectEnd);
      controllerSqueezeStartUnsubscribeRef.current = sessionManager.onSqueezeStart(handleControllerSqueezeStart);
      controllerSqueezeEndUnsubscribeRef.current = sessionManager.onSqueezeEnd(handleControllerSqueezeEnd);
      
      // Add to cleanup
      cleanupFunctionsRef.current.add(() => {
        controllerSelectStartUnsubscribeRef.current?.();
        controllerSelectEndUnsubscribeRef.current?.();
        controllerSqueezeStartUnsubscribeRef.current?.();
        controllerSqueezeEndUnsubscribeRef.current?.();
      });

      // Set up session event listeners
      const handleSessionStart = () => {
        setIsSessionActive(true);
        setIsPresenting(true);
        
        // Get current session to determine type
        const session = sessionManager.getRenderer()?.xr.getSession();
        if (session) {
          // XRSession doesn't expose mode directly, so we'll track it via the session request
          // For now, we'll detect based on environment blend mode or other properties
          const environmentBlendMode = session.environmentBlendMode;
          if (environmentBlendMode === 'additive' || environmentBlendMode === 'alpha-blend') {
            setSessionType('immersive-ar');
          } else {
            setSessionType('immersive-vr');
          }
          
          // Notify callbacks
          sessionStartCallbacksRef.current.forEach(callback => {
            try {
              callback(session);
            } catch (error) {
              logger.error('Error in session start callback:', error);
            }
          });
        }
        
        logger.info('XR session started');
      };

      const handleSessionEnd = () => {
        // Clean up all XR resources
        performCompleteCleanup();
        
        setIsSessionActive(false);
        setIsPresenting(false);
        setSessionType(null);
        setControllers([]);
        setControllerGrips([]);
        setHandsVisible(false);
        setHandTrackingEnabled(false);
        setIsTeleporting(false);
        setTeleportPosition(null);
        
        // Notify callbacks
        sessionEndCallbacksRef.current.forEach(callback => {
          try {
            callback();
          } catch (error) {
            logger.error('Error in session end callback:', error);
          }
        });
        
        logger.info('XR session ended and resources cleaned up');
      };

      // Get renderer for event subscription
      const renderer = sessionManager.getRenderer();
      if (renderer) {
        renderer.xr.addEventListener('sessionstart', handleSessionStart);
        renderer.xr.addEventListener('sessionend', handleSessionEnd);
        
        // Store cleanup functions
        cleanupFunctionsRef.current.add(() => {
          renderer.xr.removeEventListener('sessionstart', handleSessionStart);
          renderer.xr.removeEventListener('sessionend', handleSessionEnd);
        });
      }

      // Set up controller tracking
      const updateControllerState = () => {
        if (sessionManager) {
          setControllers([...sessionManager.getControllers()]);
          setControllerGrips([...sessionManager.getControllerGrips()]);
        }
      };

      // Update controller state periodically during session
      const controllerUpdateInterval = setInterval(() => {
        if (isSessionActive) {
          updateControllerState();
        }
      }, 100);

      cleanupFunctionsRef.current.add(() => {
        clearInterval(controllerUpdateInterval);
      });

      logger.info('XR Core Provider initialized successfully');
    } catch (error) {
      logger.error('Failed to initialize XR session manager:', error);
    }
  }, [isXRSupported, settings, externalRenderer, handleControllerSelectStart, handleControllerSelectEnd, handleControllerSqueezeStart, handleControllerSqueezeEnd]);

  // Complete cleanup function
  const performCompleteCleanup = useCallback(() => {
    try {
      // Run all registered cleanup functions
      cleanupFunctionsRef.current.forEach(cleanup => {
        try {
          cleanup();
        } catch (error) {
          logger.error('Error during cleanup:', error);
        }
      });
      
      // Dispose session manager resources
      if (sessionManagerRef.current) {
        sessionManagerRef.current.dispose();
      }
      
      // Clear event handler sets
      sessionStartCallbacksRef.current.clear();
      sessionEndCallbacksRef.current.clear();
      controllerConnectCallbacksRef.current.clear();
      controllerDisconnectCallbacksRef.current.clear();
      cleanupFunctionsRef.current.clear();
      
      logger.info('Complete XR resource cleanup performed');
    } catch (error) {
      logger.error('Error during complete cleanup:', error);
    }
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      performCompleteCleanup();
    };
  }, [performCompleteCleanup]);

  // Session management methods (Quest 3 AR focused)
  const startSession = useCallback(async (mode: XRSessionMode = 'immersive-ar') => {
    if (!sessionManagerRef.current || !isXRSupported) {
      throw new Error('Quest 3 AR not supported or session manager not initialized');
    }

    try {
      // Quest 3 AR optimized session configuration
      const sessionInit: XRSessionInit = {
        requiredFeatures: ['local-floor'],
        optionalFeatures: [
          'hand-tracking',      // Quest 3 hand tracking
          'hit-test',           // AR hit testing
          'anchors',            // AR anchors
          'plane-detection',    // Quest 3 plane detection
          'light-estimation',   // AR lighting estimation
          'depth-sensing',      // Quest 3 depth sensing
          'mesh-detection'      // Quest 3 mesh detection
        ],
      };

      if (settings?.system?.debug?.enabled) {
        logger.info(`Starting Quest 3 ${mode} session with AR features`);
      }

      const session = await navigator.xr!.requestSession(mode, sessionInit);
      const renderer = sessionManagerRef.current.getRenderer();
      if (renderer) {
        await renderer.xr.setSession(session);
      }
    } catch (error) {
      logger.error(`Failed to start Quest 3 ${mode} session:`, error);
      throw error;
    }
  }, [isXRSupported, settings?.system?.debug?.enabled]);

  const endSession = useCallback(async () => {
    if (!sessionManagerRef.current) return;

    try {
      const renderer = sessionManagerRef.current.getRenderer();
      const session = renderer?.xr.getSession();
      if (session) {
        await session.end();
      }
    } catch (error) {
      logger.error('Failed to end XR session:', error);
      throw error;
    }
  }, []);

  // Event subscription methods
  const onSessionStart = useCallback((callback: (session: XRSession) => void) => {
    sessionStartCallbacksRef.current.add(callback);
    return () => {
      sessionStartCallbacksRef.current.delete(callback);
    };
  }, []);

  const onSessionEnd = useCallback((callback: () => void) => {
    sessionEndCallbacksRef.current.add(callback);
    return () => {
      sessionEndCallbacksRef.current.delete(callback);
    };
  }, []);

  const onControllerConnect = useCallback((callback: (controller: THREE.XRTargetRaySpace) => void) => {
    controllerConnectCallbacksRef.current.add(callback);
    return () => {
      controllerConnectCallbacksRef.current.delete(callback);
    };
  }, []);

  const onControllerDisconnect = useCallback((callback: (controller: THREE.XRTargetRaySpace) => void) => {
    controllerDisconnectCallbacksRef.current.add(callback);
    return () => {
      controllerDisconnectCallbacksRef.current.delete(callback);
    };
  }, []);
  
  // Update XR settings
  const updateXRSettings = useCallback((newSettings: XRSettings) => {
    if (sessionManagerRef.current) {
      sessionManagerRef.current.updateSettings({ ...settings, xr: newSettings });
    }
  }, [settings]);

  const contextValue: XRCoreContextProps = {
    isXRCapable,
    isXRSupported,
    isSessionActive,
    sessionType,
    isPresenting,
    controllers,
    controllerGrips,
    handsVisible,
    handTrackingEnabled,
    sessionManager: sessionManagerRef.current,
    isTeleporting,
    teleportPosition,
    startSession,
    endSession,
    onSessionStart,
    onSessionEnd,
    onControllerConnect,
    onControllerDisconnect,
    updateXRSettings,
  };

  return (
    <XRCoreContext.Provider value={contextValue}>
      {children}
    </XRCoreContext.Provider>
  );
};

export default XRCoreProvider;