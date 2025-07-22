// client/src/features/settings/config/settingsUIDefinition.ts

export type SettingWidgetType =
  | 'toggle'
  | 'slider'
  | 'numberInput'
  | 'textInput'
  | 'colorPicker'
  | 'select'
  | 'radioGroup' // For a small set of mutually exclusive choices
  | 'rangeSlider' // For [number, number] arrays
  | 'buttonAction'
  | 'dualColorPicker'; // Custom type for [string, string] color arrays

export interface UISettingDefinition {
  label: string;
  type: SettingWidgetType;
  path: string; // Full path in the SettingsStore, e.g., "visualisation.nodes.baseColor"
  description?: string; // Tooltip text
  options?: Array<{ value: string | number; label: string }>; // For select
  min?: number; // For slider, numberInput, rangeSlider
  max?: number; // For slider, numberInput, rangeSlider
  step?: number; // For slider, numberInput, rangeSlider
  unit?: string; // e.g., "px", "ms"
  isAdvanced?: boolean; // To hide behind an "Advanced" toggle if needed
  isPowerUserOnly?: boolean; // Only visible/editable by power users
  action?: () => void; // For buttonAction type
}

export interface UISubsectionDefinition {
  label: string;
  settings: Record<string, UISettingDefinition>;
}

export interface UICategoryDefinition {
  label: string;
  icon?: string; // Lucide icon name
  subsections: Record<string, UISubsectionDefinition>;
}

