import * as THREE from 'three';
import { createLogger, createErrorMetadata } from '@/utils/logger';
import { debugState } from '@/utils/debugState';
import { XRSessionManager, XRControllerEvent } from './xrSessionManager';
import { XRSettings } from '../types/xr';
import { SceneManager } from '../../visualisation/managers/sceneManager';

const logger = createLogger('XRInitializer');

export class XRInitializer {
  private static instance: XRInitializer;
  private xrSessionManager: XRSessionManager;
  private sceneManager: SceneManager;
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private teleportMarker: THREE.Mesh | null = null;
  private floorPlane: THREE.Mesh | null = null;
  private settings: XRSettings | null = null;
  private raycaster: THREE.Raycaster = new THREE.Raycaster();
  private controllerIntersections: Map<THREE.XRTargetRaySpace, THREE.Intersection[]> = new Map();
  
  // Teleportation state
  private isTeleporting: boolean = false;
  private teleportPosition: THREE.Vector3 = new THREE.Vector3();
  
  // Movement state
  private movementEnabled: boolean = true;
  private movementSpeed: number = 1.0;
  
  // Controller handlers
  private controllerSelectStartUnsubscribe: (() => void) | null = null;
  private controllerSelectEndUnsubscribe: (() => void) | null = null;
  private controllerSqueezeStartUnsubscribe: (() => void) | null = null;
  private controllerSqueezeEndUnsubscribe: (() => void) | null = null;
  
  private constructor(xrSessionManager: XRSessionManager) {
    this.xrSessionManager = xrSessionManager;
    this.sceneManager = SceneManager.getInstance();
    this.scene = this.sceneManager.getScene();
    
    // Get camera and ensure it's a PerspectiveCamera
    const camera = this.sceneManager.getCamera();
    
    if (!camera || !(camera instanceof THREE.PerspectiveCamera)) {
      logger.warn('PerspectiveCamera not available from SceneManager, creating default camera');
      this.camera = new THREE.PerspectiveCamera(75, window.innerWidth / window.innerHeight, 0.1, 1000);
      this.camera.position.z = 5;
    } else {
      // We have a valid PerspectiveCamera
      this.camera = camera as THREE.PerspectiveCamera;
    }
    
    // Setup XR interactions
    this.setupXRInteractions();
  }
  
  public static getInstance(xrSessionManager: XRSessionManager): XRInitializer {
    if (!XRInitializer.instance) {
      XRInitializer.instance = new XRInitializer(xrSessionManager);
    }
    return XRInitializer.instance;
  }
  
  // Initialize XR capabilities with current settings
  public initialize(settings: XRSettings): void {
    this.settings = settings;
    
    // The XRSettings interface was updated, so we access properties directly.
    // We also need to check if settings itself is null before accessing its properties.
    if (settings) {
      this.movementEnabled = true; // Assuming movement is enabled if XR settings are present
      this.movementSpeed = settings.movementSpeed || 1.0;

      // Setup floor if enabled
      if (settings.showFloor) {
        this.createFloor();
      } else if (this.floorPlane) {
        this.scene.remove(this.floorPlane);
        this.floorPlane.geometry.dispose();
        (this.floorPlane.material as THREE.Material).dispose();
        this.floorPlane = null;
      }

      // Setup teleport marker if teleport is enabled
      if (settings.teleportEnabled) {
        this.createTeleportMarker();
      } else if (this.teleportMarker) {
        this.scene.remove(this.teleportMarker);
        this.teleportMarker.geometry.dispose();
        (this.teleportMarker.material as THREE.Material).dispose();
        this.teleportMarker = null;
      }
    }
    
    if (debugState.isEnabled()) {
      logger.info('XR initializer initialized with settings');
    }
  }
  
  // Setup all XR interactions
  private setupXRInteractions(): void {
    // Register for controller events
    this.controllerSelectStartUnsubscribe = this.xrSessionManager.onSelectStart(
      this.handleControllerSelectStart.bind(this)
    );
    
    this.controllerSelectEndUnsubscribe = this.xrSessionManager.onSelectEnd(
      this.handleControllerSelectEnd.bind(this)
    );
    
    this.controllerSqueezeStartUnsubscribe = this.xrSessionManager.onSqueezeStart(
      this.handleControllerSqueezeStart.bind(this)
    );
    
    this.controllerSqueezeEndUnsubscribe = this.xrSessionManager.onSqueezeEnd(
      this.handleControllerSqueezeEnd.bind(this)
    );
    
    // Add render callback for continuous interaction checks
    this.sceneManager.addRenderCallback(this.update.bind(this));
    
    if (debugState.isEnabled()) {
      logger.info('XR interactions setup complete');
    }
  }
  
