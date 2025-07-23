/**
 * Centralized configuration for all visualization hardcoded values
 * This replaces hardcoded values across the visualization components
 */

export interface VisualizationConfig {
  mainLayout: {
    camera: {
      position: [number, number, number];
      fov: number;
      near: number;
      far: number;
    };
    scene: {
      backgroundColor: string;
      backgroundColorRGB: [number, number, number];
    };
    lighting: {
      ambientIntensity: number;
      directionalIntensity: number;
      directionalPosition: [number, number, number];
    };
    controls: {
      zoomSpeed: number;
      panSpeed: number;
      rotateSpeed: number;
    };
  };
  
  botsVisualization: {
    colors: {
      roles: Record<string, string>;
      coordination: Record<string, string>;
      health: {
        good: string;
        medium: string;
        warning: string;
        critical: string;
      };
    };
    nodes: {
      baseSize: number;
      workloadScale: number;
      pulseSpeedFactor: number;
      pulseAmplitude: number;
      glowScale: number;
      glowOpacityBase: number;
      glowOpacityHover: number;
      cpuOpacityFactor: number;
      healthBorderInnerRadius: number;
      healthBorderOuterRadius: number;
      healthBorderSegments: number;
      healthBorderOpacity: number;
      glowSize: number;
      glowSegments: number;
      glowOpacity: number;
      labelFontSize: number;
      labelFontSizeHover: number;
      labelOutlineWidth: number;
      labelOffsetY: number;
      sphereSegmentsIdle: number;
      sphereSegmentsBusy: number;
      metalness: number;
      roughness: number;
      materialOpacity: number;
    };
    edges: {
      activityTimeout: number;
      staleTimeout: number;
      animationSpeedFactor: number;
      particleCount: number;
      particleSize: number;
      particleColor: string;
      particleOpacity: number;
      cylinderRadius: number;
      cylinderSegments: number;
      edgeOpacity: number;
    };
    particles: {
      ambientCount: number;
      ambientSize: number;
      ambientOpacity: number;
      ambientSpread: number;
    };
    update: {
      pollInterval: number;
    };
  };
  
  graphManager: {
    colors: {
      nodeTypes: Record<string, string>;
    };
    nodes: {
      baseSize: number;
      connectionScaleFactor: number;
      typeImportance: Record<string, number>;
      baseSphereRadius: number;
      sphereSegments: number;
      edgeOffsetGap: number;
      minEdgeLength: number;
    };
    positioning: {
      goldenAngle: number;
      scaleFactorMin: number;
      scaleFactorMax: number;
    };
    material: {
      baseColor: string;
      emissiveColor: string;
      opacity: number;
      glowStrength: number;
      pulseSpeed: number;
      hologramStrength: number;
      rimPower: number;
    };
    labels: {
      offsetYFactor: number;
      offsetYBase: number;
      desktopFontSize: number;
      textColor: string;
      textOutlineWidth: number;
      textOutlineColor: string;
      maxWidth: number;
      subtextScale: number;
      subtextOffsetY: number;
      subtextMaxWidth: number;
    };
    animation: {
      pulseFrequency: number;
      pulseAmplitude: number;
    };
  };
  
  quest3ARLayout: {
    performance: {
      defaultUpdateRate: number;
      quest3UpdateRate: number;
      maxRenderDistance: number;
    };
    ui: {
      voiceControlsBottom: string;
      voiceButtonSize: string;
      statusIndicatorTop: string;
      statusIndicatorLeft: string;
      statusIndicatorPadding: string;
      statusIndicatorBorderRadius: string;
      statusIndicatorFontSize: string;
      debugInfoTop: string;
      debugInfoLeft: string;
      debugInfoPadding: string;
      debugInfoBorderRadius: string;
      debugInfoFontSize: string;
      debugInfoMaxWidth: string;
    };
    colors: {
      statusIndicatorBg: string;
      statusIndicatorColor: string;
      statusIndicatorBorder: string;
      debugInfoBg: string;
      debugInfoColor: string;
      debugInfoBorder: string;
      voiceButtonBg: string;
      voiceIndicatorBg: string;
    };
    effects: {
      backdropBlur: string;
      borderOpacity: number;
    };
  };
  
  spacePilot: {
    controller: {
      translationSpeed: number;
      rotationSpeed: number;
      rotationSpeedRY: number;
      deadzone: number;
      smoothing: number;
      invertRX: boolean;
      normalizationScale: number;
    };
    camera: {
      resetDistance: number;
      resetTheta: number;
      resetPhi: number;
      resetTarget: [number, number, number];
    };
    buttons: Record<string, string>;
  };
  
  hologramManager: {
    defaults: {
      size: number;
      color: string;
      opacity: number;
      sphereOpacity: number;
      triangleSphereOpacity: number;
      rotationSpeed: number;
      sphereRotationSpeed: number;
      segments: number;
      segmentsLow: number;
      sphereDetail: number;
      sphereDetailHigh: number;
      sphereSizes: number[];
      triangleSphereSize: number;
      sizeConversion: number;
      ringInnerRadiusFactor: number;
      rotationSpeedIncrement: number;
      sphereRotationFactor: number;
    };
    geometry: {
      ringRotationFactorX: number;
      ringRotationFactorY: number;
    };
    rendering: {
      defaultLayer: number;
      bloomLayer: number;
      depthWrite: boolean;
    };
  };
  
