import { BotsVisualConfig } from '../types/BotsTypes';

/**
 * Configuration Mapper for Bots Visualization
 * Maps and manages all configurable values for the visualization
 */
export interface VisualizationConfig {
  colors: {
    agents: BotsVisualConfig['colors'];
    health: {
      excellent: string;  // 80-100%
      good: string;       // 50-80%
      warning: string;    // 30-50%
      critical: string;   // 0-30%
    };
    edges: {
      active: string;
      inactive: string;
      particle: string;
    };
    background: {
      ambientParticles: string;
      glowEffect: string;
    };
  };
  sizes: {
    nodeBaseSize: number;
    nodeMaxSize: number;
    edgeWidth: number;
    glowScale: number;
    labelFontSize: number;
    labelFontSizeHover: number;
  };
  animation: {
    pulseSpeed: number;
    pulseAmplitude: number;
    particleSpeed: number;
    particleCount: number;
    edgeActivityThreshold: number; // milliseconds
    staleEdgeThreshold: number;    // milliseconds
  };
  physics: BotsVisualConfig['physics'];
  camera: {
    defaultPosition: [number, number, number];
    defaultTarget: [number, number, number];
    fov: number;
    near: number;
    far: number;
  };
  lighting: {
    ambientIntensity: number;
    ambientColor: string;
    directionalIntensity: number;
    directionalColor: string;
    directionalPosition: [number, number, number];
  };
  rendering: {
    metalness: number;
    roughness: number;
    emissiveIntensity: number;
    nodeOpacity: number;
    edgeOpacity: number;
    glowOpacity: number;
    enableShadows: boolean;
    enablePostProcessing: boolean;
  };
}

export class ConfigurationMapper {
  private static instance: ConfigurationMapper;
  private config: VisualizationConfig;
  private listeners: Map<string, (config: VisualizationConfig) => void> = new Map();

  private constructor() {
    this.config = this.getDefaultConfig();
  }

  static getInstance(): ConfigurationMapper {
    if (!ConfigurationMapper.instance) {
      ConfigurationMapper.instance = new ConfigurationMapper();
    }
    return ConfigurationMapper.instance;
  }

  /**
   * Get default configuration
   */
  private getDefaultConfig(): VisualizationConfig {
    return {
      colors: {
        agents: {
          // Primary agent types - Greens for roles
          coder: '#2ECC71',       // Emerald green
          tester: '#27AE60',      // Nephritis green
          researcher: '#1ABC9C',  // Turquoise
          reviewer: '#16A085',    // Green sea
          documenter: '#229954',  // Forest green
          specialist: '#239B56',  // Emerald dark
          
          // Meta roles - Golds for coordination
          coordinator: '#F1C40F', // Gold
          analyst: '#F39C12',     // Orange gold
          architect: '#E67E22',   // Carrot gold
          optimizer: '#D68910',   // Dark gold
          monitor: '#D4AC0D',     // Bright gold
        },
        health: {
          excellent: '#2ECC71',   // Green
          good: '#F1C40F',        // Gold
          warning: '#E67E22',     // Orange
          critical: '#E74C3C',    // Red
        },
        edges: {
          active: '#FFD700',      // Gold
          inactive: '#F1C40F',    // Dimmer gold
          particle: '#FFD700',    // Bright gold
        },
        background: {
          ambientParticles: '#F1C40F',
          glowEffect: '#F1C40F',
        }
      },
      sizes: {
        nodeBaseSize: 0.5,
        nodeMaxSize: 2.0,
        edgeWidth: 0.05,
        glowScale: 1.8,
        labelFontSize: 0.5,
        labelFontSizeHover: 0.6,
      },
      animation: {
        pulseSpeed: 1.0,
        pulseAmplitude: 0.1,
        particleSpeed: 1.0,
        particleCount: 8,
        edgeActivityThreshold: 5000,  // 5 seconds
        staleEdgeThreshold: 30000,    // 30 seconds
      },
      physics: {
        springStrength: 0.3,
        linkDistance: 20,
        damping: 0.95,
        nodeRepulsion: 15,
        gravityStrength: 0.1,
        maxVelocity: 0.5,
      },
      camera: {
        defaultPosition: [0, 20, 40],
        defaultTarget: [0, 0, 0],
        fov: 75,
        near: 0.1,
        far: 1000,
      },
      lighting: {
        ambientIntensity: 0.6,
        ambientColor: '#ffffff',
        directionalIntensity: 0.8,
        directionalColor: '#ffffff',
        directionalPosition: [10, 20, 10],
      },
      rendering: {
        metalness: 0.8,
        roughness: 0.2,
        emissiveIntensity: 0.3,
        nodeOpacity: 0.5,
        edgeOpacity: 0.2,
        glowOpacity: 0.1,
        enableShadows: true,
        enablePostProcessing: true,
      }
    };
  }

