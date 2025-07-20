import React, { createContext, useContext, useEffect, useState, ReactNode, useCallback, useRef } from 'react';
import { XRSessionManager } from '../managers/xrSessionManager';
import { SceneManager } from '../../visualisation/managers/sceneManager';
import { useSettingsStore } from '../../../store/settingsStore';
import { createLogger } from '../../../utils/logger';
import * as THREE from 'three';

const logger = createLogger('XRCoreProvider');

// Enhanced XR Context interface with complete session state
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
  // Session management methods
  startSession: (mode?: XRSessionMode) => Promise<void>;
  endSession: () => Promise<void>;
  // Event subscription methods
  onSessionStart: (callback: (session: XRSession) => void) => () => void;
  onSessionEnd: (callback: () => void) => () => void;
  onControllerConnect: (callback: (controller: THREE.XRTargetRaySpace) => void) => () => void;
  onControllerDisconnect: (callback: (controller: THREE.XRTargetRaySpace) => void) => () => void;
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
  startSession: async () => {},
  endSession: async () => {},
  onSessionStart: () => () => {},
  onSessionEnd: () => () => {},
  onControllerConnect: () => () => {},
  onControllerDisconnect: () => () => {},
});

export const useXRCore = () => useContext(XRCoreContext);

interface XRCoreProviderProps {
  children: ReactNode;
  sceneManager?: SceneManager;
  renderer?: THREE.WebGLRenderer;
}

const XRCoreProvider: React.FC<XRCoreProviderProps> = ({ 
  children, 
  sceneManager: externalSceneManager, 
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
  
  // Session manager and event handlers
  const sessionManagerRef = useRef<XRSessionManager | null>(null);
  const sessionStartCallbacksRef = useRef<Set<(session: XRSession) => void>>(new Set());
  const sessionEndCallbacksRef = useRef<Set<() => void>>(new Set());
  const controllerConnectCallbacksRef = useRef<Set<(controller: THREE.XRTargetRaySpace) => void>>(new Set());
  const controllerDisconnectCallbacksRef = useRef<Set<(controller: THREE.XRTargetRaySpace) => void>>(new Set());
  
  // Cleanup tracking
  const cleanupFunctionsRef = useRef<Set<() => void>>(new Set());
  
  const { settings } = useSettingsStore();

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

  // Initialize session manager when XR is supported and dependencies are available
  useEffect(() => {
    if (!isXRSupported || sessionManagerRef.current) return;

    try {
      // Use external scene manager or create a default one
      let sceneManager = externalSceneManager;
      if (!sceneManager) {
        // Create a minimal scene manager for XR if none provided
        logger.warn('No SceneManager provided, XR functionality may be limited');
        return;
      }

      // Initialize XR session manager
      const sessionManager = XRSessionManager.getInstance(sceneManager, externalRenderer);
      sessionManager.initialize(settings);
      sessionManagerRef.current = sessionManager;

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
  }, [isXRSupported, settings, externalSceneManager, externalRenderer]);

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
    startSession,
    endSession,
    onSessionStart,
    onSessionEnd,
    onControllerConnect,
    onControllerDisconnect,
  };

  return (
    <XRCoreContext.Provider value={contextValue}>
      {children}
    </XRCoreContext.Provider>
  );
};

export default XRCoreProvider;