  // Create floor plane for reference and teleportation
  private createFloor(): void {
    if (this.floorPlane) {
      return;
    }
    
    const geometry = new THREE.PlaneGeometry(20, 20);
    const material = new THREE.MeshBasicMaterial({
      color: 0x808080,
      transparent: true,
      opacity: 0.2,
      side: THREE.DoubleSide
    });
    
    this.floorPlane = new THREE.Mesh(geometry, material);
    this.floorPlane.rotation.x = -Math.PI / 2;
    this.floorPlane.position.y = 0;
    this.floorPlane.receiveShadow = true;
    this.floorPlane.name = 'xr-floor';
    
    this.scene.add(this.floorPlane);
    
    if (debugState.isEnabled()) {
      logger.info('XR floor plane created');
    }
  }
  
  // Create teleport marker for showing valid teleport locations
  private createTeleportMarker(): void {
    if (this.teleportMarker) {
      return;
    }
    
    const geometry = new THREE.RingGeometry(0.15, 0.2, 32);
    const material = new THREE.MeshBasicMaterial({
      color: 0x00ff00,
      transparent: true,
      opacity: 0.6,
      side: THREE.DoubleSide
    });
    
    this.teleportMarker = new THREE.Mesh(geometry, material);
    this.teleportMarker.rotation.x = -Math.PI / 2;
    this.teleportMarker.visible = false;
    this.teleportMarker.name = 'teleport-marker';
    
    this.scene.add(this.teleportMarker);
    
    if (debugState.isEnabled()) {
      logger.info('Teleport marker created');
    }
  }
  
  // Update loop for continuous interactions
  private update(): void {
    if (!this.xrSessionManager.isSessionActive()) {
      return;
    }
    
    // Get controllers
    const controllers = this.xrSessionManager.getControllers();
    
    // Update controller interactions
    controllers.forEach(controller => {
      this.updateControllerInteractions(controller);
    });
  }
  
  // Check for intersections with objects
  private updateControllerInteractions(controller: THREE.XRTargetRaySpace): void {
    // Skip if there's no session
    if (!this.xrSessionManager.isSessionActive()) {
      return;
    }
    
    // Initialize raycaster from controller
    const tempMatrix = new THREE.Matrix4();
    tempMatrix.identity().extractRotation(controller.matrixWorld);
    
    this.raycaster.ray.origin.setFromMatrixPosition(controller.matrixWorld);
    this.raycaster.ray.direction.set(0, 0, -1).applyMatrix4(tempMatrix);
    
    // Store intersections for this controller
    const intersections: THREE.Intersection[] = [];
    
    // Check for floor intersection if teleport is enabled
    if (this.floorPlane && this.settings?.teleportEnabled) {
      const floorIntersects = this.raycaster.intersectObject(this.floorPlane);
      
      if (floorIntersects.length > 0) {
        intersections.push(...floorIntersects);
        
        // Update teleport marker position if currently teleporting
        if (this.isTeleporting && this.teleportMarker) {
          this.teleportPosition.copy(floorIntersects[0].point);
          this.teleportMarker.position.copy(this.teleportPosition);
          this.teleportMarker.visible = true;
        }
      } else if (this.isTeleporting && this.teleportMarker) {
        // Hide marker if not pointing at floor
        this.teleportMarker.visible = false;
      }
    }
    
    // Store intersections for this controller
    this.controllerIntersections.set(controller, intersections);
  }
  
  // Handle controller select start event (trigger press)
  private handleControllerSelectStart(event: XRControllerEvent): void {
    const { controller } = event;
    
    // Start teleportation if enabled
    if (this.settings?.teleportEnabled) {
      this.isTeleporting = true;
      
      // Show teleport marker if there's a valid intersection
      const intersections = this.controllerIntersections.get(controller) || [];
      if (intersections.length > 0 && this.floorPlane && this.teleportMarker) {
        const floorIntersect = intersections.find(
          i => i.object === this.floorPlane
        );
        
        if (floorIntersect) {
          this.teleportPosition.copy(floorIntersect.point);
          this.teleportMarker.position.copy(this.teleportPosition);
          this.teleportMarker.visible = true;
        }
      }
    }
  }
  
