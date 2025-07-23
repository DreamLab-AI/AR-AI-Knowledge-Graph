/**
 * Control Panel Configuration Structure
 * Defines the UI structure for dynamically modifying visualization config values
 */

export type ControlType = 
  | 'slider' 
  | 'color' 
  | 'toggle' 
  | 'select' 
  | 'number' 
  | 'text' 
  | 'vector3' 
  | 'range';

export interface ControlDefinition {
  type: ControlType;
  label: string;
  path: string; // Path in the config object, e.g., 'mainLayout.camera.fov'
  min?: number;
  max?: number;
  step?: number;
  options?: Array<{ label: string; value: any }>;
  unit?: string;
  description?: string;
  category?: string;
  subcategory?: string;
  defaultValue?: any;
  dependencies?: string[]; // Other config paths this depends on
  validate?: (value: any) => boolean;
  transform?: (value: any) => any; // Transform input before applying
}

export interface ControlGroup {
  id: string;
  label: string;
  icon?: string;
  description?: string;
  controls: ControlDefinition[];
  subgroups?: ControlGroup[];
}

export interface ControlPanelConfig {
  version: string;
  groups: ControlGroup[];
  presets?: ConfigPreset[];
}

export interface ConfigPreset {
  id: string;
  name: string;
  description?: string;
  category?: string;
  config: any; // Partial<VisualizationConfig>
  thumbnail?: string;
}