  /**
   * Get current configuration
   */
  getConfig(): VisualizationConfig {
    return { ...this.config };
  }

  /**
   * Update configuration
   */
  updateConfig(updates: Partial<VisualizationConfig>): void {
    this.config = this.deepMerge(this.config, updates);
    this.notifyListeners();
  }

  /**
   * Update specific path in configuration
   */
  updatePath(path: string, value: any): void {
    const keys = path.split('.');
    let current: any = this.config;
    
    for (let i = 0; i < keys.length - 1; i++) {
      if (!current[keys[i]]) {
        current[keys[i]] = {};
      }
      current = current[keys[i]];
    }
    
    current[keys[keys.length - 1]] = value;
    this.notifyListeners();
  }

  /**
   * Reset to default configuration
   */
  resetToDefault(): void {
    this.config = this.getDefaultConfig();
    this.notifyListeners();
  }

  /**
   * Subscribe to configuration changes
   */
  subscribe(id: string, callback: (config: VisualizationConfig) => void): void {
    this.listeners.set(id, callback);
  }

  /**
   * Unsubscribe from configuration changes
   */
  unsubscribe(id: string): void {
    this.listeners.delete(id);
  }

  /**
   * Notify all listeners of configuration changes
   */
  private notifyListeners(): void {
    this.listeners.forEach(callback => {
      callback(this.getConfig());
    });
  }

  /**
   * Deep merge objects
   */
  private deepMerge(target: any, source: any): any {
    const output = { ...target };
    
    if (this.isObject(target) && this.isObject(source)) {
      Object.keys(source).forEach(key => {
        if (this.isObject(source[key])) {
          if (!(key in target)) {
            Object.assign(output, { [key]: source[key] });
          } else {
            output[key] = this.deepMerge(target[key], source[key]);
          }
        } else {
          Object.assign(output, { [key]: source[key] });
        }
      });
    }
    
    return output;
  }

  /**
   * Check if value is an object
   */
  private isObject(item: any): boolean {
    return item && typeof item === 'object' && !Array.isArray(item);
  }

  /**
   * Export configuration as JSON
   */
  exportConfig(): string {
    return JSON.stringify(this.config, null, 2);
  }

  /**
   * Import configuration from JSON
   */
  importConfig(json: string): void {
    try {
      const imported = JSON.parse(json);
      this.config = this.deepMerge(this.getDefaultConfig(), imported);
      this.notifyListeners();
    } catch (error) {
      console.error('Failed to import configuration:', error);
      throw new Error('Invalid configuration JSON');
    }
  }

  /**
   * Get configuration presets
   */
  getPresets(): { [key: string]: Partial<VisualizationConfig> } {
    return {
      default: {},
      highPerformance: {
        rendering: {
          enableShadows: false,
          enablePostProcessing: false,
          nodeOpacity: 0.7,
        },
        animation: {
          particleCount: 4,
        }
      },
      darkMode: {
        colors: {
          agents: {
            coder: '#00D9FF',
            tester: '#00B8D4',
            researcher: '#00ACC1',
            reviewer: '#0097A7',
            documenter: '#00838F',
            specialist: '#006064',
            coordinator: '#FFD700',
            analyst: '#FFC107',
            architect: '#FFB300',
            optimizer: '#FFA000',
            monitor: '#FF8F00',
          },
          background: {
            ambientParticles: '#FFD700',
            glowEffect: '#FFD700',
          }
        },
        lighting: {
          ambientIntensity: 0.4,
        }
      },
      presentation: {
        sizes: {
          nodeBaseSize: 0.8,
          nodeMaxSize: 2.5,
          labelFontSize: 0.7,
          labelFontSizeHover: 0.9,
        },
        rendering: {
          emissiveIntensity: 0.5,
          nodeOpacity: 0.8,
        }
      }
    };
  }

  /**
   * Apply a preset
   */
  applyPreset(presetName: string): void {
    const presets = this.getPresets();
    if (presets[presetName]) {
      this.updateConfig(presets[presetName]);
    }
  }
}

export const configurationMapper = ConfigurationMapper.getInstance();