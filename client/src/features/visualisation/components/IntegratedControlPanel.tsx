import React, { useState, useEffect, useRef } from 'react';
import { SpaceDriver } from '../../../services/SpaceDriverService';
import { useSettingsStore } from '../../../store/settingsStore';

interface IntegratedControlPanelProps {
  showStats: boolean;
  enableBloom: boolean;
  onOrbitControlsToggle?: (enabled: boolean) => void;
  botsData?: {
    nodeCount: number;
    edgeCount: number;
    tokenCount: number;
    mcpConnected: boolean;
    dataSource: string;
  };
}

interface SettingField {
  key: string;
  label: string;
  type: 'slider' | 'toggle' | 'color' | 'nostr-button' | 'text' | 'select';
  path: string;
  min?: number;
  max?: number;
  options?: string[];
}

// Map SpacePilot buttons to menu sections
const BUTTON_MENU_MAP = {
  '1': { id: 'appearance', label: 'Appearance' },
  '2': { id: 'physics', label: 'Physics' },
  '3': { id: 'visual', label: 'Visual FX' },
  '4': { id: 'auth', label: 'Auth' },
  '5': { id: 'data', label: 'Data' },
  '6': { id: 'info', label: 'Info' }
};

// Navigation button mappings
const NAV_BUTTONS = {
  '7': 'up',     // Up button
  'A': 'down',   // Down button (hex A = 10)
  '9': 'right',  // Right button
  '8': 'left',   // Left button
  'F': 'commit'  // F button to commit (hex F = 15)
};

