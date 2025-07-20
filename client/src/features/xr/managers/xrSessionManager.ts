import * as THREE from 'three';
import { VRButton } from 'three/examples/jsm/webxr/VRButton.js';
import { XRControllerModelFactory } from 'three/examples/jsm/webxr/XRControllerModelFactory.js';
import { createLogger, createErrorMetadata } from '@/utils/logger';
import { debugState } from '@/utils/debugState'; // Assuming debugState.ts exists in utils
import { SceneManager } from '@/features/visualisation/managers/sceneManager'; // Correct path
import { GestureRecognitionResult } from '@/features/xr/systems/HandInteractionSystem'; // Correct path
import { Settings } from '@/features/settings/config/settings'; // Correct path, assuming Settings is defined here

const logger = createLogger('XRSessionManager');

export interface XRControllerEvent {
  controller: THREE.XRTargetRaySpace;
  inputSource: XRInputSource;
  data?: any;
}

type XRControllerEventHandler = (event: XRControllerEvent) => void;

// New event handler types for hand interactions
type GestureEventHandler = (gesture: GestureRecognitionResult) => void;
type HandVisibilityHandler = (visible: boolean) => void;
type XRSessionStateHandler = (state: string) => void;
type HandTrackingHandler = (enabled: boolean) => void;

export class XRSessionManager {
  private static instance: XRSessionManager;
  private sceneManager: SceneManager;
  private renderer: THREE.WebGLRenderer | null = null;
  private camera: THREE.PerspectiveCamera | null = null;
  private scene: THREE.Scene | null = null;
  private controllers: THREE.XRTargetRaySpace[] = [];
  private controllerGrips: THREE.Object3D[] = [];
  private controllerModelFactory: XRControllerModelFactory | null = null;
  private vrButton: HTMLElement | null = null;
  private sessionActive: boolean = false;
  private settings: Settings | null = null;
  
  // Event handlers
  private selectStartHandlers: XRControllerEventHandler[] = [];
  private selectEndHandlers: XRControllerEventHandler[] = [];
  private squeezeStartHandlers: XRControllerEventHandler[] = [];
  private squeezeEndHandlers: XRControllerEventHandler[] = [];
  
  // New event handlers for hand interactions
  private gestureRecognizedHandlers: GestureEventHandler[] = [];
  private handsVisibilityChangedHandlers: HandVisibilityHandler[] = [];
  private handTrackingStateHandlers: HandTrackingHandler[] = [];
  
  private constructor(sceneManager: SceneManager, externalRenderer?: THREE.WebGLRenderer) {
    this.sceneManager = sceneManager;    
    // Allow using an external renderer (from React Three Fiber) or try to get one from SceneManager
    this.renderer = externalRenderer || sceneManager.getRenderer();
    
    // Get camera and ensure it's a PerspectiveCamera
    const camera = sceneManager.getCamera();
    if (!camera || !(camera instanceof THREE.PerspectiveCamera)) {
      logger.warn('PerspectiveCamera not available from SceneManager, creating default camera');
      this.camera = new THREE.PerspectiveCamera(75, window.innerWidth / window.innerHeight, 0.1, 1000);
      this.camera.position.z = 5;
    } else {
      this.camera = camera as THREE.PerspectiveCamera;
    }
    
    // Get scene
    this.scene = sceneManager.getScene();
    if (!this.scene) {
      logger.warn('Scene not found in SceneManager, creating default scene');
      this.scene = new THREE.Scene();
    }
    
    // Log warning instead of throwing error so application can continue
    if (!this.renderer) {
      logger.warn('XRSessionManager: No renderer provided. XR functionality will be limited.');
    }
    
    try {
      // Initialize controller model factory
      this.controllerModelFactory = new XRControllerModelFactory();
    } catch (error) {
      logger.error('Failed to create XRControllerModelFactory:', createErrorMetadata(error));
      this.controllerModelFactory = null;
    }
  }
  
  public static getInstance(sceneManager: SceneManager, externalRenderer?: THREE.WebGLRenderer): XRSessionManager {
    if (!XRSessionManager.instance) {
      XRSessionManager.instance = new XRSessionManager(sceneManager, externalRenderer);
    } else if (externalRenderer && !XRSessionManager.instance.renderer) {
      // If instance exists but has no renderer, we can update it with the external renderer
      XRSessionManager.instance.renderer = externalRenderer;
      logger.info('Updated XRSessionManager with external renderer');
    }
    return XRSessionManager.instance;
  }
  
