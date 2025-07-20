// Stub implementation of SceneManager to prevent conflicts with React Three Fiber
import { createLogger, createErrorMetadata } from '@/utils/logger';
import { debugState } from '@/utils/debugState';
import * as THREE from 'three';
import { XRSettings } from '@/features/xr/types/xr';

const logger = createLogger('SceneManager');

/**
 * IMPORTANT: This is a stub implementation of SceneManager
 * The application has been migrated to use React Three Fiber
 * This stub exists only to satisfy imports and prevent runtime errors
 */

export class SceneManager {
  private static instance: SceneManager;
  private scene: any = {};
  private camera: THREE.PerspectiveCamera | null = null;
  private renderer: THREE.WebGLRenderer | null = null;
  private running: boolean = false;
  private renderCallbacks: any[] = [];
  private resizeCallbacks: any[] = [];
  private disposeCallbacks: any[] = [];

  private constructor() {
    logger.info('Using React Three Fiber for rendering - SceneManager is in compatibility mode');
    
    // Create minimal THREE.js objects to prevent errors
    try {
      this.scene = new THREE.Scene();
      // Create perspective camera specifically since XR systems need it
      this.camera = new THREE.PerspectiveCamera(
        75, // FOV
        window.innerWidth / window.innerHeight, // Aspect ratio
        0.1, // Near plane
        1000 // Far plane
      );
      this.camera.position.z = 5;
    } catch (error) {
      logger.error('Error creating THREE.js objects:', createErrorMetadata(error));
      // Fall back to mock objects if THREE.js fails to initialize
    }
  }

  public static getInstance(canvas?: HTMLCanvasElement): SceneManager {
    if (!SceneManager.instance) {
      SceneManager.instance = new SceneManager();
    }
    return SceneManager.instance;
  }

  public static cleanup(): void {
    if (SceneManager.instance) {
      logger.info('SceneManager cleanup called');
    }
  }

  // Stub methods that do nothing
  private initRenderer(canvas: HTMLCanvasElement): void {
    // Only try to initialize if we don't already have a renderer
    if (this.renderer) return;
    
    try {
      logger.info('Attempting to create WebGLRenderer (compatibility mode)');
      
      // Verify that the canvas is valid
      if (!canvas || !canvas.getContext) {
        throw new Error('Invalid canvas element or getContext is not a function');
      }
      
      // Try to create a renderer with minimal settings
      this.renderer = new THREE.WebGLRenderer({
        canvas,
        alpha: true,
        antialias: true,
        powerPreference: 'default'
      });
      
      this.renderer.setSize(window.innerWidth, window.innerHeight);
      this.renderer.setPixelRatio(window.devicePixelRatio);
      
    } catch (error) {
      logger.error('Failed to initialize renderer:', createErrorMetadata(error));
      
      // Create a mock renderer to prevent further errors
      this.renderer = {
        domElement: canvas,
        setSize: () => {},
        render: () => {},
        dispose: () => {}
      } as unknown as THREE.WebGLRenderer;
    }
  }
  
  private setupResizeHandler(): void {}
  public handleSettingsUpdate(settings: XRSettings): void {
    if (debugState.isEnabled()) {
      logger.info('SceneManager.start() called but using React Three Fiber instead');
    }
  }

  public stop(): void {}
  private render = (): void => {};
  public addRenderCallback(callback: () => void): () => void {
    return () => {};
  }
  public addResizeCallback(callback: () => void): () => void {
    // Store callback in array but don't actually use it
    this.resizeCallbacks.push(callback);
    // Return remove function
    return () => {
      const index = this.resizeCallbacks.indexOf(callback);
      if (index !== -1) {
        this.resizeCallbacks.splice(index, 1);
      }
    };
  }
  
  public start(): void {
    logger.info('SceneManager.start() called (compatibility mode - no action taken)');
    this.running = true;
  }
  
  public dispose(): void {
    logger.info('SceneManager.dispose() called (compatibility mode)');
    this.running = false;
  }
  
  // Getters
  public getScene(): any {
    return this.scene;
  }
  public getCamera = (): THREE.PerspectiveCamera | null => this.camera;
  public getRenderer = (): THREE.WebGLRenderer | null => this.renderer;
  public isRunning(): boolean {
    return this.running;
  }
}