  flowingEdges: {
    material: {
      defaultColor: string;
      opacity: number;
      linewidth: number;
      alphaTest: number;
      depthWrite: boolean;
      renderOrder: number;
    };
    animation: {
      flowSpeed: number;
      opacityMin: number;
      opacityMax: number;
      opacityRange: number;
    };
    shader: {
      sinWaveMultiplier: number;
      flowPowerExponent: number;
      glowPowerExponent: number;
      colorBoostFactor: number;
      alphaBoostFactor: number;
    };
  };
}

// Default configuration values
export const DEFAULT_VISUALIZATION_CONFIG: VisualizationConfig = {
  mainLayout: {
    camera: {
      position: [0, 20, 60],
      fov: 75,
      near: 0.1,
      far: 2000
    },
    scene: {
      backgroundColor: '#000022',
      backgroundColorRGB: [0, 0, 0.05]
    },
    lighting: {
      ambientIntensity: 0.6,
      directionalIntensity: 0.8,
      directionalPosition: [1, 1, 1]
    },
    controls: {
      zoomSpeed: 0.8,
      panSpeed: 0.8,
      rotateSpeed: 0.8
    }
  },
  
  botsVisualization: {
    colors: {
      roles: {
        coder: '#2ECC71',
        tester: '#27AE60',
        researcher: '#1ABC9C',
        reviewer: '#16A085',
        documenter: '#229954',
        specialist: '#239B56'
      },
      coordination: {
        coordinator: '#F1C40F', // Will be replaced by settings.baseColor
        analyst: '#F39C12',
        architect: '#E67E22',
        optimizer: '#D68910',
        monitor: '#D4AC0D'
      },
      health: {
        good: '#2ECC71',
        medium: '#F1C40F',
        warning: '#E67E22',
        critical: '#E74C3C'
      }
    },
    nodes: {
      baseSize: 0.5,
      workloadScale: 1.5,
      pulseSpeedFactor: 20,
      pulseAmplitude: 0.1,
      glowScale: 1.2,
      glowOpacityBase: 0.2,
      glowOpacityHover: 0.2,
      cpuOpacityFactor: 200,
      healthBorderInnerRadius: 1.3,
      healthBorderOuterRadius: 1.5,
      healthBorderSegments: 32,
      healthBorderOpacity: 0.8,
      glowSize: 1.8,
      glowSegments: 16,
      glowOpacity: 0.1,
      labelFontSize: 0.6,
      labelFontSizeHover: 0.6,
      labelOutlineWidth: 0.05,
      labelOffsetY: 2,
      sphereSegmentsIdle: 16,
      sphereSegmentsBusy: 32,
      metalness: 0.8,
      roughness: 0.2,
      materialOpacity: 0.5
    },
    edges: {
      activityTimeout: 5000,
      staleTimeout: 30000,
      animationSpeedFactor: 5,
      particleCount: 8,
      particleSize: 0.4,
      particleColor: '#FFD700',
      particleOpacity: 0.95,
      cylinderRadius: 0.05,
      cylinderSegments: 8,
      edgeOpacity: 0.2
    },
    particles: {
      ambientCount: 200,
      ambientSize: 0.05,
      ambientOpacity: 0.3,
      ambientSpread: 60
    },
    update: {
      pollInterval: 3000
    }
  },
  
  graphManager: {
    colors: {
      nodeTypes: {
        folder: '#FFD700',
        file: '#00CED1',
        function: '#FF6B6B',
        class: '#4ECDC4',
        variable: '#95E1D3',
        import: '#F38181',
        export: '#AA96DA',
        default: '#00ffff'
      }
    },
    nodes: {
      baseSize: 1.0,
      connectionScaleFactor: 0.3,
      typeImportance: {
        folder: 1.5,
        function: 1.3,
        class: 1.4,
        file: 1.0,
        variable: 0.8,
        import: 0.7,
        export: 0.9,
        default: 1.0
      },
      baseSphereRadius: 0.5,
      sphereSegments: 32,
      edgeOffsetGap: 0.1,
      minEdgeLength: 0.2
    },
    positioning: {
      goldenAngle: Math.PI * (3 - Math.sqrt(5)),
      scaleFactorMin: 15,
      scaleFactorMax: 20
    },
    material: {
      baseColor: '#0066ff',
      emissiveColor: '#00ffff',
      opacity: 0.8,
      glowStrength: 3.0,
      pulseSpeed: 1.0,
      hologramStrength: 0.8,
      rimPower: 3.0
    },
    labels: {
      offsetYFactor: 1.5,
      offsetYBase: 0.5,
      desktopFontSize: 0.2,
      textColor: '#ffffff',
      textOutlineWidth: 0.02,
      textOutlineColor: '#000000',
      maxWidth: 3,
      subtextScale: 0.6,
      subtextOffsetY: -0.15,
      subtextMaxWidth: 2
    },
    animation: {
      pulseFrequency: 3,
      pulseAmplitude: 0.1
    }
  },
  
  quest3ARLayout: {
    performance: {
      defaultUpdateRate: 30,
      quest3UpdateRate: 72,
      maxRenderDistance: 100
    },
    ui: {
      voiceControlsBottom: '40px',
      voiceButtonSize: 'lg',
      statusIndicatorTop: '20px',
      statusIndicatorLeft: '20px',
      statusIndicatorPadding: '8px 12px',
      statusIndicatorBorderRadius: '20px',
      statusIndicatorFontSize: '14px',
      debugInfoTop: '60px',
      debugInfoLeft: '20px',
      debugInfoPadding: '12px',
      debugInfoBorderRadius: '8px',
      debugInfoFontSize: '12px',
      debugInfoMaxWidth: '300px'
    },
    colors: {
      statusIndicatorBg: 'rgba(0, 255, 0, 0.8)',
      statusIndicatorColor: 'black',
      statusIndicatorBorder: 'rgba(255, 255, 255, 0.3)',
      debugInfoBg: 'rgba(0, 0, 0, 0.8)',
      debugInfoColor: 'white',
      debugInfoBorder: 'rgba(255, 255, 255, 0.2)',
      voiceButtonBg: 'bg-blue-500 bg-opacity-90',
      voiceIndicatorBg: 'bg-black bg-opacity-70'
    },
    effects: {
      backdropBlur: '10px',
      borderOpacity: 0.3
    }
  },
  
  spacePilot: {
    controller: {
      translationSpeed: 1.0,
      rotationSpeed: 0.1,
      rotationSpeedRY: 0.02,
      deadzone: 0.02,
      smoothing: 0.85,
      invertRX: true,
      normalizationScale: 450
    },
    camera: {
      resetDistance: 50,
      resetTheta: Math.PI / 4,
      resetPhi: 0,
      resetTarget: [0, 0, 0]
    },
    buttons: {
      '1': 'resetView'
    }
  },
  
  hologramManager: {
    defaults: {
      size: 1,
      color: '#00ffff',
      opacity: 0.7,
      sphereOpacity: 0.5,
      triangleSphereOpacity: 0.3,
      rotationSpeed: 0.5,
      sphereRotationSpeed: 0.2,
      segments: 64,
      segmentsLow: 32,
      sphereDetail: 1,
      sphereDetailHigh: 2,
      sphereSizes: [40, 80],
      triangleSphereSize: 60,
      sizeConversion: 100,
      ringInnerRadiusFactor: 0.8,
      rotationSpeedIncrement: 0.2,
      sphereRotationFactor: 0.5
    },
    geometry: {
      ringRotationFactorX: Math.PI / 3,
      ringRotationFactorY: Math.PI / 6
    },
    rendering: {
      defaultLayer: 0,
      bloomLayer: 1,
      depthWrite: false
    }
  },
  
  flowingEdges: {
    material: {
      defaultColor: '#56b6c2',
      opacity: 0.6,
      linewidth: 2,
      alphaTest: 0.01,
      depthWrite: true,
      renderOrder: 5
    },
    animation: {
      flowSpeed: 1.0,
      opacityMin: 0.7,
      opacityMax: 1.0,
      opacityRange: 0.3
    },
    shader: {
      sinWaveMultiplier: 10.0,
      flowPowerExponent: 3.0,
      glowPowerExponent: 2.0,
      colorBoostFactor: 0.5,
      alphaBoostFactor: 0.5
    }
  }
};