  public initialize(settings: Settings): void {
    if (!this.renderer || !this.scene) {
      logger.error('Cannot initialize XR: renderer or scene is missing');
      return;
    }
    
    this.settings = settings;
    
    try {
      // Check if WebXR is supported
      if ('xr' in navigator && this.renderer) {
        // Set up renderer for XR
        this.renderer.xr.enabled = true;
        
        // Set reference space type based on settings (assuming teleport implies room scale)
        const refSpace = settings.xr?.locomotionMethod === 'teleport' ? 'local-floor' : 'local';
        this.renderer.xr.setReferenceSpaceType(refSpace);
        
        if (debugState.isEnabled()) {
          logger.info(`Set XR reference space to ${refSpace}`);
        }
        
        // Create VR button
        this.createVRButton();
        
        // Create controllers
        this.setupControllers();
        
        if (debugState.isEnabled()) {
          logger.info('XR session manager initialized successfully');
        }
      } else if (debugState.isEnabled()) {
        logger.warn('WebXR not supported in this browser');
      }
    } catch (error) {
      logger.error('Failed to initialize XR:', createErrorMetadata(error));
    }
  }
  
  private createVRButton(): void {
    if (!this.renderer || !navigator.xr) {
      logger.warn('WebXR not supported or renderer not available for VR button creation.');
      return;
    }

    const button = document.createElement('button');
    button.id = 'xr-button';
    button.style.position = 'absolute';
    button.style.bottom = '20px';
    button.style.right = '20px';
    button.style.padding = '12px 24px';
    button.style.border = '1px solid #fff';
    button.style.borderRadius = '4px';
    button.style.background = 'rgba(0,0,0,0.5)';
    button.style.color = '#fff';
    button.style.font = 'normal 18px sans-serif';
    button.style.textAlign = 'center';
    button.style.opacity = '0.7';
    button.style.outline = 'none';
    button.style.zIndex = '100';
    button.style.cursor = 'pointer';

    const showEnterXR = (supported: boolean, modeText: string) => {
      button.textContent = supported ? `ENTER ${modeText}` : `${modeText} NOT SUPPORTED`;
      button.disabled = !supported;
    };

    const showExitXR = () => {
      button.textContent = 'EXIT XR';
      button.disabled = false;
    };
    
    const currentRenderer = this.renderer; // Capture renderer for async operations

    const startSession = async (mode: XRSessionMode, sessionInit: XRSessionInit = {}) => {
      try {
        const session = await navigator.xr!.requestSession(mode, sessionInit);
        await currentRenderer.xr.setSession(session);
        // showExitXR() will be called by sessionstart listener
      } catch (e) {
        logger.error(`Failed to start ${mode} session:`, createErrorMetadata(e));
        // Attempt to reset button text to a valid enter state
        const arSupported = await navigator.xr!.isSessionSupported('immersive-ar').catch(() => false);
        const vrSupported = await navigator.xr!.isSessionSupported('immersive-vr').catch(() => false);
        if (this.vrButton && !currentRenderer.xr.isPresenting) { // Check if button still exists and not presenting
            if (arSupported) showEnterXR(true, 'AR');
            else if (vrSupported) showEnterXR(true, 'VR');
            else showEnterXR(false, 'XR');
        }
      }
    };

    button.onclick = async () => {
      if (currentRenderer.xr.isPresenting) {
        try {
          // sessionend event will trigger button text update via listener
          await currentRenderer.xr.getSession()?.end();
        } catch (e) {
          logger.error('Failed to end XR session:', createErrorMetadata(e));
        }
      } else {
        try {
          const arSupported = await navigator.xr!.isSessionSupported('immersive-ar');
          if (arSupported) {
            logger.info('Attempting to start AR session.');
            await startSession('immersive-ar', {
              requiredFeatures: ['local-floor'],
              optionalFeatures: ['hand-tracking', 'hit-test', 'anchors', 'plane-detection', 'light-estimation'],
            });
          } else {
            logger.info('AR not supported, attempting to start VR session.');
            const vrSupported = await navigator.xr!.isSessionSupported('immersive-vr');
            if (vrSupported) {
              await startSession('immersive-vr', {
                requiredFeatures: ['local-floor'],
                optionalFeatures: ['hand-tracking'],
              });
            } else {
              showEnterXR(false, 'XR');
              logger.warn('Neither AR nor VR is supported.');
            }
          }
        } catch (e) {
          logger.error('Error during session support check or start:', createErrorMetadata(e));
          showEnterXR(false, 'XR'); // Ensure button reflects error state
        }
      }
    };

    // Initial button state determination
    const setInitialButtonState = async () => {
        try {
            const arSupported = await navigator.xr!.isSessionSupported('immersive-ar');
            if (arSupported) {
                showEnterXR(true, 'AR');
            } else {
                const vrSupported = await navigator.xr!.isSessionSupported('immersive-vr');
                showEnterXR(vrSupported, vrSupported ? 'VR' : 'XR');
            }
        } catch (e) {
            logger.error('Error checking XR support for initial button state:', createErrorMetadata(e));
            showEnterXR(false, 'XR');
        }
    };
    
    setInitialButtonState(); // Call async function to set initial state
    
    this.vrButton = button;
    document.body.appendChild(this.vrButton);

    // Session event listeners
    currentRenderer.xr.addEventListener('sessionstart', () => {
      this.sessionActive = true;
      showExitXR();
      if (debugState.isEnabled()) {
        logger.info('XR session started. Environment Blend Mode:', currentRenderer.xr.getSession()?.environmentBlendMode);
      }
    });

    currentRenderer.xr.addEventListener('sessionend', () => {
      this.sessionActive = false;
      // Reset button to initial state after a short delay
      setTimeout(() => {
        if (this.vrButton && !currentRenderer.xr.isPresenting) { // Check button exists and not already re-entered XR
            setInitialButtonState();
        }
      }, 100);
      if (debugState.isEnabled()) {
        logger.info('XR session ended');
      }
    });
  }
  