// Helper function to create graph-specific settings for a given graph name
const createGraphSettingsSubsections = (graphName: 'logseq' | 'visionflow') => ({
  [`${graphName}Nodes`]: {
    label: `${graphName === 'logseq' ? 'Logseq' : 'VisionFlow'} Nodes`,
    settings: {
      baseColor: { label: 'Base Color', type: 'colorPicker', path: `visualisation.graphs.${graphName}.nodes.baseColor`, description: 'Default color of graph nodes.' },
      metalness: { label: 'Metalness', type: 'slider', min: 0, max: 1, step: 0.01, path: `visualisation.graphs.${graphName}.nodes.metalness`, description: 'How metallic nodes appear.' },
      opacity: { label: 'Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: `visualisation.graphs.${graphName}.nodes.opacity`, description: 'Overall opacity of nodes.' },
      roughness: { label: 'Roughness', type: 'slider', min: 0, max: 1, step: 0.01, path: `visualisation.graphs.${graphName}.nodes.roughness`, description: 'Surface roughness of nodes.' },
      nodeSize: { label: 'Node Size', type: 'slider', min: 0.01, max: 2.0, step: 0.01, path: `visualisation.graphs.${graphName}.nodes.nodeSize`, description: 'Controls the overall size of the nodes.' },
      quality: { label: 'Quality', type: 'radioGroup', options: [{value: 'low', label: 'Low'}, {value: 'medium', label: 'Medium'}, {value: 'high', label: 'High'}], path: `visualisation.graphs.${graphName}.nodes.quality`, description: 'Render quality of nodes.' },
      enableInstancing: { label: 'Enable Instancing', type: 'toggle', path: `visualisation.graphs.${graphName}.nodes.enableInstancing`, description: 'Use instanced rendering for nodes (performance).' },
      enableHologram: { label: 'Enable Hologram Effect', type: 'toggle', path: `visualisation.graphs.${graphName}.nodes.enableHologram`, description: 'Apply hologram effect to nodes.' },
      enableMetadataShape: { label: 'Enable Metadata Shape', type: 'toggle', path: `visualisation.graphs.${graphName}.nodes.enableMetadataShape`, description: 'Use shapes based on metadata.' },
      enableMetadataVisualisation: { label: 'Enable Metadata Visualisation', type: 'toggle', path: `visualisation.graphs.${graphName}.nodes.enableMetadataVisualisation`, description: 'Show metadata as part of node visualisation.' },
    },
  },
  [`${graphName}Edges`]: {
    label: `${graphName === 'logseq' ? 'Logseq' : 'VisionFlow'} Edges`,
    settings: {
      arrowSize: { label: 'Arrow Size', type: 'slider', min: 0.01, max: 0.5, step: 0.01, path: `visualisation.graphs.${graphName}.edges.arrowSize`, description: 'Size of the arrows on edges.' },
      baseWidth: { label: 'Base Width', type: 'slider', min: 0.01, max: 2, step: 0.01, path: `visualisation.graphs.${graphName}.edges.baseWidth`, description: 'Base width of edges.' },
      color: { label: 'Color', type: 'colorPicker', path: `visualisation.graphs.${graphName}.edges.color`, description: 'Default color of edges.' },
      enableArrows: { label: 'Enable Arrows', type: 'toggle', path: `visualisation.graphs.${graphName}.edges.enableArrows`, description: 'Show arrows on directed edges.' },
      opacity: { label: 'Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: `visualisation.graphs.${graphName}.edges.opacity`, description: 'Overall opacity of edges.' },
      widthRange: { label: 'Width Range', type: 'rangeSlider', min: 0.1, max: 5, step: 0.1, path: `visualisation.graphs.${graphName}.edges.widthRange`, description: 'Minimum and maximum width for edges.' },
      quality: { label: 'Quality', type: 'select', options: [{value: 'low', label: 'Low'}, {value: 'medium', label: 'Medium'}, {value: 'high', label: 'High'}], path: `visualisation.graphs.${graphName}.edges.quality`, description: 'Render quality of edges.' },
      enableFlowEffect: { label: 'Enable Flow Effect', type: 'toggle', path: `visualisation.graphs.${graphName}.edges.enableFlowEffect`, description: 'Animate a flow effect along edges.', isAdvanced: true },
      flowSpeed: { label: 'Flow Speed', type: 'slider', min: 0.1, max: 5, step: 0.1, path: `visualisation.graphs.${graphName}.edges.flowSpeed`, description: 'Speed of the flow effect.', isAdvanced: true },
      flowIntensity: { label: 'Flow Intensity', type: 'slider', min: 0, max: 10, step: 0.1, path: `visualisation.graphs.${graphName}.edges.flowIntensity`, description: 'Intensity of the flow effect.' },
      glowStrength: { label: 'Glow Strength', type: 'slider', min: 0, max: 5, step: 0.1, path: `visualisation.graphs.${graphName}.edges.glowStrength`, description: 'Strength of the edge glow effect.' },
      distanceIntensity: { label: 'Distance Intensity', type: 'slider', min: 0, max: 10, step: 0.1, path: `visualisation.graphs.${graphName}.edges.distanceIntensity`, description: 'Intensity based on distance for some edge effects.' },
      useGradient: { label: 'Use Gradient', type: 'toggle', path: `visualisation.graphs.${graphName}.edges.useGradient`, description: 'Use a gradient for edge colors.' },
      gradientColors: { label: 'Gradient Colors', type: 'dualColorPicker', path: `visualisation.graphs.${graphName}.edges.gradientColors`, description: 'Start and end colors for edge gradient.' },
    },
  },
  [`${graphName}Labels`]: {
    label: `${graphName === 'logseq' ? 'Logseq' : 'VisionFlow'} Labels`,
    settings: {
      enableLabels: { label: 'Enable Labels', type: 'toggle', path: `visualisation.graphs.${graphName}.labels.enableLabels`, description: 'Show text labels for nodes.' },
      desktopFontSize: { label: 'Desktop Font Size', type: 'slider', min: 0.01, max: 1.5, step: 0.05, path: `visualisation.graphs.${graphName}.labels.desktopFontSize`, description: 'Font size for labels in desktop mode.' },
      textColor: { label: 'Text Color', type: 'colorPicker', path: `visualisation.graphs.${graphName}.labels.textColor`, description: 'Color of label text.' },
      textOutlineColor: { label: 'Outline Color', type: 'colorPicker', path: `visualisation.graphs.${graphName}.labels.textOutlineColor`, description: 'Color of label text outline.' },
      textOutlineWidth: { label: 'Outline Width', type: 'slider', min: 0, max: 0.01, step: 0.001, path: `visualisation.graphs.${graphName}.labels.textOutlineWidth`, description: 'Width of label text outline.' },
      textResolution: { label: 'Text Resolution', type: 'numberInput', min: 8, max: 128, step: 1, path: `visualisation.graphs.${graphName}.labels.textResolution`, description: 'Resolution of text rendering texture.' },
      textPadding: { label: 'Text Padding', type: 'slider', min: 0, max: 3.0, step: 0.1, path: `visualisation.graphs.${graphName}.labels.textPadding`, description: 'Padding around text labels.' },
      billboardMode: { label: 'Billboard Mode', type: 'select', options: [{value: 'camera', label: 'Camera Facing'}, {value: 'vertical', label: 'Vertical Lock'}], path: `visualisation.graphs.${graphName}.labels.billboardMode`, description: 'How labels orient themselves.' },
    },
  },
  [`${graphName}Physics`]: {
    label: `${graphName === 'logseq' ? 'Logseq' : 'VisionFlow'} Physics`,
    settings: {
      enabled: { label: 'Enable Physics', type: 'toggle', path: `visualisation.graphs.${graphName}.physics.enabled`, description: 'Enable physics simulation for graph layout.' },
      attractionStrength: { label: 'Attraction Strength', type: 'slider', min: 0, max: 1, step: 0.01, path: `visualisation.graphs.${graphName}.physics.attractionStrength`, description: 'Strength of attraction between connected nodes.' },
      boundsSize: { label: 'Bounds Size', type: 'slider', min: 1, max: 50, step: 0.5, path: `visualisation.graphs.${graphName}.physics.boundsSize`, description: 'Size of the simulation bounding box.' },
      collisionRadius: { label: 'Collision Radius', type: 'slider', min: 0.1, max: 5, step: 0.1, path: `visualisation.graphs.${graphName}.physics.collisionRadius`, description: 'Radius for node collision detection.' },
      damping: { label: 'Damping', type: 'slider', min: 0, max: 1, step: 0.01, path: `visualisation.graphs.${graphName}.physics.damping`, description: 'Damping factor to slow down node movement.' },
      enableBounds: { label: 'Enable Bounds', type: 'toggle', path: `visualisation.graphs.${graphName}.physics.enableBounds`, description: 'Confine nodes within the bounds size.' },
      iterations: { label: 'Iterations', type: 'slider', min: 10, max: 500, step: 10, path: `visualisation.graphs.${graphName}.physics.iterations`, description: 'Number of physics iterations per step.' },
      maxVelocity: { label: 'Max Velocity', type: 'slider', min: 0.001, max: 0.5, step: 0.001, path: `visualisation.graphs.${graphName}.physics.maxVelocity`, description: 'Maximum velocity of nodes.' },
      repulsionStrength: { label: 'Repulsion Strength', type: 'slider', min: 0, max: 2, step: 0.01, path: `visualisation.graphs.${graphName}.physics.repulsionStrength`, description: 'Strength of repulsion between nodes.' },
      springStrength: { label: 'Spring Strength', type: 'slider', min: 0, max: 1, step: 0.01, path: `visualisation.graphs.${graphName}.physics.springStrength`, description: 'Strength of springs (edges) pulling nodes together.' },
      repulsionDistance: { label: 'Repulsion Distance', type: 'slider', min: 0.1, max: 10, step: 0.1, path: `visualisation.graphs.${graphName}.physics.repulsionDistance`, description: 'Distance at which repulsion force acts.' },
      massScale: { label: 'Mass Scale', type: 'slider', min: 0.1, max: 10, step: 0.1, path: `visualisation.graphs.${graphName}.physics.massScale`, description: 'Scaling factor for node mass.' },
      boundaryDamping: { label: 'Boundary Damping', type: 'slider', min: 0, max: 1, step: 0.01, path: `visualisation.graphs.${graphName}.physics.boundaryDamping`, description: 'Damping when nodes hit boundaries.' },
      updateThreshold: {
        label: 'Update Threshold',
        type: 'slider',
        min: 0,
        max: 0.5,
        step: 0.001,
        path: `visualisation.graphs.${graphName}.physics.updateThreshold`,
        description: 'Distance threshold below which server position updates are ignored to let nodes settle. A value of 0 applies all updates.'
      },
    },
  },
});

export const settingsUIDefinition: Record<string, UICategoryDefinition> = {
  visualisation: {
    label: 'Visualisation',
    icon: 'Eye',
    subsections: {
      // Graph-specific settings
      ...createGraphSettingsSubsections('logseq'),
      ...createGraphSettingsSubsections('visionflow'),
      rendering: {
        label: 'Rendering',
        settings: {
          backgroundColor: { label: 'Background Color', type: 'colorPicker', path: 'visualisation.rendering.backgroundColor', description: 'Color of the viewport background.' },
          enableAntialiasing: { label: 'Enable Antialiasing', type: 'toggle', path: 'visualisation.rendering.enableAntialiasing', description: 'Smooth jagged edges (MSAA/FXAA).' },
          ambientLightIntensity: { label: 'Ambient Light Intensity', type: 'slider', min: 0, max: 2, step: 0.05, path: 'visualisation.rendering.ambientLightIntensity', description: 'Intensity of ambient light.' },
          directionalLightIntensity: { label: 'Directional Light Intensity', type: 'slider', min: 0, max: 2, step: 0.05, path: 'visualisation.rendering.directionalLightIntensity', description: 'Intensity of directional light.' },
          enableAmbientOcclusion: { label: 'Enable Ambient Occlusion', type: 'toggle', path: 'visualisation.rendering.enableAmbientOcclusion', description: 'Enable screen-space ambient occlusion (SSAO).', isAdvanced: true },
          enableShadows: { label: 'Enable Shadows', type: 'toggle', path: 'visualisation.rendering.enableShadows', description: 'Enable dynamic shadows.', isAdvanced: true },
          environmentIntensity: { label: 'Environment Intensity', type: 'slider', min: 0, max: 2, step: 0.05, path: 'visualisation.rendering.environmentIntensity', description: 'Intensity of environment lighting (IBL).' },
          shadowMapSize: { label: 'Shadow Map Size', type: 'select', options: [{value: '512', label: '512px'}, {value: '1024', label: '1024px'}, {value: '2048', label: '2048px'}, {value: '4096', label: '4096px'}], path: 'visualisation.rendering.shadowMapSize', description: 'Resolution of shadow maps.', isAdvanced: true },
          shadowBias: { label: 'Shadow Bias', type: 'slider', min: -0.01, max: 0.01, step: 0.0001, path: 'visualisation.rendering.shadowBias', description: 'Bias to prevent shadow acne.', isAdvanced: true },
          context: { label: 'Rendering Context', type: 'select', options: [{value: 'desktop', label: 'Desktop'}, {value: 'ar', label: 'AR'}], path: 'visualisation.rendering.context', description: 'Current rendering context.' },
        },
      },
      bloom: {
        label: 'Bloom Effect',
        settings: {
          enabled: { label: 'Enable Bloom', type: 'toggle', path: 'visualisation.bloom.enabled', description: 'Enable post-processing bloom effect.' },
          strength: { label: 'Strength', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.bloom.strength', description: 'Overall strength of the bloom effect.' },
          edgeBloomStrength: { label: 'Edge Bloom Strength', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.bloom.edgeBloomStrength', description: 'Bloom strength for edges.' },
          environmentBloomStrength: { label: 'Environment Bloom Strength', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.bloom.environmentBloomStrength', description: 'Bloom strength from environment.' },
          nodeBloomStrength: { label: 'Node Bloom Strength', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.bloom.nodeBloomStrength', description: 'Bloom strength for nodes.' },
          radius: { label: 'Radius', type: 'slider', min: 0, max: 5, step: 0.05, path: 'visualisation.bloom.radius', description: 'Radius of the bloom effect.' },
          threshold: { label: 'Threshold', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.bloom.threshold', description: 'Luminance threshold for bloom.' },
        },
      },
      hologram: {
        label: 'Hologram Effect',
        settings: {
          ringCount: { label: 'Ring Count', type: 'slider', min: 0, max: 10, step: 1, path: 'visualisation.hologram.ringCount', description: 'Number of rings in hologram effect.' },
          ringColor: { label: 'Ring Color', type: 'colorPicker', path: 'visualisation.hologram.ringColor', description: 'Color of hologram rings.' },
          ringOpacity: { label: 'Ring Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.hologram.ringOpacity', description: 'Opacity of hologram rings.' },
          sphereSizes: { label: 'Sphere Sizes (Min/Max)', type: 'rangeSlider', min: 0.1, max: 20, step: 0.1, path: 'visualisation.hologram.sphereSizes', description: 'Min/max sizes for hologram spheres.' },
          ringRotationSpeed: { label: 'Ring Rotation Speed', type: 'slider', min: 0, max: 50, step: 0.5, path: 'visualisation.hologram.ringRotationSpeed', description: 'Rotation speed of hologram rings.' },
          enableBuckminster: { label: 'Enable Buckminster', type: 'toggle', path: 'visualisation.hologram.enableBuckminster', description: 'Enable Buckminster fullerene style hologram.' },
          buckminsterSize: { label: 'Buckminster Size', type: 'slider', min: 1, max: 20, step: 0.5, path: 'visualisation.hologram.buckminsterSize', description: 'Size of Buckminster hologram.' },
          buckminsterOpacity: { label: 'Buckminster Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.hologram.buckminsterOpacity', description: 'Opacity of Buckminster hologram.' },
          enableGeodesic: { label: 'Enable Geodesic', type: 'toggle', path: 'visualisation.hologram.enableGeodesic', description: 'Enable geodesic dome style hologram.' },
          geodesicSize: { label: 'Geodesic Size', type: 'slider', min: 1, max: 20, step: 0.5, path: 'visualisation.hologram.geodesicSize', description: 'Size of geodesic hologram.' },
          geodesicOpacity: { label: 'Geodesic Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.hologram.geodesicOpacity', description: 'Opacity of geodesic hologram.' },
          enableTriangleSphere: { label: 'Enable Triangle Sphere', type: 'toggle', path: 'visualisation.hologram.enableTriangleSphere', description: 'Enable triangle sphere style hologram.' },
          triangleSphereSize: { label: 'Triangle Sphere Size', type: 'slider', min: 1, max: 20, step: 0.5, path: 'visualisation.hologram.triangleSphereSize', description: 'Size of triangle sphere hologram.' },
          triangleSphereOpacity: { label: 'Triangle Sphere Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.hologram.triangleSphereOpacity', description: 'Opacity of triangle sphere hologram.' },
          globalRotationSpeed: { label: 'Global Rotation Speed', type: 'slider', min: 0, max: 20, step: 0.1, path: 'visualisation.hologram.globalRotationSpeed', description: 'Global rotation speed for hologram effects.' },
        },
      },
      animations: {
        label: 'Animations',
        settings: {
          enableNodeAnimations: { label: 'Enable Node Animations', type: 'toggle', path: 'visualisation.animations.enableNodeAnimations', description: 'Enable generic node animations.' },
          enableMotionBlur: { label: 'Enable Motion Blur', type: 'toggle', path: 'visualisation.animations.enableMotionBlur', description: 'Enable motion blur effect.', isAdvanced: true },
          motionBlurStrength: { label: 'Motion Blur Strength', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.animations.motionBlurStrength', description: 'Strength of motion blur.', isAdvanced: true },
          selectionWaveEnabled: { label: 'Enable Selection Wave', type: 'toggle', path: 'visualisation.animations.selectionWaveEnabled', description: 'Enable wave animation on selection.' },
          pulseEnabled: { label: 'Enable Pulse Animation', type: 'toggle', path: 'visualisation.animations.pulseEnabled', description: 'Enable pulsing animation on nodes.' },
          pulseSpeed: { label: 'Pulse Speed', type: 'slider', min: 0.1, max: 2, step: 0.05, path: 'visualisation.animations.pulseSpeed', description: 'Speed of pulse animation.' },
          pulseStrength: { label: 'Pulse Strength', type: 'slider', min: 0.1, max: 2, step: 0.05, path: 'visualisation.animations.pulseStrength', description: 'Strength of pulse animation.' },
          waveSpeed: { label: 'Wave Speed', type: 'slider', min: 0.1, max: 2, step: 0.05, path: 'visualisation.animations.waveSpeed', description: 'Speed of selection wave animation.' },
        },
      },
    },
  },
  system: {
    label: 'System',
    icon: 'Settings',
    subsections: {
      general: {
        label: 'General',
        settings: {
          persistSettings: { label: 'Persist Settings on Server', type: 'toggle', path: 'system.persistSettings', description: 'Save settings to your user profile on the server (if authenticated).'},
          customBackendUrl: { label: 'Custom Backend URL', type: 'textInput', path: 'system.customBackendUrl', description: 'Overrides the default backend URL. Requires app reload. Leave empty for default.', isPowerUserOnly: true },
        }
      },
      websocket: {
        label: 'WebSocket',
        settings: {
          updateRate: { label: 'Update Rate (Hz)', type: 'slider', min: 1, max: 60, step: 1, path: 'system.websocket.updateRate', description: 'Frequency of position updates from server.' },
          reconnectAttempts: { label: 'Reconnect Attempts', type: 'slider', min: 0, max: 10, step: 1, path: 'system.websocket.reconnectAttempts', description: 'Number of WebSocket reconnect attempts.' },
          reconnectDelay: { label: 'Reconnect Delay', type: 'slider', unit: 'ms', min: 500, max: 10000, step: 100, path: 'system.websocket.reconnectDelay', description: 'Delay between WebSocket reconnect attempts.' },
          binaryChunkSize: { label: 'Binary Chunk Size', type: 'slider', unit: 'bytes', min: 256, max: 8192, step: 256, path: 'system.websocket.binaryChunkSize', description: 'Chunk size for binary data transmission.' },
          compressionEnabled: { label: 'Compression Enabled', type: 'toggle', path: 'system.websocket.compressionEnabled', description: 'Enable WebSocket message compression.' },
          compressionThreshold: { label: 'Compression Threshold', type: 'slider', unit: 'bytes', min: 64, max: 4096, step: 64, path: 'system.websocket.compressionThreshold', description: 'Threshold for WebSocket compression.' },
        },
      },
      debug: {
        label: 'Client Debugging',
        settings: {
          enabled: { label: 'Enable Client Debug Mode', type: 'toggle', path: 'system.debug.enabled', description: 'Enable general client-side debug logging and features.' },
          logLevel: { label: 'Client Log Level', type: 'select', options: [{value: 'debug', label: 'Debug'}, {value: 'info', label: 'Info'}, {value: 'warn', label: 'Warn'}, {value: 'error', label: 'Error'}], path: 'system.debug.logLevel', description: 'Client console log level.', isPowerUserOnly: true },
          enableDataDebug: { label: 'Enable Data Debug', type: 'toggle', path: 'system.debug.enableDataDebug', description: 'Log detailed client data flow information.', isAdvanced: true },
          enableWebsocketDebug: { label: 'Enable WebSocket Debug', type: 'toggle', path: 'system.debug.enableWebsocketDebug', description: 'Log WebSocket communication details.', isAdvanced: true },
          logBinaryHeaders: { label: 'Log Binary Headers', type: 'toggle', path: 'system.debug.logBinaryHeaders', description: 'Log headers of binary messages.', isAdvanced: true },
          logFullJson: { label: 'Log Full JSON', type: 'toggle', path: 'system.debug.logFullJson', description: 'Log complete JSON payloads.', isAdvanced: true },
          enablePhysicsDebug: { label: 'Enable Physics Debug', type: 'toggle', path: 'system.debug.enablePhysicsDebug', description: 'Show physics debug visualizations.', isAdvanced: true },
          enableNodeDebug: { label: 'Enable Node Debug', type: 'toggle', path: 'system.debug.enableNodeDebug', description: 'Enable debug features for nodes.', isAdvanced: true },
          enableShaderDebug: { label: 'Enable Shader Debug', type: 'toggle', path: 'system.debug.enableShaderDebug', description: 'Enable shader debugging tools.', isAdvanced: true, isPowerUserOnly: true },
          enableMatrixDebug: { label: 'Enable Matrix Debug', type: 'toggle', path: 'system.debug.enableMatrixDebug', description: 'Log matrix transformations.', isAdvanced: true, isPowerUserOnly: true },
          enablePerformanceDebug: { label: 'Enable Performance Debug', type: 'toggle', path: 'system.debug.enablePerformanceDebug', description: 'Show performance metrics.', isAdvanced: true },
        },
      },
    },
  },
  xr: {
    label: 'XR',
    icon: 'Smartphone',
    subsections: {
      general: {
        label: 'General XR',
        settings: {
          clientSideEnableXR: { label: 'Enable XR Mode (Client)', type: 'toggle', path: 'xr.clientSideEnableXR', description: 'Toggle immersive XR mode. Requires a compatible headset and page reload.' },
          enabled: { label: 'XR Features Enabled (Server)', type: 'toggle', path: 'xr.enabled', description: 'Enable XR features on the server (requires server restart if changed).', isPowerUserOnly: true },
          displayMode: { label: 'XR Display Mode', type: 'select', options: [{value: 'inline', label: 'Inline (Desktop)'}, {value: 'immersive-vr', label: 'Immersive VR'}, {value: 'immersive-ar', label: 'Immersive AR'}], path: 'xr.displayMode', description: 'Preferred XR display mode.' },
          quality: { label: 'XR Quality', type: 'select', options: [{value: 'low', label: 'Low'}, {value: 'medium', label: 'Medium'}, {value: 'high', label: 'High'}], path: 'xr.quality', description: 'Overall rendering quality in XR.' },
          renderScale: { label: 'Render Scale', type: 'slider', min: 0.5, max: 2, step: 0.1, path: 'xr.renderScale', description: 'XR rendering resolution scale.' },
          interactionDistance: { label: 'Interaction Distance', type: 'slider', min: 0.1, max: 5, step: 0.1, path: 'xr.interactionDistance', description: 'Max distance for XR interactions.' },
          locomotionMethod: { label: 'Locomotion Method', type: 'select', options: [{value: 'teleport', label: 'Teleport'}, {value: 'continuous', label: 'Continuous'}], path: 'xr.locomotionMethod', description: 'Method for moving in XR.' },
          teleportRayColor: { label: 'Teleport Ray Color', type: 'colorPicker', path: 'xr.teleportRayColor', description: 'Color of the teleportation ray.' },
          controllerModel: { label: 'Controller Model Path', type: 'textInput', path: 'xr.controllerModel', description: 'Path to custom controller model (leave empty for default).', isAdvanced: true },
          roomScale: { label: 'Room Scale Factor', type: 'slider', min: 0.5, max: 2.0, step: 0.1, path: 'xr.roomScale', description: 'Scaling factor for room-scale XR experiences.'},
          spaceType: { label: 'Reference Space Type', type: 'select', options: [{value: 'local-floor', label: 'Local Floor'}, {value: 'bounded-floor', label: 'Bounded Floor'}, {value: 'unbounded', label: 'Unbounded'}], path: 'xr.spaceType', description: 'WebXR reference space type.'},
        },
      },
      handFeatures: {
        label: 'Hand Tracking & Interactions',
        settings: {
          handTracking: { label: 'Enable Hand Tracking', type: 'toggle', path: 'xr.handTracking', description: 'Enable hand tracking in XR.' },
          controllerRayColor: { label: 'Controller Ray Color', type: 'colorPicker', path: 'xr.controllerRayColor', description: 'Color of controller interaction rays.' },
          enableHaptics: { label: 'Enable Haptics', type: 'toggle', path: 'xr.enableHaptics', description: 'Enable haptic feedback in controllers.' },
          handMeshEnabled: { label: 'Show Hand Mesh', type: 'toggle', path: 'xr.handMeshEnabled', description: 'Render a mesh for tracked hands.', isAdvanced: true },
          handMeshColor: { label: 'Hand Mesh Color', type: 'colorPicker', path: 'xr.handMeshColor', description: 'Color of the hand mesh.', isAdvanced: true },
          handMeshOpacity: { label: 'Hand Mesh Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: 'xr.handMeshOpacity', description: 'Opacity of the hand mesh.', isAdvanced: true },
          handPointSize: { label: 'Hand Joint Point Size', type: 'slider', min: 0.001, max: 0.02, step: 0.001, path: 'xr.handPointSize', description: 'Size of points representing hand joints.', isAdvanced: true },
          handRayEnabled: { label: 'Enable Hand Rays', type: 'toggle', path: 'xr.handRayEnabled', description: 'Show rays originating from hands for interaction.', isAdvanced: true },
          handRayWidth: { label: 'Hand Ray Width', type: 'slider', min: 0.001, max: 0.01, step: 0.0005, path: 'xr.handRayWidth', description: 'Width of hand interaction rays.', isAdvanced: true },
          gestureSmoothing: { label: 'Gesture Smoothing', type: 'slider', min: 0, max: 1, step: 0.05, path: 'xr.gestureSmoothing', description: 'Smoothing factor for hand gestures.', isAdvanced: true },
          hapticIntensity: { label: 'Haptic Intensity', type: 'slider', min: 0, max: 1, step: 0.05, path: 'xr.hapticIntensity', description: 'Intensity of haptic feedback.' },
          dragThreshold: { label: 'Drag Threshold', type: 'slider', min: 0.01, max: 0.2, step: 0.01, path: 'xr.dragThreshold', description: 'Threshold for initiating a drag interaction.' },
          pinchThreshold: { label: 'Pinch Threshold', type: 'slider', min: 0.1, max: 0.9, step: 0.05, path: 'xr.pinchThreshold', description: 'Threshold for pinch gesture detection.' },
          rotationThreshold: { label: 'Rotation Threshold', type: 'slider', min: 0.01, max: 0.2, step: 0.01, path: 'xr.rotationThreshold', description: 'Threshold for rotation gestures.' },
          movementSpeed: { label: 'Movement Speed', type: 'slider', min: 0.01, max: 0.5, step: 0.01, path: 'xr.movementSpeed', description: 'Speed for continuous locomotion.' },
          deadZone: { label: 'Controller Dead Zone', type: 'slider', min: 0.01, max: 0.5, step: 0.01, path: 'xr.deadZone', description: 'Dead zone for controller analog sticks.' },
          interactionRadius: { label: 'Interaction Radius', type: 'slider', min: 0.05, max: 0.5, step: 0.01, path: 'xr.interactionRadius', description: 'Radius for direct hand interactions.' },
          movementAxesHorizontal: { label: 'Movement Horizontal Axis', type: 'slider', min: 0, max: 5, step: 1, path: 'xr.movementAxesHorizontal', description: 'Axis used for horizontal movement in XR.', isAdvanced: true },
          movementAxesVertical: { label: 'Movement Vertical Axis', type: 'slider', min: 0, max: 5, step: 1, path: 'xr.movementAxesVertical', description: 'Axis used for vertical movement in XR.', isAdvanced: true },
        }
      },
      environmentUnderstanding: {
        label: 'Environment Understanding',
        settings: {
          enableLightEstimation: { label: 'Enable Light Estimation', type: 'toggle', path: 'xr.enableLightEstimation', description: 'Enable light estimation for AR environments.', isAdvanced: true },
          enablePlaneDetection: { label: 'Enable Plane Detection', type: 'toggle', path: 'xr.enablePlaneDetection', description: 'Enable automatic detection of flat surfaces (planes) in AR.', isAdvanced: true },
          enableSceneUnderstanding: { label: 'Enable Scene Understanding', type: 'toggle', path: 'xr.enableSceneUnderstanding', description: 'Enable advanced scene understanding (meshes, semantics) in AR.', isAdvanced: true },
          planeColor: { label: 'Plane Color', type: 'colorPicker', path: 'xr.planeColor', description: 'Color of detected planes in AR.', isAdvanced: true },
          planeOpacity: { label: 'Plane Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: 'xr.planeOpacity', description: 'Opacity of detected planes in AR.', isAdvanced: true },
          planeDetectionDistance: { label: 'Plane Detection Distance', type: 'slider', min: 0.1, max: 10, step: 0.1, path: 'xr.planeDetectionDistance', description: 'Maximum distance for plane detection in AR.', isAdvanced: true },
          showPlaneOverlay: { label: 'Show Plane Overlay', type: 'toggle', path: 'xr.showPlaneOverlay', description: 'Show a visual overlay on detected planes.', isAdvanced: true },
          snapToFloor: { label: 'Snap to Floor', type: 'toggle', path: 'xr.snapToFloor', description: 'Automatically snap objects to detected floor planes.', isAdvanced: true },
        },
      },
      passthrough: {
        label: 'Passthrough',
        settings: {
          enablePassthroughPortal: { label: 'Enable Passthrough Portal', type: 'toggle', path: 'xr.enablePassthroughPortal', description: 'Enable a portal to the real world via passthrough camera.', isAdvanced: true },
          passthroughOpacity: { label: 'Passthrough Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: 'xr.passthroughOpacity', description: 'Opacity of the passthrough view.', isAdvanced: true },
          passthroughBrightness: { label: 'Passthrough Brightness', type: 'slider', min: 0, max: 2, step: 0.05, path: 'xr.passthroughBrightness', description: 'Brightness adjustment for passthrough.', isAdvanced: true },
          passthroughContrast: { label: 'Passthrough Contrast', type: 'slider', min: 0, max: 2, step: 0.05, path: 'xr.passthroughContrast', description: 'Contrast adjustment for passthrough.', isAdvanced: true },
          portalSize: { label: 'Portal Size', type: 'slider', min: 0.1, max: 10, step: 0.1, path: 'xr.portalSize', description: 'Size of the passthrough portal.', isAdvanced: true },
          portalEdgeColor: { label: 'Portal Edge Color', type: 'colorPicker', path: 'xr.portalEdgeColor', description: 'Color of the passthrough portal edge.', isAdvanced: true },
          portalEdgeWidth: { label: 'Portal Edge Width', type: 'slider', min: 0, max: 0.1, step: 0.001, path: 'xr.portalEdgeWidth', description: 'Width of the passthrough portal edge.', isAdvanced: true },
        },
      },
    },
  },
  ai: {
    label: 'AI Services',
    icon: 'Brain',
    subsections: {
      ragflow: {
        label: 'RAGFlow',
        settings: {
          apiKey: { label: 'API Key', type: 'textInput', path: 'ragflow.apiKey', description: 'Your RAGFlow API Key. Will be obscured.', isPowerUserOnly: true },
          agentId: { label: 'Agent ID', type: 'textInput', path: 'ragflow.agentId', description: 'RAGFlow Agent ID.', isPowerUserOnly: true },
          apiBaseUrl: { label: 'API Base URL', type: 'textInput', path: 'ragflow.apiBaseUrl', description: 'Custom RAGFlow API Base URL.', isPowerUserOnly: true, isAdvanced: true },
          timeout: { label: 'Timeout (s)', type: 'slider', unit: 's', min: 1, max: 300, step: 1, path: 'ragflow.timeout', description: 'API request timeout in seconds.' },
          maxRetries: { label: 'Max Retries', type: 'slider', min: 0, max: 10, step: 1, path: 'ragflow.maxRetries', description: 'Maximum retry attempts for API calls.' },
          chatId: { label: 'Chat ID', type: 'textInput', path: 'ragflow.chatId', description: 'Default RAGFlow Chat ID.', isPowerUserOnly: true, isAdvanced: true },
        },
      },
      perplexity: {
        label: 'Perplexity',
        settings: {
          apiKey: { label: 'API Key', type: 'textInput', path: 'perplexity.apiKey', description: 'Your Perplexity API Key. Will be obscured.', isPowerUserOnly: true },
          model: { label: 'Model', type: 'textInput', path: 'perplexity.model', description: 'Perplexity model name (e.g., llama-3.1-sonar-small-128k-online).' },
          apiUrl: { label: 'API URL', type: 'textInput', path: 'perplexity.apiUrl', description: 'Custom Perplexity API URL.', isPowerUserOnly: true, isAdvanced: true },
          maxTokens: { label: 'Max Tokens', type: 'slider', min: 1, max: 130000, step: 128, path: 'perplexity.maxTokens', description: 'Maximum tokens for API response.' }, // Adjusted max for sonar 128k
          temperature: { label: 'Temperature', type: 'slider', min: 0, max: 2, step: 0.1, path: 'perplexity.temperature', description: 'Sampling temperature.' },
          topP: { label: 'Top P', type: 'slider', min: 0, max: 1, step: 0.01, path: 'perplexity.topP', description: 'Nucleus sampling parameter.' },
          presencePenalty: { label: 'Presence Penalty', type: 'slider', min: -2, max: 2, step: 0.1, path: 'perplexity.presencePenalty', description: 'Penalty for new token presence.' },
          frequencyPenalty: { label: 'Frequency Penalty', type: 'slider', min: -2, max: 2, step: 0.1, path: 'perplexity.frequencyPenalty', description: 'Penalty for token frequency.' },
          timeout: { label: 'Timeout (s)', type: 'slider', unit: 's', min: 1, max: 300, step: 1, path: 'perplexity.timeout', description: 'API request timeout.' },
          rateLimit: { label: 'Rate Limit (req/min)', type: 'slider', min: 1, max: 1000, step: 1, path: 'perplexity.rateLimit', description: 'Requests per minute.', isAdvanced: true },
        },
      },
      openai: {
        label: 'OpenAI',
        settings: {
          apiKey: { label: 'API Key', type: 'textInput', path: 'openai.apiKey', description: 'Your OpenAI API Key. Will be obscured.', isPowerUserOnly: true },
          baseUrl: { label: 'Base URL', type: 'textInput', path: 'openai.baseUrl', description: 'Custom OpenAI Base URL (e.g., for Azure).', isPowerUserOnly: true, isAdvanced: true },
          timeout: { label: 'Timeout (s)', type: 'slider', unit: 's', min: 1, max: 300, step: 1, path: 'openai.timeout', description: 'API request timeout.' },
          rateLimit: { label: 'Rate Limit (req/min)', type: 'slider', min: 1, max: 1000, step: 1, path: 'openai.rateLimit', description: 'Requests per minute.', isAdvanced: true },
        },
      },
      kokoro: {
        label: 'Kokoro TTS',
        settings: {
          apiUrl: { label: 'API URL', type: 'textInput', path: 'kokoro.apiUrl', description: 'Kokoro TTS API URL.', isPowerUserOnly: true },
          defaultVoice: { label: 'Default Voice', type: 'textInput', path: 'kokoro.defaultVoice', description: 'Default voice for TTS.' },
          defaultFormat: { label: 'Default Format', type: 'select', options: [{value: 'mp3', label: 'MP3'}, {value: 'ogg', label: 'OGG Vorbis'}, {value: 'wav', label: 'WAV'}, {value: 'pcm', label: 'PCM'}], path: 'kokoro.defaultFormat', description: 'Default audio format.' },
          defaultSpeed: { label: 'Default Speed', type: 'slider', min: 0.25, max: 4.0, step: 0.05, path: 'kokoro.defaultSpeed', description: 'Default speech speed.' },
          timeout: { label: 'Timeout (s)', type: 'slider', unit: 's', min: 1, max: 300, step: 1, path: 'kokoro.timeout', description: 'API request timeout.' },
          stream: { label: 'Stream Audio', type: 'toggle', path: 'kokoro.stream', description: 'Enable audio streaming.' },
          returnTimestamps: { label: 'Return Timestamps', type: 'toggle', path: 'kokoro.returnTimestamps', description: 'Request word timestamps from TTS.', isAdvanced: true },
          sampleRate: { label: 'Sample Rate', type: 'select', options: [{value: '8000', label: '8000 Hz'}, {value: '16000', label: '16000 Hz'}, {value: '22050', label: '22050 Hz'}, {value: '24000', label: '24000 Hz'}, {value: '44100', label: '44100 Hz'}, {value: '48000', label: '48000 Hz'}], path: 'kokoro.sampleRate', description: 'Audio sample rate.' },
        },
      },
    },
  },
};