export const IntegratedControlPanel: React.FC<IntegratedControlPanelProps> = ({
  showStats,
  enableBloom,
  onOrbitControlsToggle,
  botsData
}) => {
  const [isExpanded, setIsExpanded] = useState(true);
  const [spacePilotConnected, setSpacePilotConnected] = useState(false);
  const [spacePilotButtons, setSpacePilotButtons] = useState<string[]>([]);
  const [webHidAvailable, setWebHidAvailable] = useState(false);
  const [spacePilotRawInput, setSpacePilotRawInput] = useState({
    translation: { x: 0, y: 0, z: 0 },
    rotation: { rx: 0, ry: 0, rz: 0 }
  });

  // Menu state
  const [activeSection, setActiveSection] = useState<string | null>(null);
  const [selectedFieldIndex, setSelectedFieldIndex] = useState(0);
  const [selectedValue, setSelectedValue] = useState<number>(0);

  // Nostr auth state
  const [nostrConnected, setNostrConnected] = useState(false);
  const [nostrPublicKey, setNostrPublicKey] = useState<string>('');

  // Settings store access
  const settings = useSettingsStore(state => state.settings);
  const updateSettings = useSettingsStore(state => state.updateSettings);

  // Ref for scrollable container
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Helper function to get values from settings path
  const getValueFromPath = (path: string): any => {
    const keys = path.split('.');
    let value = settings;
    for (const key of keys) {
      value = value?.[key];
    }
    return value;
  };

  // Check WebHID availability and initialize Nostr state
  useEffect(() => {
    setWebHidAvailable('hid' in navigator);

    // Initialize Nostr state from settings
    const nostrConnected = getValueFromPath('auth.nostr.connected');
    const nostrPubkey = getValueFromPath('auth.nostr.publicKey');

    if (nostrConnected && nostrPubkey) {
      setNostrConnected(true);
      setNostrPublicKey(nostrPubkey);
    }
  }, []);

  // Auto-scroll to selected item when section changes or index changes
  useEffect(() => {
    if (scrollContainerRef.current && activeSection) {
      const selectedElement = scrollContainerRef.current.querySelector(`[data-field-index="${selectedFieldIndex}"]`);
      if (selectedElement) {
        selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }
    }
  }, [activeSection, selectedFieldIndex]);

  // Set up SpacePilot event listeners
  useEffect(() => {
    const handleConnect = () => {
      setSpacePilotConnected(true);
      if (onOrbitControlsToggle) {
        onOrbitControlsToggle(false);
      }
    };

    const handleDisconnect = () => {
      setSpacePilotConnected(false);
      setSpacePilotButtons([]);
      if (onOrbitControlsToggle) {
        onOrbitControlsToggle(true);
      }
    };

    const handleButtons = (event: any) => {
      const buttons = event.detail.buttons || [];
      setSpacePilotButtons(buttons);

      // Handle menu section buttons (1-6)
      buttons.forEach((btn: string) => {
        const btnNum = btn.replace('[', '').replace(']', '');
        if (BUTTON_MENU_MAP[btnNum]) {
          setActiveSection(BUTTON_MENU_MAP[btnNum].id);
          setSelectedFieldIndex(0); // Reset to first item when changing sections
        }

        // Handle navigation buttons
        if (NAV_BUTTONS[btnNum]) {
          handleNavigation(NAV_BUTTONS[btnNum]);
        }
      });
    };

    const handleTranslate = (event: any) => {
      setSpacePilotRawInput(prev => ({
        ...prev,
        translation: {
          x: event.detail.x || 0,
          y: event.detail.y || 0,
          z: event.detail.z || 0
        }
      }));
    };

    const handleRotate = (event: any) => {
      setSpacePilotRawInput(prev => ({
        ...prev,
        rotation: {
          rx: event.detail.rx || 0,
          ry: event.detail.ry || 0,
          rz: event.detail.rz || 0
        }
      }));
    };

    SpaceDriver.addEventListener('connect', handleConnect);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);
    SpaceDriver.addEventListener('buttons', handleButtons);
    SpaceDriver.addEventListener('translate', handleTranslate);
    SpaceDriver.addEventListener('rotate', handleRotate);

    return () => {
      SpaceDriver.removeEventListener('connect', handleConnect);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
      SpaceDriver.removeEventListener('buttons', handleButtons);
      SpaceDriver.removeEventListener('translate', handleTranslate);
      SpaceDriver.removeEventListener('rotate', handleRotate);
    };
  }, [onOrbitControlsToggle, activeSection, selectedFieldIndex]);

  const handleConnectClick = async () => {
    try {
      await SpaceDriver.scan();
    } catch (error) {
      console.error('Failed to connect to SpacePilot:', error);
    }
  };

  const handleNavigation = (action: string) => {
    if (!activeSection) return;

    const sectionSettings = getSectionSettings();
    if (!sectionSettings) return;

    const fieldCount = sectionSettings.fields.length;

    switch (action) {
      case 'up':
        setSelectedFieldIndex(prev => {
          const newIndex = Math.max(0, prev - 1);
          // Auto-scroll to selected item when navigating up
          if (scrollContainerRef.current && newIndex !== prev) {
            setTimeout(() => {
              const container = scrollContainerRef.current;
              const selectedElement = container?.querySelector(`[data-field-index="${newIndex}"]`);
              if (selectedElement) {
                selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
              }
            }, 0);
          }
          return newIndex;
        });
        break;
      case 'down':
        setSelectedFieldIndex(prev => {
          const newIndex = Math.min(fieldCount - 1, prev + 1);
          // Auto-scroll to selected item when navigating down
          if (scrollContainerRef.current && newIndex !== prev) {
            setTimeout(() => {
              const container = scrollContainerRef.current;
              const selectedElement = container?.querySelector(`[data-field-index="${newIndex}"]`);
              if (selectedElement) {
                selectedElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
              }
            }, 0);
          }
          return newIndex;
        });
        break;
      case 'left':
      case 'right':
        handleValueAdjustment(action);
        break;
      case 'commit':
        handleCommitSettings();
        break;
    }
  };

  const handleValueAdjustment = (direction: string) => {
    const sectionSettings = getSectionSettings();
    if (!sectionSettings) return;

    const field = sectionSettings.fields[selectedFieldIndex];
    if (!field) return;

    const currentValue = getValueFromPath(field.path);

    switch (field.type) {
      case 'slider':
        const min = field.min ?? 0;
        const max = field.max ?? 1;
        const current = typeof currentValue === 'number' && !isNaN(currentValue) ? currentValue : min;
        const step = (max - min) / 20; // 5% steps
        const delta = direction === 'right' ? step : -step;
        const newValue = Math.max(min, Math.min(max, current + delta));
        updateSettingByPath(field.path, newValue);
        break;
      case 'toggle':
        updateSettingByPath(field.path, !currentValue);
        break;
      case 'nostr-button':
        // Toggle between login/logout
        if (nostrConnected) {
          handleNostrLogout();
        } else {
          handleNostrLogin();
        }
        break;
      case 'color':
        // For color, cycle through preset colors
        const colors = ['#FF0000', '#00FF00', '#0000FF', '#FFFF00', '#FF00FF', '#00FFFF', '#FFFFFF'];
        const currentIndex = colors.indexOf(currentValue) || 0;
        const nextIndex = direction === 'right'
          ? (currentIndex + 1) % colors.length
          : (currentIndex - 1 + colors.length) % colors.length;
        updateSettingByPath(field.path, colors[nextIndex]);
        break;
      case 'select':
        const options = field.options || [];
        const currentSelectIndex = options.indexOf(currentValue) || 0;
        const nextSelectIndex = direction === 'right'
          ? (currentSelectIndex + 1) % options.length
          : (currentSelectIndex - 1 + options.length) % options.length;
        updateSettingByPath(field.path, options[nextSelectIndex]);
        break;
      case 'text':
        // Text fields cannot be adjusted with left/right, only edited directly
        // Could implement character-by-character editing in the future
        break;
    }
  };

  const updateSettingByPath = (path: string, value: any) => {
    updateSettings((draft) => {
      const keys = path.split('.');
      let obj: any = draft;
      for (let i = 0; i < keys.length - 1; i++) {
        if (!obj[keys[i]]) obj[keys[i]] = {};
        obj = obj[keys[i]];
      }
      obj[keys[keys.length - 1]] = value;
    });
  };

  const handleCommitSettings = () => {
    // Settings are already committed via updateSettings
    // This could trigger additional actions like saving to localStorage
    // or sending telemetry
    console.log('Settings committed for section:', activeSection);
  };

  const handleNostrLogin = async () => {
    try {
      // Check if window.nostr is available (NIP-07)
      if (!window.nostr) {
        alert('Nostr extension not found. Please install a Nostr browser extension like nos2x or Alby.');
        return;
      }

      const publicKey = await window.nostr.getPublicKey();
      setNostrConnected(true);
      setNostrPublicKey(publicKey);

      // Store in settings
      updateSettingByPath('auth.nostr.connected', true);
      updateSettingByPath('auth.nostr.publicKey', publicKey);

      console.log('Nostr connected:', { publicKey: publicKey.slice(0, 8) + '...' });
    } catch (error) {
      console.error('Nostr login failed:', error);
      alert('Failed to connect to Nostr. Please try again.');
    }
  };

  const handleNostrLogout = () => {
    setNostrConnected(false);
    setNostrPublicKey('');

    // Clear from settings
    updateSettingByPath('auth.nostr.connected', false);
    updateSettingByPath('auth.nostr.publicKey', '');

    console.log('Nostr disconnected');
  };

  // Get section settings based on active section
  const getSectionSettings = () => {
    switch (activeSection) {
      case 'appearance':
        return {
          title: 'Appearance Settings',
          fields: [
            // Node Settings
            { key: 'nodeColor', label: 'Node Color', type: 'color', path: 'visualisation.graphs.logseq.nodes.baseColor' },
            { key: 'nodeSize', label: 'Node Size', type: 'slider', min: 0.2, max: 2, path: 'visualisation.graphs.logseq.nodes.nodeSize' },
            { key: 'nodeMetalness', label: 'Node Metalness', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.nodes.metalness' },
            { key: 'nodeOpacity', label: 'Node Opacity', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.graphs.logseq.nodes.opacity' },
            { key: 'nodeRoughness', label: 'Node Roughness', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.nodes.roughness' },
            { key: 'enableInstancing', label: 'Enable Instancing', type: 'toggle', path: 'visualisation.graphs.logseq.nodes.enableInstancing' },
            { key: 'enableMetadataShape', label: 'Metadata Shape', type: 'toggle', path: 'visualisation.graphs.logseq.nodes.enableMetadataShape' },
            { key: 'enableMetadataVis', label: 'Metadata Visual', type: 'toggle', path: 'visualisation.graphs.logseq.nodes.enableMetadataVisualisation' },

            // Edge Settings
            { key: 'edgeColor', label: 'Edge Color', type: 'color', path: 'visualisation.graphs.logseq.edges.color' },
            { key: 'edgeWidth', label: 'Edge Width', type: 'slider', min: 0.01, max: 2, path: 'visualisation.graphs.logseq.edges.baseWidth' },
            { key: 'edgeOpacity', label: 'Edge Opacity', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.edges.opacity' },
            { key: 'enableArrows', label: 'Show Arrows', type: 'toggle', path: 'visualisation.graphs.logseq.edges.enableArrows' },
            { key: 'arrowSize', label: 'Arrow Size', type: 'slider', min: 0.01, max: 0.5, path: 'visualisation.graphs.logseq.edges.arrowSize' },
            { key: 'glowStrength', label: 'Edge Glow', type: 'slider', min: 0, max: 5, path: 'visualisation.graphs.logseq.edges.glowStrength' },

            // Label Settings
            { key: 'enableLabels', label: 'Show Labels', type: 'toggle', path: 'visualisation.graphs.logseq.labels.enableLabels' },
            { key: 'labelSize', label: 'Label Size', type: 'slider', min: 0.01, max: 1.5, path: 'visualisation.graphs.logseq.labels.desktopFontSize' },
            { key: 'labelColor', label: 'Label Color', type: 'color', path: 'visualisation.graphs.logseq.labels.textColor' },
            { key: 'labelOutlineColor', label: 'Outline Color', type: 'color', path: 'visualisation.graphs.logseq.labels.textOutlineColor' },
            { key: 'labelOutlineWidth', label: 'Outline Width', type: 'slider', min: 0, max: 0.01, path: 'visualisation.graphs.logseq.labels.textOutlineWidth' },

            // Lighting
            { key: 'ambientLight', label: 'Ambient Light', type: 'slider', min: 0, max: 2, path: 'visualisation.rendering.ambientLightIntensity' },
            { key: 'directionalLight', label: 'Direct Light', type: 'slider', min: 0, max: 2, path: 'visualisation.rendering.directionalLightIntensity' }
          ]
        };
      case 'physics':
        return {
          title: 'Physics Settings',
          fields: [
            { key: 'enabled', label: 'Physics Enabled', type: 'toggle', path: 'visualisation.graphs.logseq.physics.enabled' },
            { key: 'damping', label: 'Damping', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.physics.damping' },
            { key: 'repulsion', label: 'Repulsion', type: 'slider', min: 0, max: 2, path: 'visualisation.graphs.logseq.physics.repulsionStrength' },
            { key: 'attraction', label: 'Attraction', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.physics.attractionStrength' },
            { key: 'springStrength', label: 'Spring Strength', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.physics.springStrength' },
            { key: 'maxVelocity', label: 'Max Velocity', type: 'slider', min: 0.001, max: 0.5, path: 'visualisation.graphs.logseq.physics.maxVelocity' },
            { key: 'iterations', label: 'Iterations', type: 'slider', min: 10, max: 500, path: 'visualisation.graphs.logseq.physics.iterations' },
            { key: 'enableBounds', label: 'Enable Bounds', type: 'toggle', path: 'visualisation.graphs.logseq.physics.enableBounds' },
            { key: 'boundsSize', label: 'Bounds Size', type: 'slider', min: 1, max: 50, path: 'visualisation.graphs.logseq.physics.boundsSize' },
            { key: 'collisionRadius', label: 'Collision Radius', type: 'slider', min: 0.1, max: 5, path: 'visualisation.graphs.logseq.physics.collisionRadius' },
            { key: 'repulsionDistance', label: 'Repulsion Dist', type: 'slider', min: 0.1, max: 10, path: 'visualisation.graphs.logseq.physics.repulsionDistance' },
            { key: 'massScale', label: 'Mass Scale', type: 'slider', min: 0.1, max: 10, path: 'visualisation.graphs.logseq.physics.massScale' },
            { key: 'boundaryDamping', label: 'Boundary Damp', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.physics.boundaryDamping' },
            { key: 'updateThreshold', label: 'Update Threshold', type: 'slider', min: 0, max: 0.5, path: 'visualisation.graphs.logseq.physics.updateThreshold' }
          ]
        };
      case 'visual':
        return {
          title: 'Visual Effects',
          fields: [
            // Bloom Settings
            { key: 'bloom', label: 'Bloom', type: 'toggle', path: 'visualisation.bloom.enabled' },
            { key: 'bloomStrength', label: 'Bloom Strength', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.bloom.strength' },
            { key: 'bloomRadius', label: 'Bloom Radius', type: 'slider', min: 0, max: 1, path: 'visualisation.bloom.radius' },
            { key: 'bloomThreshold', label: 'Bloom Threshold', type: 'slider', min: 0, max: 1, path: 'visualisation.bloom.threshold' },
            { key: 'nodeBloomStrength', label: 'Node Bloom', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.bloom.nodeBloomStrength' },
            { key: 'edgeBloomStrength', label: 'Edge Bloom', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.bloom.edgeBloomStrength' },
            { key: 'envBloomStrength', label: 'Env Bloom', type: 'slider', min: 0, max: 1, step: 0.01, path: 'visualisation.bloom.environmentBloomStrength' },

            // Hologram Settings
            { key: 'hologram', label: 'Hologram', type: 'toggle', path: 'visualisation.graphs.logseq.nodes.enableHologram' },
            { key: 'ringCount', label: 'Ring Count', type: 'slider', min: 0, max: 10, path: 'visualisation.graphs.logseq.hologram.ringCount' },
            { key: 'ringColor', label: 'Ring Color', type: 'color', path: 'visualisation.graphs.logseq.hologram.ringColor' },
            { key: 'ringOpacity', label: 'Ring Opacity', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.hologram.ringOpacity' },
            { key: 'ringRotationSpeed', label: 'Ring Speed', type: 'slider', min: 0, max: 50, path: 'visualisation.graphs.logseq.hologram.ringRotationSpeed' },

            // Flow & Animation
            { key: 'flowEffect', label: 'Edge Flow', type: 'toggle', path: 'visualisation.graphs.logseq.edges.enableFlowEffect' },
            { key: 'flowSpeed', label: 'Flow Speed', type: 'slider', min: 0.1, max: 5, path: 'visualisation.graphs.logseq.edges.flowSpeed' },
            { key: 'flowIntensity', label: 'Flow Intensity', type: 'slider', min: 0, max: 10, path: 'visualisation.graphs.logseq.edges.flowIntensity' },
            { key: 'useGradient', label: 'Edge Gradient', type: 'toggle', path: 'visualisation.graphs.logseq.edges.useGradient' },
            { key: 'distanceIntensity', label: 'Distance Int', type: 'slider', min: 0, max: 10, path: 'visualisation.graphs.logseq.edges.distanceIntensity' },

            // Animation Settings
            { key: 'nodeAnimations', label: 'Node Animations', type: 'toggle', path: 'visualisation.animation.enableNodeAnimations' },
            { key: 'pulseEnabled', label: 'Pulse Effect', type: 'toggle', path: 'visualisation.animation.pulseEnabled' },
            { key: 'pulseSpeed', label: 'Pulse Speed', type: 'slider', min: 0.1, max: 2, path: 'visualisation.animation.pulseSpeed' },
            { key: 'pulseStrength', label: 'Pulse Strength', type: 'slider', min: 0.1, max: 2, path: 'visualisation.animation.pulseStrength' },
            { key: 'selectionWave', label: 'Selection Wave', type: 'toggle', path: 'visualisation.animation.enableSelectionWave' },
            { key: 'waveSpeed', label: 'Wave Speed', type: 'slider', min: 0.1, max: 2, path: 'visualisation.animation.waveSpeed' },

            // Advanced Rendering
            { key: 'antialiasing', label: 'Antialiasing', type: 'toggle', path: 'visualisation.rendering.enableAntialiasing' },
            { key: 'shadows', label: 'Shadows', type: 'toggle', path: 'visualisation.rendering.enableShadows' },
            { key: 'ambientOcclusion', label: 'Ambient Occl', type: 'toggle', path: 'visualisation.rendering.enableAmbientOcclusion' }
          ]
        };
      case 'auth':
        return {
          title: 'Authentication Settings',
          fields: [
            { key: 'nostr', label: 'Nostr Login', type: 'nostr-button', path: 'auth.nostr' },
            { key: 'enabled', label: 'Auth Required', type: 'toggle', path: 'auth.enabled' },
            { key: 'required', label: 'Require Auth', type: 'toggle', path: 'auth.required' },
            { key: 'provider', label: 'Auth Provider', type: 'text', path: 'auth.provider' }
          ]
        };
      case 'data':
        return {
          title: 'Data Settings',
          fields: [
            // General Data Settings
            { key: 'autoSave', label: 'Auto Save', type: 'toggle', path: 'data.autoSave' },
            { key: 'saveInterval', label: 'Save Interval (min)', type: 'slider', min: 1, max: 30, path: 'data.saveInterval' },
            { key: 'cacheSize', label: 'Cache Size (MB)', type: 'slider', min: 10, max: 500, path: 'data.cacheSize' },
            { key: 'compression', label: 'Data Compression', type: 'toggle', path: 'data.compression' },

            // WebSocket Settings
            { key: 'wsUpdateRate', label: 'Update Rate (Hz)', type: 'slider', min: 1, max: 60, path: 'system.websocket.updateRate' },
            { key: 'wsReconnectAttempts', label: 'Reconnect Tries', type: 'slider', min: 0, max: 10, path: 'system.websocket.reconnectAttempts' },
            { key: 'wsReconnectDelay', label: 'Reconnect Delay', type: 'slider', min: 500, max: 10000, path: 'system.websocket.reconnectDelay' },
            { key: 'wsCompression', label: 'WS Compression', type: 'toggle', path: 'system.websocket.compression.enabled' },
            { key: 'wsCompressionThreshold', label: 'Compress Size', type: 'slider', min: 64, max: 4096, path: 'system.websocket.compression.threshold' },

            // SpacePilot Settings
            { key: 'spTranslationX', label: 'SP Trans X Sens', type: 'slider', min: 0.1, max: 5, path: 'system.spacepilot.sensitivity.translation.x' },
            { key: 'spTranslationY', label: 'SP Trans Y Sens', type: 'slider', min: 0.1, max: 5, path: 'system.spacepilot.sensitivity.translation.y' },
            { key: 'spTranslationZ', label: 'SP Trans Z Sens', type: 'slider', min: 0.1, max: 5, path: 'system.spacepilot.sensitivity.translation.z' },
            { key: 'spRotationX', label: 'SP Rot X Sens', type: 'slider', min: 0.1, max: 5, path: 'system.spacepilot.sensitivity.rotation.rx' },
            { key: 'spRotationY', label: 'SP Rot Y Sens', type: 'slider', min: 0.1, max: 5, path: 'system.spacepilot.sensitivity.rotation.ry' },
            { key: 'spRotationZ', label: 'SP Rot Z Sens', type: 'slider', min: 0.1, max: 5, path: 'system.spacepilot.sensitivity.rotation.rz' },
            { key: 'spDeadzone', label: 'SP Deadzone', type: 'slider', min: 0, max: 30, path: 'system.spacepilot.deadzone' },
            { key: 'spSmoothing', label: 'SP Smoothing', type: 'slider', min: 0, max: 95, path: 'system.spacepilot.smoothing' }
          ]
        };
      case 'info':
        return {
          title: 'System Information',
          fields: [
            // Debug Settings
            { key: 'enableDebug', label: 'Debug Mode', type: 'toggle', path: 'system.debug.enableClientDebugMode' },
            { key: 'showFPS', label: 'Show FPS', type: 'toggle', path: 'system.debug.showFPS' },
            { key: 'showMemory', label: 'Show Memory', type: 'toggle', path: 'system.debug.showMemory' },
            { key: 'perfDebug', label: 'Performance Debug', type: 'toggle', path: 'system.debug.enablePerformanceDebug' },
            { key: 'logLevel', label: 'Log Level', type: 'slider', min: 0, max: 3, path: 'system.debug.logLevel' },
            { key: 'telemetry', label: 'Telemetry', type: 'toggle', path: 'system.debug.enableTelemetry' },

            // Debug Toggles
            { key: 'dataDebug', label: 'Data Debug', type: 'toggle', path: 'system.debug.enableDataDebug' },
            { key: 'wsDebug', label: 'WebSocket Debug', type: 'toggle', path: 'system.debug.enableWebSocketDebug' },
            { key: 'physicsDebug', label: 'Physics Debug', type: 'toggle', path: 'system.debug.enablePhysicsDebug' },
            { key: 'nodeDebug', label: 'Node Debug', type: 'toggle', path: 'system.debug.enableNodeDebug' },
            { key: 'shaderDebug', label: 'Shader Debug', type: 'toggle', path: 'system.debug.enableShaderDebug' },
            { key: 'matrixDebug', label: 'Matrix Debug', type: 'toggle', path: 'system.debug.enableMatrixDebug' },

            // System Settings
            { key: 'persistSettings', label: 'Persist Settings', type: 'toggle', path: 'system.persistSettingsOnServer' },
            { key: 'customBackendURL', label: 'Custom Backend URL', type: 'text', path: 'system.customBackendUrl' },

            // XR Settings
            { key: 'xrEnabled', label: 'XR Mode', type: 'toggle', path: 'xr.enableXrMode' },
            { key: 'xrQuality', label: 'XR Quality', type: 'select', options: ['Low', 'Medium', 'High'], path: 'xr.quality' },
            { key: 'xrRenderScale', label: 'XR Render Scale', type: 'slider', min: 0.5, max: 2, path: 'xr.renderScale' },
            { key: 'handTracking', label: 'Hand Tracking', type: 'toggle', path: 'xr.handTracking.enabled' },
            { key: 'enableHaptics', label: 'Haptics', type: 'toggle', path: 'xr.interactions.enableHaptics' }
          ]
        };
      default:
        return null;
    }
  };

  if (!isExpanded) {
    // Collapsed state
    return (
      <div style={{
        position: 'absolute',
        top: 10,
        left: 10,
        width: '40px',
        height: '40px',
        backgroundColor: 'rgba(0,0,0,0.8)',
        border: '1px solid rgba(255,255,255,0.3)',
        borderRadius: '5px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        cursor: 'pointer'
      }}
      onClick={() => setIsExpanded(true)}
      >
        <div style={{
          width: '12px',
          height: '12px',
          backgroundColor: '#E74C3C',
          borderRadius: '50%',
          boxShadow: '0 0 5px rgba(231, 76, 60, 0.5)'
        }} />
      </div>
    );
  }

  const sectionSettings = getSectionSettings();

  // Expanded state
  return (
    <div style={{
      position: 'absolute',
      top: 10,
      left: 10,
      color: 'white',
      fontFamily: 'monospace',
      fontSize: '12px',
      backgroundColor: 'rgba(0,0,0,0.85)',
      padding: '10px',
      borderRadius: '5px',
      border: '1px solid rgba(255,255,255,0.3)',
      minWidth: '320px',
      maxWidth: '400px',
      maxHeight: '80vh',
      overflowY: 'auto'
    }}>
      {/* Header with fold button */}
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        marginBottom: '10px'
      }}>
        <div style={{ fontWeight: 'bold' }}>Control Center</div>
        <div
          style={{
            width: '16px',
            height: '16px',
            backgroundColor: '#2ECC71',
            borderRadius: '50%',
            cursor: 'pointer',
            boxShadow: '0 0 5px rgba(46, 204, 113, 0.5)'
          }}
          onClick={() => setIsExpanded(false)}
          title="Fold panel"
        />
      </div>

      {/* System info */}
      <div style={{ marginBottom: '10px', paddingBottom: '10px', borderBottom: '1px solid rgba(255,255,255,0.2)' }}>
        <div>Stats: {showStats ? 'ON' : 'OFF'}</div>
        <div>Bloom: {enableBloom ? 'ON' : 'OFF'}</div>
      </div>

      {/* VisionFlow Status */}
      {botsData && (
        <div style={{ marginBottom: '10px', paddingBottom: '10px', borderBottom: '1px solid rgba(255,255,255,0.2)' }}>
          <div style={{ fontWeight: 'bold', marginBottom: '8px', color: '#F1C40F' }}>
            ⚡ VisionFlow ({botsData.dataSource.toUpperCase()})
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: 'auto auto', gap: '3px 15px', fontSize: '11px' }}>
            <span style={{ opacity: 0.7 }}>Agents:</span>
            <span style={{ color: '#F1C40F' }}>{botsData.nodeCount}</span>
            <span style={{ opacity: 0.7 }}>Links:</span>
            <span style={{ color: '#F1C40F' }}>{botsData.edgeCount}</span>
            <span style={{ opacity: 0.7 }}>Tokens:</span>
            <span style={{ color: '#F39C12' }}>{botsData.tokenCount.toLocaleString()}</span>
          </div>
        </div>
      )}

      {/* SpacePilot Menu Controls */}
      <div>
        <div style={{ fontWeight: 'bold', marginBottom: '8px' }}>SpacePilot Controls</div>

        {/* Connection status */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
          <span style={{ fontSize: '11px' }}>Status:</span>
          {!webHidAvailable ? (
            <span style={{ fontSize: '11px', color: '#E74C3C' }}>WebHID not available</span>
          ) : spacePilotConnected ? (
            <>
              <span style={{
                width: '8px',
                height: '8px',
                borderRadius: '50%',
                backgroundColor: '#2ECC71',
                display: 'inline-block'
              }}></span>
              <span style={{ fontSize: '11px', color: '#2ECC71' }}>Connected</span>
            </>
          ) : (
            <>
              <span style={{
                width: '8px',
                height: '8px',
                borderRadius: '50%',
                backgroundColor: '#E74C3C',
                display: 'inline-block'
              }}></span>
              <button
                onClick={handleConnectClick}
                style={{
                  background: '#3498DB',
                  color: 'white',
                  border: 'none',
                  borderRadius: '3px',
                  padding: '2px 8px',
                  fontSize: '11px',
                  cursor: 'pointer'
                }}
              >
                Connect
              </button>
            </>
          )}
        </div>

        {/* Menu Buttons (1-6) - Always visible for mouse control */}
        <div style={{ fontSize: '10px', marginBottom: '4px', opacity: 0.7 }}>Menu Sections:</div>
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(3, 1fr)',
          gap: '4px',
          marginBottom: '12px'
        }}>
          {Object.entries(BUTTON_MENU_MAP).map(([btnNum, menu]) => {
            const isPressed = spacePilotConnected && spacePilotButtons.includes(`[${btnNum}]`);
            const isActive = activeSection === menu.id;
            return (
              <div
                key={btnNum}
                style={{
                  padding: '6px 8px',
                  borderRadius: '3px',
                  border: `1px solid ${isActive ? '#F1C40F' : (isPressed ? '#2ECC71' : '#555')}`,
                  background: isActive ? 'rgba(241, 196, 15, 0.2)' : (isPressed ? 'rgba(46, 204, 113, 0.1)' : 'rgba(255, 255, 255, 0.05)'),
                  fontSize: '11px',
                  color: isActive ? '#F1C40F' : (isPressed ? '#2ECC71' : '#888'),
                  transition: 'all 0.1s',
                  cursor: 'pointer',
                  textAlign: 'center'
                }}
                onClick={() => {
                  setActiveSection(menu.id);
                  setSelectedFieldIndex(0); // Reset to first item when changing sections
                }}
              >
                {btnNum}. {menu.label}
              </div>
            );
          })}
        </div>

        {/* Active Section Settings - Always visible for mouse control */}
        {activeSection && sectionSettings && (
          <div
            ref={scrollContainerRef}
            style={{
            marginTop: '12px',
            padding: '8px',
            border: '1px solid rgba(241, 196, 15, 0.3)',
            borderRadius: '4px',
            background: 'rgba(241, 196, 15, 0.05)',
            maxHeight: '400px',
            overflowY: 'auto',
            // Custom scrollbar styling
            scrollbarWidth: 'thin',
            scrollbarColor: 'rgba(241, 196, 15, 0.3) rgba(255,255,255,0.1)'
          }}>
            <div style={{ fontWeight: 'bold', marginBottom: '8px', color: '#F1C40F' }}>
              {sectionSettings.title}
            </div>
            <div style={{ fontSize: '11px' }}>
              {sectionSettings.fields.map((field, index) => {
                const currentValue = getValueFromPath(field.path);
                const isSelected = selectedFieldIndex === index;

                return (
                  <div
                    key={field.key}
                    data-field-index={index}
                    style={{
                      padding: '6px',
                      marginBottom: '4px',
                      background: isSelected ? 'rgba(241, 196, 15, 0.1)' : 'transparent',
                      border: isSelected ? '1px solid rgba(241, 196, 15, 0.3)' : '1px solid transparent',
                      borderRadius: '3px',
                      transition: 'all 0.1s',
                      cursor: 'pointer'
                    }}
                    onClick={() => setSelectedFieldIndex(index)}
                  >
                    <div style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      marginBottom: '2px'
                    }}>
                      <span>{field.label}</span>
                      {isSelected && (
                        <span style={{ fontSize: '9px', color: '#F1C40F' }}>◀ ▶</span>
                      )}
                    </div>

                    {/* Value display based on type */}
                    <div style={{
                      opacity: 0.8,
                      fontSize: '10px',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px'
                    }}>
                      {field.type === 'slider' && (
                        <>
                          <div style={{
                            flex: 1,
                            height: '4px',
                            background: 'rgba(255,255,255,0.1)',
                            borderRadius: '2px',
                            position: 'relative',
                            cursor: 'pointer'
                          }}
                          onClick={(e) => {
                            e.stopPropagation();
                            const rect = e.currentTarget.getBoundingClientRect();
                            const x = e.clientX - rect.left;
                            const percentage = x / rect.width;
                            const newValue = field.min + (field.max - field.min) * percentage;
                            updateSettingByPath(field.path, newValue);
                          }}>
                            <div style={{
                              position: 'absolute',
                              left: 0,
                              top: 0,
                              height: '100%',
                              width: `${((currentValue - field.min) / (field.max - field.min)) * 100}%`,
                              background: isSelected ? '#F1C40F' : '#888',
                              borderRadius: '2px',
                              transition: 'width 0.1s',
                              pointerEvents: 'none'
                            }} />
                          </div>
                          <span style={{ minWidth: '40px', textAlign: 'right' }}>
                            {typeof currentValue === 'number' ? currentValue.toFixed(1) : '0.0'}
                          </span>
                        </>
                      )}

                      {field.type === 'toggle' && (
                        <div style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '4px',
                          cursor: 'pointer'
                        }}
                        onClick={(e) => {
                          e.stopPropagation();
                          updateSettingByPath(field.path, !currentValue);
                        }}>
                          <div style={{
                            width: '30px',
                            height: '16px',
                            borderRadius: '8px',
                            background: currentValue ? '#2ECC71' : '#555',
                            position: 'relative',
                            transition: 'background 0.2s'
                          }}>
                            <div style={{
                              position: 'absolute',
                              top: '2px',
                              left: currentValue ? '16px' : '2px',
                              width: '12px',
                              height: '12px',
                              borderRadius: '50%',
                              background: 'white',
                              transition: 'left 0.2s'
                            }} />
                          </div>
                          <span>{currentValue ? 'ON' : 'OFF'}</span>
                        </div>
                      )}

                      {field.type === 'color' && (
                        <div style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '4px'
                        }}>
                          <input
                            type="color"
                            value={currentValue || '#888888'}
                            onChange={(e) => {
                              updateSettingByPath(field.path, e.target.value);
                            }}
                            style={{
                              width: '20px',
                              height: '20px',
                              border: 'none',
                              borderRadius: '3px',
                              cursor: 'pointer'
                            }}
                            onClick={(e) => e.stopPropagation()}
                          />
                          <span>{currentValue || '#888888'}</span>
                        </div>
                      )}

                      {field.type === 'nostr-button' && (
                        <div style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '6px'
                        }}>
                          <div style={{
                            padding: '4px 8px',
                            borderRadius: '3px',
                            background: nostrConnected ? '#2ECC71' : '#3498DB',
                            border: `1px solid ${nostrConnected ? '#27AE60' : '#2980B9'}`,
                            fontSize: '9px',
                            color: 'white',
                            cursor: 'pointer',
                            transition: 'all 0.2s'
                          }}
                          onClick={(e) => {
                            e.stopPropagation();
                            if (nostrConnected) {
                              handleNostrLogout();
                            } else {
                              handleNostrLogin();
                            }
                          }}>
                            {nostrConnected ? 'LOGOUT' : 'LOGIN'}
                          </div>
                          {nostrConnected && (
                            <span style={{ fontSize: '9px', opacity: 0.7 }}>
                              {nostrPublicKey.slice(0, 8)}...
                            </span>
                          )}
                        </div>
                      )}

                      {field.type === 'text' && (
                        <div style={{
                          flex: 1,
                          padding: '3px 6px',
                          background: 'rgba(255,255,255,0.1)',
                          border: '1px solid rgba(255,255,255,0.2)',
                          borderRadius: '3px',
                          fontSize: '10px',
                          color: '#CCC',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap'
                        }}>
                          {currentValue || 'Not set'}
                        </div>
                      )}

                      {field.type === 'select' && (
                        <div style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: '4px'
                        }}>
                          <select
                            value={currentValue || field.options?.[0] || ''}
                            onChange={(e) => {
                              updateSettingByPath(field.path, e.target.value);
                            }}
                            onClick={(e) => e.stopPropagation()}
                            style={{
                              padding: '3px 8px',
                              background: isSelected ? 'rgba(241, 196, 15, 0.2)' : 'rgba(255,255,255,0.1)',
                              border: `1px solid ${isSelected ? 'rgba(241, 196, 15, 0.3)' : 'rgba(255,255,255,0.2)'}`,
                              borderRadius: '3px',
                              fontSize: '10px',
                              color: isSelected ? '#F1C40F' : '#CCC',
                              cursor: 'pointer'
                            }}
                          >
                            {field.options?.map(option => (
                              <option key={option} value={option}>{option}</option>
                            ))}
                          </select>
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>

            {/* Navigation hints */}
            <div style={{
              marginTop: '8px',
              paddingTop: '8px',
              borderTop: '1px solid rgba(255,255,255,0.2)',
              fontSize: '9px',
              opacity: 0.6
            }}>
              {spacePilotConnected ? '7/A: Navigate ↑↓ | 8/9: Adjust ←→ | F: Commit' : 'Click to select and adjust controls'}
            </div>
          </div>
        )}

        {/* Raw input values (debug) - Only show when SpacePilot is connected */}
        {spacePilotConnected && (
          <div style={{
            marginTop: '8px',
            paddingTop: '8px',
            borderTop: '1px solid rgba(255,255,255,0.2)',
            fontSize: '10px',
            fontFamily: 'monospace',
            opacity: 0.5
          }}>
            <div style={{ marginBottom: '4px' }}>Raw Input:</div>
            <div style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(3, 1fr)',
              gap: '5px',
              fontSize: '9px'
            }}>
              <div>X: {spacePilotRawInput.translation.x}</div>
              <div>Y: {spacePilotRawInput.translation.y}</div>
              <div>Z: {spacePilotRawInput.translation.z}</div>
              <div>RX: {spacePilotRawInput.rotation.rx}</div>
              <div>RY: {spacePilotRawInput.rotation.ry}</div>
              <div>RZ: {spacePilotRawInput.rotation.rz}</div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};