  private setupControllers(): void {
    if (!this.renderer || !this.scene) return;
    
    try {
      // Create controllers
      for (let i = 0; i < 2; i++) {
        // Controller
        const controller = this.renderer.xr.getController(i);
        controller.addEventListener('selectstart', (event) => this.handleSelectStart(event, i));
        controller.addEventListener('selectend', (event) => this.handleSelectEnd(event, i));
        controller.addEventListener('squeezestart', (event) => this.handleSqueezeStart(event, i));
        controller.addEventListener('squeezeend', (event) => this.handleSqueezeEnd(event, i));
        controller.addEventListener('connected', (event) => {
          if (debugState.isEnabled()) {
            logger.info(`Controller ${i} connected:`, { 
              handedness: (event as any).data?.handedness,
              targetRayMode: (event as any).data?.targetRayMode
            });
          }
        });
        controller.addEventListener('disconnected', () => {
          if (debugState.isEnabled()) {
            logger.info(`Controller ${i} disconnected`);
          }
        });
        
        this.scene.add(controller);
        this.controllers.push(controller as THREE.XRTargetRaySpace);
        
        // Controller grip
        const controllerGrip = this.renderer.xr.getControllerGrip(i);
        if (this.controllerModelFactory) {
          controllerGrip.add(this.controllerModelFactory.createControllerModel(controllerGrip));
        }
        this.scene.add(controllerGrip);
        this.controllerGrips.push(controllerGrip);
        
        // Add visual indicators for the controllers
        const geometry = new THREE.BufferGeometry().setFromPoints([
          new THREE.Vector3(0, 0, 0),
          new THREE.Vector3(0, 0, -1)
        ]);
        
        const line = new THREE.Line(geometry);
        line.name = 'controller-line';
        line.scale.z = 5;
        
        controller.add(line);
        controller.userData.selectPressed = false;
        controller.userData.squeezePressed = false;
      }
      
      if (debugState.isEnabled()) {
        logger.info('XR controllers set up successfully');
      }
    } catch (error) {
      logger.error('Failed to set up XR controllers:', createErrorMetadata(error));
    }
  }
  
  // Event handlers
  private handleSelectStart(event: any, controllerId: number): void {
    if (controllerId >= this.controllers.length) return;
    
    const controller = this.controllers[controllerId];
    controller.userData.selectPressed = true;
    
    const inputSource = event.data;
    if (debugState.isDataDebugEnabled()) {
      logger.debug(`Controller ${controllerId} select start`);
    }
    
    this.selectStartHandlers.forEach(handler => {
      try {
        handler({ controller, inputSource, data: event.data });
      } catch (error) {
        logger.error('Error in selectStart handler:', createErrorMetadata(error));
      }
    });
  }
  