// Control panel configuration
export const CONTROL_PANEL_CONFIG: ControlPanelConfig = {
  version: '1.0.0',
  groups: [
    {
      id: 'camera',
      label: 'Camera & View',
      icon: '📷',
      description: 'Camera positioning and view controls',
      controls: [
        {
          type: 'vector3',
          label: 'Camera Position',
          path: 'mainLayout.camera.position',
          description: 'Initial camera position in 3D space',
          unit: 'units'
        },
        {
          type: 'slider',
          label: 'Field of View',
          path: 'mainLayout.camera.fov',
          min: 30,
          max: 120,
          step: 5,
          unit: '°',
          description: 'Camera field of view angle'
        },
        {
          type: 'range',
          label: 'Clipping Planes',
          path: 'mainLayout.camera',
          min: 0.01,
          max: 5000,
          step: 0.01,
          description: 'Near and far clipping planes'
        },
        {
          type: 'slider',
          label: 'Zoom Speed',
          path: 'mainLayout.controls.zoomSpeed',
          min: 0.1,
          max: 2.0,
          step: 0.1,
          defaultValue: 0.8
        },
        {
          type: 'slider',
          label: 'Pan Speed',
          path: 'mainLayout.controls.panSpeed',
          min: 0.1,
          max: 2.0,
          step: 0.1,
          defaultValue: 0.8
        },
        {
          type: 'slider',
          label: 'Rotate Speed',
          path: 'mainLayout.controls.rotateSpeed',
          min: 0.1,
          max: 2.0,
          step: 0.1,
          defaultValue: 0.8
        }
      ]
    },
    
    {
      id: 'scene',
      label: 'Scene & Lighting',
      icon: '💡',
      description: 'Scene background and lighting settings',
      controls: [
        {
          type: 'color',
          label: 'Background Color',
          path: 'mainLayout.scene.backgroundColor',
          defaultValue: '#000022'
        },
        {
          type: 'slider',
          label: 'Ambient Light Intensity',
          path: 'mainLayout.lighting.ambientIntensity',
          min: 0,
          max: 2,
          step: 0.1,
          defaultValue: 0.6
        },
        {
          type: 'slider',
          label: 'Directional Light Intensity',
          path: 'mainLayout.lighting.directionalIntensity',
          min: 0,
          max: 2,
          step: 0.1,
          defaultValue: 0.8
        },
        {
          type: 'vector3',
          label: 'Directional Light Position',
          path: 'mainLayout.lighting.directionalPosition',
          defaultValue: [1, 1, 1]
        }
      ]
    },
    
    {
      id: 'bots',
      label: 'Bots Visualization',
      icon: '🤖',
      description: 'VisionFlow bots appearance and behavior',
      subgroups: [
        {
          id: 'bots-colors',
          label: 'Bot Colors',
          controls: [
            {
              type: 'color',
              label: 'Coder',
              path: 'botsVisualization.colors.roles.coder',
              category: 'Role Colors',
              defaultValue: '#2ECC71'
            },
            {
              type: 'color',
              label: 'Tester',
              path: 'botsVisualization.colors.roles.tester',
              category: 'Role Colors',
              defaultValue: '#27AE60'
            },
            {
              type: 'color',
              label: 'Researcher',
              path: 'botsVisualization.colors.roles.researcher',
              category: 'Role Colors',
              defaultValue: '#1ABC9C'
            },
            {
              type: 'color',
              label: 'Coordinator',
              path: 'botsVisualization.colors.coordination.coordinator',
              category: 'Coordination Colors',
              defaultValue: '#F1C40F'
            },
            {
              type: 'color',
              label: 'Good Health',
              path: 'botsVisualization.colors.health.good',
              category: 'Health Status',
              defaultValue: '#2ECC71'
            },
            {
              type: 'color',
              label: 'Critical Health',
              path: 'botsVisualization.colors.health.critical',
              category: 'Health Status',
              defaultValue: '#E74C3C'
            }
          ]
        },
        {
          id: 'bots-nodes',
          label: 'Node Appearance',
          controls: [
            {
              type: 'slider',
              label: 'Base Node Size',
              path: 'botsVisualization.nodes.baseSize',
              min: 0.1,
              max: 2.0,
              step: 0.1,
              defaultValue: 0.5
            },
            {
              type: 'slider',
              label: 'Workload Scale Factor',
              path: 'botsVisualization.nodes.workloadScale',
              min: 0.5,
              max: 3.0,
              step: 0.1,
              defaultValue: 1.5
            },
            {
              type: 'slider',
              label: 'Pulse Speed',
              path: 'botsVisualization.nodes.pulseSpeedFactor',
              min: 5,
              max: 50,
              step: 5,
              defaultValue: 20
            },
            {
              type: 'slider',
              label: 'Metalness',
              path: 'botsVisualization.nodes.metalness',
              min: 0,
              max: 1,
              step: 0.1,
              defaultValue: 0.8
            },
            {
              type: 'slider',
              label: 'Roughness',
              path: 'botsVisualization.nodes.roughness',
              min: 0,
              max: 1,
              step: 0.1,
              defaultValue: 0.2
            },
            {
              type: 'slider',
              label: 'Label Font Size',
              path: 'botsVisualization.nodes.labelFontSize',
              min: 0.2,
              max: 1.0,
              step: 0.1,
              defaultValue: 0.6
            }
          ]
        },
        {
          id: 'bots-edges',
          label: 'Edge Settings',
          controls: [
            {
              type: 'number',
              label: 'Activity Timeout (ms)',
              path: 'botsVisualization.edges.activityTimeout',
              min: 1000,
              max: 10000,
              step: 1000,
              defaultValue: 5000
            },
            {
              type: 'slider',
              label: 'Particle Size',
              path: 'botsVisualization.edges.particleSize',
              min: 0.1,
              max: 1.0,
              step: 0.1,
              defaultValue: 0.4
            },
            {
              type: 'color',
              label: 'Particle Color',
              path: 'botsVisualization.edges.particleColor',
              defaultValue: '#FFD700'
            },
            {
              type: 'slider',
              label: 'Edge Opacity',
              path: 'botsVisualization.edges.edgeOpacity',
              min: 0,
              max: 1,
              step: 0.1,
              defaultValue: 0.2
            }
          ]
        }
      ]
    },
    
    {
      id: 'graph',
      label: 'Knowledge Graph',
      icon: '🌐',
      description: 'Logseq graph visualization settings',
      subgroups: [
        {
          id: 'graph-colors',
          label: 'Node Type Colors',
          controls: [
            {
              type: 'color',
              label: 'Folder',
              path: 'graphManager.colors.nodeTypes.folder',
              defaultValue: '#FFD700'
            },
            {
              type: 'color',
              label: 'File',
              path: 'graphManager.colors.nodeTypes.file',
              defaultValue: '#00CED1'
            },
            {
              type: 'color',
              label: 'Function',
              path: 'graphManager.colors.nodeTypes.function',
              defaultValue: '#FF6B6B'
            },
            {
              type: 'color',
              label: 'Class',
              path: 'graphManager.colors.nodeTypes.class',
              defaultValue: '#4ECDC4'
            }
          ]
        },
        {
          id: 'graph-material',
          label: 'Material Properties',
          controls: [
            {
              type: 'color',
              label: 'Base Color',
              path: 'graphManager.material.baseColor',
              defaultValue: '#0066ff'
            },
            {
              type: 'color',
              label: 'Emissive Color',
              path: 'graphManager.material.emissiveColor',
              defaultValue: '#00ffff'
            },
            {
              type: 'slider',
              label: 'Opacity',
              path: 'graphManager.material.opacity',
              min: 0,
              max: 1,
              step: 0.1,
              defaultValue: 0.8
            },
            {
              type: 'slider',
              label: 'Glow Strength',
              path: 'graphManager.material.glowStrength',
              min: 0,
              max: 5,
              step: 0.5,
              defaultValue: 3.0
            },
            {
              type: 'slider',
              label: 'Hologram Strength',
              path: 'graphManager.material.hologramStrength',
              min: 0,
              max: 1,
              step: 0.1,
              defaultValue: 0.8
            }
          ]
        }
      ]
    },
    
    {
      id: 'xr',
      label: 'XR & AR Settings',
      icon: '🥽',
      description: 'Quest 3 and AR-specific settings',
      controls: [
        {
          type: 'number',
          label: 'Default Update Rate (fps)',
          path: 'quest3ARLayout.performance.defaultUpdateRate',
          min: 15,
          max: 120,
          step: 15,
          defaultValue: 30
        },
        {
          type: 'number',
          label: 'Quest 3 Update Rate (fps)',
          path: 'quest3ARLayout.performance.quest3UpdateRate',
          min: 30,
          max: 120,
          step: 6,
          defaultValue: 72
        },
        {
          type: 'slider',
          label: 'Max Render Distance',
          path: 'quest3ARLayout.performance.maxRenderDistance',
          min: 50,
          max: 500,
          step: 50,
          defaultValue: 100
        }
      ]
    },
    
    {
      id: 'spacepilot',
      label: 'SpacePilot Controller',
      icon: '🎮',
      description: '6DOF controller settings',
      controls: [
        {
          type: 'slider',
          label: 'Translation Speed',
          path: 'spacePilot.controller.translationSpeed',
          min: 0.1,
          max: 5.0,
          step: 0.1,
          defaultValue: 1.0
        },
        {
          type: 'slider',
          label: 'Rotation Speed',
          path: 'spacePilot.controller.rotationSpeed',
          min: 0.01,
          max: 0.5,
          step: 0.01,
          defaultValue: 0.1
        },
        {
          type: 'slider',
          label: 'Deadzone',
          path: 'spacePilot.controller.deadzone',
          min: 0,
          max: 0.1,
          step: 0.01,
          defaultValue: 0.02
        },
        {
          type: 'slider',
          label: 'Smoothing',
          path: 'spacePilot.controller.smoothing',
          min: 0,
          max: 1,
          step: 0.05,
          defaultValue: 0.85
        },
        {
          type: 'toggle',
          label: 'Invert Pitch (RX)',
          path: 'spacePilot.controller.invertRX',
          defaultValue: true
        }
      ]
    },
    
    {
      id: 'hologram',
      label: 'Hologram Effects',
      icon: '✨',
      description: 'Holographic visualization settings',
      controls: [
        {
          type: 'color',
          label: 'Hologram Color',
          path: 'hologramManager.defaults.color',
          defaultValue: '#00ffff'
        },
        {
          type: 'slider',
          label: 'Ring Opacity',
          path: 'hologramManager.defaults.opacity',
          min: 0,
          max: 1,
          step: 0.1,
          defaultValue: 0.7
        },
        {
          type: 'slider',
          label: 'Rotation Speed',
          path: 'hologramManager.defaults.rotationSpeed',
          min: 0,
          max: 2,
          step: 0.1,
          defaultValue: 0.5
        },
        {
          type: 'text',
          label: 'Sphere Sizes',
          path: 'hologramManager.defaults.sphereSizes',
          description: 'Comma-separated values (e.g., 40,80)',
          transform: (value: string) => value.split(',').map(v => parseFloat(v.trim()))
        }
      ]
    },
    
    {
      id: 'edges',
      label: 'Edge Flow Effects',
      icon: '〰️',
      description: 'Flowing edge visualization',
      controls: [
        {
          type: 'color',
          label: 'Edge Color',
          path: 'flowingEdges.material.defaultColor',
          defaultValue: '#56b6c2'
        },
        {
          type: 'slider',
          label: 'Edge Opacity',
          path: 'flowingEdges.material.opacity',
          min: 0,
          max: 1,
          step: 0.1,
          defaultValue: 0.6
        },
        {
          type: 'slider',
          label: 'Flow Speed',
          path: 'flowingEdges.animation.flowSpeed',
          min: 0,
          max: 5,
          step: 0.5,
          defaultValue: 1.0
        },
        {
          type: 'number',
          label: 'Line Width',
          path: 'flowingEdges.material.linewidth',
          min: 1,
          max: 10,
          step: 1,
          defaultValue: 2
        }
      ]
    }
  ],
  
  presets: [
    {
      id: 'default',
      name: 'Default',
      category: 'System',
      description: 'Factory default settings',
      config: {} // Will use DEFAULT_VISUALIZATION_CONFIG
    },
    {
      id: 'cyberpunk',
      name: 'Cyberpunk',
      category: 'Themes',
      description: 'Neon colors with high contrast',
      config: {
        mainLayout: {
          scene: { backgroundColor: '#0a0a0a' },
          lighting: { ambientIntensity: 0.4, directionalIntensity: 1.2 }
        },
        botsVisualization: {
          colors: {
            roles: {
              coder: '#ff006e',
              tester: '#8338ec',
              researcher: '#3a86ff'
            }
          }
        },
        graphManager: {
          material: {
            baseColor: '#ff006e',
            emissiveColor: '#fb5607',
            glowStrength: 5.0
          }
        }
      }
    },
    {
      id: 'nature',
      name: 'Nature',
      category: 'Themes',
      description: 'Organic greens and earth tones',
      config: {
        mainLayout: {
          scene: { backgroundColor: '#1a2f1a' },
          lighting: { ambientIntensity: 0.7, directionalIntensity: 0.6 }
        },
        botsVisualization: {
          colors: {
            roles: {
              coder: '#2d6a4f',
              tester: '#40916c',
              researcher: '#52b788'
            }
          }
        }
      }
    },
    {
      id: 'performance',
      name: 'Performance Mode',
      category: 'Optimization',
      description: 'Optimized for lower-end hardware',
      config: {
        botsVisualization: {
          nodes: {
            sphereSegmentsIdle: 8,
            sphereSegmentsBusy: 16
          },
          particles: {
            ambientCount: 50
          }
        },
        graphManager: {
          nodes: {
            sphereSegments: 16
          }
        },
        hologramManager: {
          defaults: {
            segments: 32,
            segmentsLow: 16
          }
        }
      }
    },
    {
      id: 'xr-optimized',
      name: 'XR Optimized',
      category: 'XR',
      description: 'Best settings for Quest 3 AR',
      config: {
        quest3ARLayout: {
          performance: {
            defaultUpdateRate: 72,
            maxRenderDistance: 200
          }
        },
        hologramManager: {
          defaults: {
            segments: 48,
            sphereDetailHigh: 1
          }
        }
      }
    }
  ]
};