  // Handle controller select end event (trigger release)
  private handleControllerSelectEnd(event: XRControllerEvent): void {
    // Complete teleportation if in progress
    if (this.isTeleporting && this.teleportMarker && this.teleportMarker.visible) {
      // Get camera position but keep y-height the same
      const cameraPosition = new THREE.Vector3();
      cameraPosition.setFromMatrixPosition(this.camera.matrixWorld);
      
      // Calculate teleport offset (where we want camera to end up)
      const offsetX = this.teleportPosition.x - cameraPosition.x;
      const offsetZ = this.teleportPosition.z - cameraPosition.z;
      
      // Find camera rig/offset parent - in WebXR the camera is often a child of a rig
      let cameraRig = this.camera.parent;
      if (cameraRig) {
        // Apply offset to camera rig's position
        cameraRig.position.x += offsetX;
        cameraRig.position.z += offsetZ;
      } else {
        // Fallback to moving camera directly if no rig
        this.camera.position.x += offsetX;
        this.camera.position.z += offsetZ;
      }
      
      // Hide teleport marker
      this.teleportMarker.visible = false;
      
      if (debugState.isDataDebugEnabled()) {
        logger.debug('Teleported to', { x: this.teleportPosition.x, z: this.teleportPosition.z });
      }
    }
    
    // Reset teleport state
    this.isTeleporting = false;
  }
  
  // Handle controller squeeze start event (grip press)
  private handleControllerSqueezeStart(event: XRControllerEvent): void {
    // Placeholder for future interactions
    // Could be used for grabbing objects, scaling the environment, etc.
  }
  
  // Handle controller squeeze end event (grip release)
  private handleControllerSqueezeEnd(event: XRControllerEvent): void {
    // Placeholder for future interactions
  }
  
  // Update settings for XR
  public updateSettings(settings: XRSettings): void {
    this.settings = settings;
    
    // The XRSettings interface was updated, so we access properties directly.
    // We also need to check if settings itself is null before accessing its properties.
    if (settings) {
      this.movementSpeed = settings.movementSpeed || 1.0;

      // Update floor visibility
      if (settings.showFloor) {
        if (!this.floorPlane) {
          this.createFloor();
        }
      } else if (this.floorPlane) {
        this.scene.remove(this.floorPlane);
        this.floorPlane.geometry.dispose();
        (this.floorPlane.material as THREE.Material).dispose();
        this.floorPlane = null;
      }

      // Update teleport marker
      if (settings.teleportEnabled) {
        if (!this.teleportMarker) {
          this.createTeleportMarker();
        }
      } else if (this.teleportMarker) {
        this.scene.remove(this.teleportMarker);
        this.teleportMarker.geometry.dispose();
        (this.teleportMarker.material as THREE.Material).dispose();
        this.teleportMarker = null;
      }
    }
  }
  
  // Clean up all XR-related resources
  public dispose(): void {
    // Unsubscribe from controller events
    if (this.controllerSelectStartUnsubscribe) {
      this.controllerSelectStartUnsubscribe();
      this.controllerSelectStartUnsubscribe = null;
    }
    
    if (this.controllerSelectEndUnsubscribe) {
      this.controllerSelectEndUnsubscribe();
      this.controllerSelectEndUnsubscribe = null;
    }
    
    if (this.controllerSqueezeStartUnsubscribe) {
      this.controllerSqueezeStartUnsubscribe();
      this.controllerSqueezeStartUnsubscribe = null;
    }
    
    if (this.controllerSqueezeEndUnsubscribe) {
      this.controllerSqueezeEndUnsubscribe();
      this.controllerSqueezeEndUnsubscribe = null;
    }
    
    // Remove floor and teleport marker
    if (this.floorPlane) {
      this.scene.remove(this.floorPlane);
      this.floorPlane.geometry.dispose();
      (this.floorPlane.material as THREE.Material).dispose();
      this.floorPlane = null;
    }
    
    if (this.teleportMarker) {
      this.scene.remove(this.teleportMarker);
      this.teleportMarker.geometry.dispose();
      (this.teleportMarker.material as THREE.Material).dispose();
      this.teleportMarker = null;
    }
    
    // Clear intersections map
    this.controllerIntersections.clear();
    
    if (debugState.isEnabled()) {
      logger.info('XR initializer disposed');
    }
  }
}