// Type definitions for settings

export type SettingsPath = string | '';

// Node settings
export interface NodeSettings {
  baseColor: string;
  metalness: number;
  opacity: number;
  roughness: number;
  nodeSize: number; // Changed from sizeRange: [number, number]
  quality: 'low' | 'medium' | 'high';
  enableInstancing: boolean;
  enableHologram: boolean;
  enableMetadataShape: boolean;
  enableMetadataVisualisation: boolean;
}

// Edge settings
export interface EdgeSettings {
  arrowSize: number;
  baseWidth: number;
  color: string;
  enableArrows: boolean;
  opacity: number;
  widthRange: [number, number];
  quality: 'low' | 'medium' | 'high';
  enableFlowEffect: boolean;
  flowSpeed: number;
  flowIntensity: number;
  glowStrength: number;
  distanceIntensity: number;
  useGradient: boolean;
  gradientColors: [string, string];
}

// Physics settings
export interface PhysicsSettings {
  attractionStrength: number;
  boundsSize: number;
  collisionRadius: number;
  damping: number;
  enableBounds: boolean;
  enabled: boolean;
  iterations: number;
  maxVelocity: number;
  repulsionStrength: number;
  springStrength: number;
  repulsionDistance: number;
  massScale: number;
  boundaryDamping: number;
 updateThreshold: number;
}

// Rendering settings
export interface RenderingSettings {
  ambientLightIntensity: number;
  backgroundColor: string;
  directionalLightIntensity: number;
  enableAmbientOcclusion: boolean;
  enableAntialiasing: boolean;
  enableShadows: boolean;
  environmentIntensity: number;
  shadowMapSize: string;
  shadowBias: number;
  context: 'desktop' | 'ar';
}

// Animation settings
export interface AnimationSettings {
  enableMotionBlur: boolean;
  enableNodeAnimations: boolean;
  motionBlurStrength: number;
  selectionWaveEnabled: boolean;
  pulseEnabled: boolean;
  pulseSpeed: number;
  pulseStrength: number;
  waveSpeed: number;
}

// Label settings
export interface LabelSettings {
  desktopFontSize: number;
  enableLabels: boolean;
  textColor: string;
  textOutlineColor: string;
  textOutlineWidth: number;
  textResolution: number;
  textPadding: number;
  billboardMode: 'camera' | 'vertical';
}

// Bloom settings
export interface BloomSettings {
  edgeBloomStrength: number;
  enabled: boolean;
  environmentBloomStrength: number;
  nodeBloomStrength: number;
  radius: number;
  strength: number;
  threshold: number;
}

// Hologram settings
export interface HologramSettings {
  ringCount: number;
  ringColor: string;
  ringOpacity: number;
  sphereSizes: [number, number];
  ringRotationSpeed: number;
  enableBuckminster: boolean;
  buckminsterSize: number;
  buckminsterOpacity: number;
  enableGeodesic: boolean;
  geodesicSize: number;
  geodesicOpacity: number;
  enableTriangleSphere: boolean;
  triangleSphereSize: number;
  triangleSphereOpacity: number;
  globalRotationSpeed: number;
}

// WebSocket settings
export interface WebSocketSettings {
  reconnectAttempts: number;
  reconnectDelay: number;
  binaryChunkSize: number;
  binaryUpdateRate?: number;
  minUpdateRate?: number;
  maxUpdateRate?: number;
  motionThreshold?: number;
  motionDamping?: number;
  binaryMessageVersion?: number;
  compressionEnabled: boolean;
  compressionThreshold: number;
  heartbeatInterval?: number;
  heartbeatTimeout?: number;
  maxConnections?: number;
  maxMessageSize?: number;
  updateRate: number;
}

// Debug settings
export interface DebugSettings {
  enabled: boolean;
  logLevel?: 'debug' | 'info' | 'warn' | 'error'; // Added for client-side logging
  logFormat?: 'json' | 'text';
  enableDataDebug: boolean;
  enableWebsocketDebug: boolean;
  logBinaryHeaders: boolean;
  logFullJson: boolean;
  enablePhysicsDebug?: boolean;
  enableNodeDebug?: boolean;
  enableShaderDebug?: boolean;
  enableMatrixDebug?: boolean;
  enablePerformanceDebug?: boolean;
}