  private handleSelectEnd(event: any, controllerId: number): void {
    if (controllerId >= this.controllers.length) return;
    
    const controller = this.controllers[controllerId];
    controller.userData.selectPressed = false;
    
    const inputSource = event.data;
    if (debugState.isDataDebugEnabled()) {
      logger.debug(`Controller ${controllerId} select end`);
    }
    
    this.selectEndHandlers.forEach(handler => {
      try {
        handler({ controller, inputSource, data: event.data });
      } catch (error) {
        logger.error('Error in selectEnd handler:', createErrorMetadata(error));
      }
    });
  }
  
  private handleSqueezeStart(event: any, controllerId: number): void {
    if (controllerId >= this.controllers.length) return;
    
    const controller = this.controllers[controllerId];
    controller.userData.squeezePressed = true;
    
    const inputSource = event.data;
    if (debugState.isDataDebugEnabled()) {
      logger.debug(`Controller ${controllerId} squeeze start`);
    }
    
    this.squeezeStartHandlers.forEach(handler => {
      try {
        handler({ controller, inputSource, data: event.data });
      } catch (error) {
        logger.error('Error in squeezeStart handler:', createErrorMetadata(error));
      }
    });
  }
  
  private handleSqueezeEnd(event: any, controllerId: number): void {
    if (controllerId >= this.controllers.length) return;
    
    const controller = this.controllers[controllerId];
    controller.userData.squeezePressed = false;
    
    const inputSource = event.data;
    if (debugState.isDataDebugEnabled()) {
      logger.debug(`Controller ${controllerId} squeeze end`);
    }
    
    this.squeezeEndHandlers.forEach(handler => {
      try {
        handler({ controller, inputSource, data: event.data });
      } catch (error) {
        logger.error('Error in squeezeEnd handler:', createErrorMetadata(error));
      }
    });
  }
  
  // Event subscription methods
  public onSelectStart(handler: XRControllerEventHandler): () => void {
    this.selectStartHandlers.push(handler);
    return () => {
      this.selectStartHandlers = this.selectStartHandlers.filter(h => h !== handler);
    };
  }
  
  public onSelectEnd(handler: XRControllerEventHandler): () => void {
    this.selectEndHandlers.push(handler);
    return () => {
      this.selectEndHandlers = this.selectEndHandlers.filter(h => h !== handler);
    };
  }
  
  public onSqueezeStart(handler: XRControllerEventHandler): () => void {
    this.squeezeStartHandlers.push(handler);
    return () => {
      this.squeezeStartHandlers = this.squeezeStartHandlers.filter(h => h !== handler);
    };
  }
  
  public onSqueezeEnd(handler: XRControllerEventHandler): () => void {
    this.squeezeEndHandlers.push(handler);
    return () => {
      this.squeezeEndHandlers = this.squeezeEndHandlers.filter(h => h !== handler);
    };
  }

  // New event subscription methods for hand interactions
  public onGestureRecognized(handler: GestureEventHandler): () => void {
    this.gestureRecognizedHandlers.push(handler);
    return () => {
      this.gestureRecognizedHandlers = this.gestureRecognizedHandlers.filter(h => h !== handler);
    };
  }
  
  public onHandsVisibilityChanged(handler: HandVisibilityHandler): () => void {
    this.handsVisibilityChangedHandlers.push(handler);
    return () => {
      this.handsVisibilityChangedHandlers = this.handsVisibilityChangedHandlers.filter(h => h !== handler);
    };
  }
  
  // Method to notify gesture events
  public notifyGestureRecognized(gesture: GestureRecognitionResult): void {
    this.gestureRecognizedHandlers.forEach(handler => {
      try {
        handler(gesture);
      } catch (error) {
        logger.error('Error in gesture recognition handler:', createErrorMetadata(error));
      }
    });
  }
  
  // Method to notify hand visibility changes
  public notifyHandsVisibilityChanged(visible: boolean): void {
    this.handsVisibilityChangedHandlers.forEach(handler => {
      try {
        handler(visible);
      } catch (error) {
        logger.error('Error in hand visibility handler:', createErrorMetadata(error));
      }
    });
  }
  
