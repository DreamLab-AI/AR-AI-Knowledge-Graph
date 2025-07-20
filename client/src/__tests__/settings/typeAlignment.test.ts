import { describe, it, expect } from 'vitest';
import { Settings } from '../../features/settings/config/settings';
import { defaultSettings } from '../../features/settings/config/defaultSettings';

describe('Settings Type Alignment', () => {
  it('should have defaultSettings that conform to Settings interface', () => {
    // This test verifies that defaultSettings matches the Settings type
    const settings: Settings = defaultSettings;
    expect(settings).toBeDefined();
  });

  it('should have all required properties in defaultSettings', () => {
    // Check top-level properties
    expect(defaultSettings).toHaveProperty('visualisation');
    expect(defaultSettings).toHaveProperty('system');
    expect(defaultSettings).toHaveProperty('xr');
    expect(defaultSettings).toHaveProperty('auth');
  });

  it('should have correct types for visualization settings', () => {
    const { visualisation } = defaultSettings;
    
    // Node settings
    expect(typeof visualisation.nodes.baseColor).toBe('string');
    expect(typeof visualisation.nodes.metalness).toBe('number');
    expect(typeof visualisation.nodes.opacity).toBe('number');
    expect(typeof visualisation.nodes.roughness).toBe('number');
    expect(typeof visualisation.nodes.nodeSize).toBe('number');
    expect(['low', 'medium', 'high']).toContain(visualisation.nodes.quality);
    expect(typeof visualisation.nodes.enableInstancing).toBe('boolean');
    expect(typeof visualisation.nodes.enableHologram).toBe('boolean');
    expect(typeof visualisation.nodes.enableMetadataShape).toBe('boolean');
    expect(typeof visualisation.nodes.enableMetadataVisualisation).toBe('boolean');
  });

  it('should have correct types for edge settings', () => {
    const { edges } = defaultSettings.visualisation;
    
    expect(typeof edges.arrowSize).toBe('number');
    expect(typeof edges.baseWidth).toBe('number');
    expect(typeof edges.color).toBe('string');
    expect(typeof edges.enableArrows).toBe('boolean');
    expect(typeof edges.opacity).toBe('number');
    expect(Array.isArray(edges.widthRange)).toBe(true);
    expect(edges.widthRange).toHaveLength(2);
    expect(['low', 'medium', 'high']).toContain(edges.quality);
    expect(typeof edges.enableFlowEffect).toBe('boolean');
    expect(typeof edges.flowSpeed).toBe('number');
    expect(typeof edges.flowIntensity).toBe('number');
    expect(typeof edges.glowStrength).toBe('number');
    expect(typeof edges.distanceIntensity).toBe('number');
    expect(typeof edges.useGradient).toBe('boolean');
    expect(Array.isArray(edges.gradientColors)).toBe(true);
    expect(edges.gradientColors).toHaveLength(2);
  });

  it('should have correct types for physics settings', () => {
    const { physics } = defaultSettings.visualisation;
    
    expect(typeof physics.attractionStrength).toBe('number');
    expect(typeof physics.boundsSize).toBe('number');
    expect(typeof physics.collisionRadius).toBe('number');
    expect(typeof physics.damping).toBe('number');
    expect(typeof physics.enableBounds).toBe('boolean');
    expect(typeof physics.enabled).toBe('boolean');
    expect(typeof physics.iterations).toBe('number');
    expect(typeof physics.maxVelocity).toBe('number');
    expect(typeof physics.repulsionStrength).toBe('number');
    expect(typeof physics.springStrength).toBe('number');
    expect(typeof physics.repulsionDistance).toBe('number');
    expect(typeof physics.massScale).toBe('number');
    expect(typeof physics.boundaryDamping).toBe('number');
    expect(typeof physics.updateThreshold).toBe('number');
  });

  it('should have correct types for rendering settings', () => {
    const { rendering } = defaultSettings.visualisation;
    
    expect(typeof rendering.ambientLightIntensity).toBe('number');
    expect(typeof rendering.backgroundColor).toBe('string');
    expect(typeof rendering.directionalLightIntensity).toBe('number');
    expect(typeof rendering.enableAmbientOcclusion).toBe('boolean');
    expect(typeof rendering.enableAntialiasing).toBe('boolean');
    expect(typeof rendering.enableShadows).toBe('boolean');
    expect(typeof rendering.environmentIntensity).toBe('number');
    expect(typeof rendering.shadowMapSize).toBe('string');
    expect(typeof rendering.shadowBias).toBe('number');
    expect(['desktop', 'ar']).toContain(rendering.context);
  });

  it('should have correct types for bloom settings', () => {
    const { bloom } = defaultSettings.visualisation;
    
    expect(typeof bloom.edgeBloomStrength).toBe('number');
    expect(typeof bloom.enabled).toBe('boolean');
    expect(typeof bloom.environmentBloomStrength).toBe('number');
    expect(typeof bloom.nodeBloomStrength).toBe('number');
    expect(typeof bloom.radius).toBe('number');
    expect(typeof bloom.strength).toBe('number');
    expect(typeof bloom.threshold).toBe('number');
  });

  it('should have correct types for hologram settings', () => {
    const { hologram } = defaultSettings.visualisation;
    
    expect(typeof hologram.ringCount).toBe('number');
    expect(typeof hologram.ringColor).toBe('string');
    expect(typeof hologram.ringOpacity).toBe('number');
    expect(Array.isArray(hologram.sphereSizes)).toBe(true);
    expect(hologram.sphereSizes).toHaveLength(2);
    expect(typeof hologram.ringRotationSpeed).toBe('number');
    expect(typeof hologram.enableBuckminster).toBe('boolean');
    expect(typeof hologram.buckminsterSize).toBe('number');
    expect(typeof hologram.buckminsterOpacity).toBe('number');
    expect(typeof hologram.enableGeodesic).toBe('boolean');
    expect(typeof hologram.geodesicSize).toBe('number');
    expect(typeof hologram.geodesicOpacity).toBe('number');
    expect(typeof hologram.enableTriangleSphere).toBe('boolean');
    expect(typeof hologram.triangleSphereSize).toBe('number');
    expect(typeof hologram.triangleSphereOpacity).toBe('number');
    expect(typeof hologram.globalRotationSpeed).toBe('number');
  });

  it('should have correct types for XR settings', () => {
    const { xr } = defaultSettings;
    
    expect(typeof xr.enabled).toBe('boolean');
    expect(typeof xr.enableHandTracking).toBe('boolean');
    expect(typeof xr.enableHaptics).toBe('boolean');
    
    // Optional fields that should exist
    expect(xr).toHaveProperty('clientSideEnableXR');
    expect(xr).toHaveProperty('displayMode');
    expect(xr).toHaveProperty('roomScale');
    expect(xr).toHaveProperty('spaceType');
    expect(xr).toHaveProperty('quality');
  });

  it('should have correct types for WebSocket settings', () => {
    const { websocket } = defaultSettings.system;
    
    expect(typeof websocket.reconnectAttempts).toBe('number');
    expect(typeof websocket.reconnectDelay).toBe('number');
    expect(typeof websocket.binaryChunkSize).toBe('number');
    expect(typeof websocket.compressionEnabled).toBe('boolean');
    expect(typeof websocket.compressionThreshold).toBe('number');
    expect(typeof websocket.updateRate).toBe('number');
    
    // Optional fields
    if (websocket.binaryUpdateRate !== undefined) {
      expect(typeof websocket.binaryUpdateRate).toBe('number');
    }
    if (websocket.heartbeatInterval !== undefined) {
      expect(typeof websocket.heartbeatInterval).toBe('number');
    }
  });

  it('should have correct types for debug settings', () => {
    const { debug } = defaultSettings.system;
    
    expect(typeof debug.enabled).toBe('boolean');
    expect(typeof debug.enableDataDebug).toBe('boolean');
    expect(typeof debug.enableWebsocketDebug).toBe('boolean');
    expect(typeof debug.logBinaryHeaders).toBe('boolean');
    expect(typeof debug.logFullJson).toBe('boolean');
    
    // Optional fields
    if (debug.logLevel !== undefined) {
      expect(['debug', 'info', 'warn', 'error']).toContain(debug.logLevel);
    }
    if (debug.logFormat !== undefined) {
      expect(['json', 'text']).toContain(debug.logFormat);
    }
  });

  it('should have correct types for auth settings', () => {
    const { auth } = defaultSettings;
    
    expect(typeof auth.enabled).toBe('boolean');
    expect(typeof auth.provider).toBe('string');
    expect(typeof auth.required).toBe('boolean');
  });

  it('should handle optional AI service settings', () => {
    // RAGFlow settings
    if (defaultSettings.ragflow) {
      const { ragflow } = defaultSettings;
      if (ragflow.apiKey !== undefined) {
        expect(typeof ragflow.apiKey).toBe('string');
      }
      if (ragflow.agentId !== undefined) {
        expect(typeof ragflow.agentId).toBe('string');
      }
      if (ragflow.timeout !== undefined) {
        expect(typeof ragflow.timeout).toBe('number');
      }
    }

    // Perplexity settings
    if (defaultSettings.perplexity) {
      const { perplexity } = defaultSettings;
      if (perplexity.apiKey !== undefined) {
        expect(typeof perplexity.apiKey).toBe('string');
      }
      if (perplexity.model !== undefined) {
        expect(typeof perplexity.model).toBe('string');
      }
    }

    // OpenAI settings
    if (defaultSettings.openai) {
      const { openai } = defaultSettings;
      if (openai.apiKey !== undefined) {
        expect(typeof openai.apiKey).toBe('string');
      }
      if (openai.baseUrl !== undefined) {
        expect(typeof openai.baseUrl).toBe('string');
      }
    }

    // Kokoro settings
    if (defaultSettings.kokoro) {
      const { kokoro } = defaultSettings;
      if (kokoro.apiUrl !== undefined) {
        expect(typeof kokoro.apiUrl).toBe('string');
      }
      if (kokoro.defaultVoice !== undefined) {
        expect(typeof kokoro.defaultVoice).toBe('string');
      }
    }
  });

  it('should have valid number ranges for settings', () => {
    // Check opacity values are between 0 and 1
    expect(defaultSettings.visualisation.nodes.opacity).toBeGreaterThanOrEqual(0);
    expect(defaultSettings.visualisation.nodes.opacity).toBeLessThanOrEqual(1);
    
    expect(defaultSettings.visualisation.edges.opacity).toBeGreaterThanOrEqual(0);
    expect(defaultSettings.visualisation.edges.opacity).toBeLessThanOrEqual(1);
    
    // Check metalness and roughness are between 0 and 1
    expect(defaultSettings.visualisation.nodes.metalness).toBeGreaterThanOrEqual(0);
    expect(defaultSettings.visualisation.nodes.metalness).toBeLessThanOrEqual(1);
    
    expect(defaultSettings.visualisation.nodes.roughness).toBeGreaterThanOrEqual(0);
    expect(defaultSettings.visualisation.nodes.roughness).toBeLessThanOrEqual(1);
    
    // Check physics damping is between 0 and 1
    expect(defaultSettings.visualisation.physics.damping).toBeGreaterThanOrEqual(0);
    expect(defaultSettings.visualisation.physics.damping).toBeLessThanOrEqual(1);
    
    // Check positive values
    expect(defaultSettings.visualisation.physics.iterations).toBeGreaterThan(0);
    expect(defaultSettings.system.websocket.binaryChunkSize).toBeGreaterThan(0);
  });
});