// XR settings
export interface XRSettings {
  enabled: boolean;
  clientSideEnableXR?: boolean; // Client-side XR toggle
  mode?: 'inline' | 'immersive-vr' | 'immersive-ar'; // Added from YAML
  roomScale?: number;
  spaceType?: 'local-floor' | 'bounded-floor' | 'unbounded';
  quality?: 'low' | 'medium' | 'high';
  enableHandTracking: boolean;
  handMeshEnabled?: boolean;
  handMeshColor?: string;
  handMeshOpacity?: number;
  handPointSize?: number;
  handRayEnabled?: boolean;
  handRayColor?: string;
  handRayWidth?: number;
  gestureSmoothing?: number;
  enableHaptics: boolean;
  hapticIntensity?: number;
  dragThreshold?: number;
  pinchThreshold?: number;
  rotationThreshold?: number;
  interactionRadius?: number;
  movementSpeed?: number;
  deadZone?: number;
  movementAxesHorizontal?: number;
  movementAxesVertical?: number;
  enableLightEstimation?: boolean;
  enablePlaneDetection?: boolean;
  enableSceneUnderstanding?: boolean;
  planeColor?: string;
  planeOpacity?: number;
  planeDetectionDistance?: number;
  showPlaneOverlay?: boolean;
  snapToFloor?: boolean;
  enablePassthroughPortal?: boolean;
  passthroughOpacity?: number;
  passthroughBrightness?: number;
  passthroughContrast?: number;
  portalSize?: number;
  portalEdgeColor?: string;
  portalEdgeWidth?: number;
  controllerModel?: string;
  renderScale?: number;
  interactionDistance?: number;
  locomotionMethod?: 'teleport' | 'continuous';
  teleportRayColor?: string;
  displayMode?: 'inline' | 'immersive-vr' | 'immersive-ar';
  controllerRayColor?: string;
}

// Visualisation settings
export interface CameraSettings {
  fov: number;
  near: number;
  far: number;
  position: { x: number; y: number; z: number };
  lookAt?: { x: number; y: number; z: number }; // lookAt is often dynamic
}
export interface VisualisationSettings {
  nodes: NodeSettings;
  edges: EdgeSettings;
  physics: PhysicsSettings;
  rendering: RenderingSettings;
  animations: AnimationSettings;
  labels: LabelSettings;
  bloom: BloomSettings;
  hologram: HologramSettings;
  camera?: CameraSettings;
}

// System settings
export interface SystemSettings {
  websocket: WebSocketSettings;
  debug: DebugSettings;
  persistSettings: boolean; // Added to control server-side persistence
  customBackendUrl?: string; // Add if missing
}

// RAGFlow settings
export interface RAGFlowSettings {
  apiKey?: string;
  agentId?: string;
  apiBaseUrl?: string;
  timeout?: number;
  maxRetries?: number;
  chatId?: string;
}

// Perplexity settings
export interface PerplexitySettings {
  apiKey?: string;
  model?: string;
  apiUrl?: string;
  maxTokens?: number;
  temperature?: number;
  topP?: number;
  presencePenalty?: number;
  frequencyPenalty?: number;
  timeout?: number;
  rateLimit?: number;
}

// OpenAI settings
export interface OpenAISettings {
  apiKey?: string;
  baseUrl?: string;
  timeout?: number;
  rateLimit?: number;
}

// Kokoro TTS settings
export interface KokoroSettings {
  apiUrl?: string;
  defaultVoice?: string;
  defaultFormat?: string;
  defaultSpeed?: number;
  timeout?: number;
  stream?: boolean;
  returnTimestamps?: boolean;
  sampleRate?: number;
}

// Auth settings
export interface AuthSettings {
  enabled: boolean;
  provider: 'nostr' | string; // Allow other providers potentially
  required: boolean;
}

// Main settings interface
export interface Settings {
  visualisation: VisualisationSettings;
  system: SystemSettings;
  xr: XRSettings;
  auth: AuthSettings; // Make auth required
  ragflow?: RAGFlowSettings; // Add optional AI settings
  perplexity?: PerplexitySettings;
  openai?: OpenAISettings;
  kokoro?: KokoroSettings;
}