// Helper functions for control panel
export function getControlByPath(path: string): ControlDefinition | undefined {
  const findControl = (groups: ControlGroup[]): ControlDefinition | undefined => {
    for (const group of groups) {
      const control = group.controls.find(c => c.path === path);
      if (control) return control;
      
      if (group.subgroups) {
        const subControl = findControl(group.subgroups);
        if (subControl) return subControl;
      }
    }
    return undefined;
  };
  
  return findControl(CONTROL_PANEL_CONFIG.groups);
}

export function getControlsByCategory(category: string): ControlDefinition[] {
  const controls: ControlDefinition[] = [];
  
  const collectControls = (groups: ControlGroup[]) => {
    for (const group of groups) {
      controls.push(...group.controls.filter(c => c.category === category));
      if (group.subgroups) {
        collectControls(group.subgroups);
      }
    }
  };
  
  collectControls(CONTROL_PANEL_CONFIG.groups);
  return controls;
}

export function applyPreset(presetId: string): any {
  const preset = CONTROL_PANEL_CONFIG.presets?.find(p => p.id === presetId);
  return preset?.config || {};
}

export function validateControlValue(path: string, value: any): boolean {
  const control = getControlByPath(path);
  if (!control) return false;
  
  if (control.validate) {
    return control.validate(value);
  }
  
  // Default validation based on type
  switch (control.type) {
    case 'number':
    case 'slider':
      return typeof value === 'number' && 
             (!control.min || value >= control.min) && 
             (!control.max || value <= control.max);
    case 'color':
      return typeof value === 'string' && /^#[0-9A-F]{6}$/i.test(value);
    case 'toggle':
      return typeof value === 'boolean';
    case 'vector3':
      return Array.isArray(value) && value.length === 3 && 
             value.every(v => typeof v === 'number');
    default:
      return true;
  }
}

export type { ControlPanelConfig, ControlGroup, ControlDefinition, ConfigPreset };