// Helper function to deep merge configurations
export function mergeConfigs(
  defaultConfig: VisualizationConfig,
  userConfig: Partial<VisualizationConfig>
): VisualizationConfig {
  const merge = (target: any, source: any): any => {
    const output = { ...target };
    if (isObject(target) && isObject(source)) {
      Object.keys(source).forEach(key => {
        if (isObject(source[key])) {
          if (!(key in target)) {
            Object.assign(output, { [key]: source[key] });
          } else {
            output[key] = merge(target[key], source[key]);
          }
        } else {
          Object.assign(output, { [key]: source[key] });
        }
      });
    }
    return output;
  };
  
  const isObject = (item: any): item is Record<string, any> => {
    return item && typeof item === 'object' && !Array.isArray(item);
  };
  
  return merge(defaultConfig, userConfig);
}

// Export singleton instance
let configInstance: VisualizationConfig = DEFAULT_VISUALIZATION_CONFIG;

export function getVisualizationConfig(): VisualizationConfig {
  return configInstance;
}

export function updateVisualizationConfig(updates: Partial<VisualizationConfig>): void {
  configInstance = mergeConfigs(configInstance, updates);
}

export function resetVisualizationConfig(): void {
  configInstance = DEFAULT_VISUALIZATION_CONFIG;
}