  // XR state methods
  public isSessionActive(): boolean {
    return this.sessionActive;
  }
  
  public getControllers(): THREE.XRTargetRaySpace[] {
    return this.controllers;
  }
  
  public getControllerGrips(): THREE.Object3D[] {
    return this.controllerGrips;
  }
  
  public getRenderer(): THREE.WebGLRenderer | null {
    return this.renderer;
  }
  
  public updateSettings(settings: Settings): void {
    this.settings = settings;
    
    // Update reference space if settings changed
    if (this.renderer && settings.xr) {
      this.renderer.xr.setReferenceSpaceType(
        settings.xr.locomotionMethod === 'teleport' ? 'local-floor' : 'local'
      );
    }
  }
  
  public dispose(): void {
    try {
      // End active session first
      if (this.sessionActive && this.renderer?.xr.isPresenting) {
        const session = this.renderer.xr.getSession();
        if (session) {
          session.end().catch(error => {
            logger.error('Error ending XR session during disposal:', error);
          });
        }
      }

      // Remove controllers from scene and clear event listeners
      this.controllers.forEach((controller, index) => {
        try {
          // Remove from scene
          if (controller.parent) {
            controller.removeFromParent();
          }
          
          // Clear controller-specific data
          if (controller.userData) {
            controller.userData.selectPressed = false;
            controller.userData.squeezePressed = false;
          }
          
          // Remove visual indicators (lines, etc.)
          const line = controller.getObjectByName('controller-line');
          if (line) {
            controller.remove(line);
            // Properly dispose of geometry and material if it's a Line object
            if (line instanceof THREE.Line) {
              if (line.geometry) line.geometry.dispose();
              if (line.material) {
                if (Array.isArray(line.material)) {
                  line.material.forEach(mat => mat.dispose());
                } else {
                  line.material.dispose();
                }
              }
            }
          }
        } catch (error) {
          logger.error(`Error disposing controller ${index}:`, error);
        }
      });
      
      // Remove controller grips and their models
      this.controllerGrips.forEach((grip, index) => {
        try {
          if (grip.parent) {
            grip.removeFromParent();
          }
          
          // Dispose of controller models created by XRControllerModelFactory
          grip.traverse((child) => {
            if (child instanceof THREE.Mesh) {
              if (child.geometry) child.geometry.dispose();
              if (child.material) {
                if (Array.isArray(child.material)) {
                  child.material.forEach(mat => mat.dispose());
                } else {
                  child.material.dispose();
                }
              }
            }
          });
        } catch (error) {
          logger.error(`Error disposing controller grip ${index}:`, error);
        }
      });
      
      // Remove VR button safely
      if (this.vrButton) {
        try {
          if (this.vrButton.parentNode) {
            this.vrButton.parentNode.removeChild(this.vrButton);
          }
        } catch (error) {
          logger.error('Error removing VR button:', error);
        }
      }
      
      // Clear all event listeners and handler arrays
      this.selectStartHandlers.length = 0;
      this.selectEndHandlers.length = 0;
      this.squeezeStartHandlers.length = 0;
      this.squeezeEndHandlers.length = 0;
      this.gestureRecognizedHandlers.length = 0;
      this.handsVisibilityChangedHandlers.length = 0;
      this.handTrackingStateHandlers.length = 0;
      
      // Clear arrays
      this.controllers.length = 0;
      this.controllerGrips.length = 0;
      
      // Clear factory reference
      this.controllerModelFactory = null;
      
      // Clear renderer XR session listeners if renderer exists
      if (this.renderer?.xr) {
        try {
          // Note: We can't remove specific listeners without references,
          // but setting to null will prevent new events from being processed
          this.renderer.xr.enabled = false;
        } catch (error) {
          logger.error('Error disabling XR on renderer:', error);
        }
      }
      
      // Clear object references
      this.renderer = null;
      this.camera = null;
      this.scene = null;
      this.vrButton = null;
      this.settings = null;
      
      // Reset state flags
      this.sessionActive = false;
      
      if (debugState.isEnabled()) {
        logger.info('XR session manager disposed with complete resource cleanup');
      }
    } catch (error) {
      logger.error('Error during XR session manager disposal:', error);
    }
  }
}