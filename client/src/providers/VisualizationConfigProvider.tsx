/**
 * Visualization Config Provider
 * Provides centralized configuration management for all visualization components
 * Integrates with existing settings store while providing a migration path
 */

import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { useSettingsStore } from '../store/settingsStore';
import { 
  VisualizationConfig, 
  DEFAULT_VISUALIZATION_CONFIG, 
  mergeConfigs,
  getVisualizationConfig,
  updateVisualizationConfig
} from '../config/visualization-config';
import { CONTROL_PANEL_CONFIG, applyPreset } from '../config/control-panel-config';

interface VisualizationConfigContextValue {
  config: VisualizationConfig;
  updateConfig: (path: string, value: any) => void;
  batchUpdateConfig: (updates: Record<string, any>) => void;
  resetConfig: () => void;
  applyConfigPreset: (presetId: string) => void;
  getConfigValue: (path: string) => any;
  isUsingLegacySettings: boolean;
  migrateFromLegacySettings: () => void;
}

const VisualizationConfigContext = createContext<VisualizationConfigContextValue | null>(null);

export const useVisualizationConfig = () => {
  const context = useContext(VisualizationConfigContext);
  if (!context) {
    throw new Error('useVisualizationConfig must be used within VisualizationConfigProvider');
  }
  return context;
};

// Helper to get nested value by path
function getValueByPath(obj: any, path: string): any {
  return path.split('.').reduce((curr, key) => curr?.[key], obj);
}

// Helper to set nested value by path
function setValueByPath(obj: any, path: string, value: any): any {
  const keys = path.split('.');
  const lastKey = keys.pop()!;
  const target = keys.reduce((curr, key) => {
    if (!curr[key]) curr[key] = {};
    return curr[key];
  }, obj);
  target[lastKey] = value;
  return obj;
}

// Migration helper to convert legacy settings to new config
function migrateSettingsToConfig(settings: any): Partial<VisualizationConfig> {
  const migrated: any = {};
  
  // Migrate visualisation settings
  if (settings?.visualisation) {
    const vis = settings.visualisation;
    
    // Scene settings
    if (vis.scene) {
      migrated.mainLayout = {
        ...migrated.mainLayout,
        scene: {
          backgroundColor: vis.scene.backgroundColor || DEFAULT_VISUALIZATION_CONFIG.mainLayout.scene.backgroundColor,
          backgroundColorRGB: vis.scene.backgroundColorRGB || DEFAULT_VISUALIZATION_CONFIG.mainLayout.scene.backgroundColorRGB
        }
      };
    }
    
    // Lighting settings
    if (vis.lighting) {
      migrated.mainLayout = {
        ...migrated.mainLayout,
        lighting: {
          ambientIntensity: vis.lighting.ambientIntensity ?? DEFAULT_VISUALIZATION_CONFIG.mainLayout.lighting.ambientIntensity,
          directionalIntensity: vis.lighting.directionalIntensity ?? DEFAULT_VISUALIZATION_CONFIG.mainLayout.lighting.directionalIntensity,
          directionalPosition: vis.lighting.directionalPosition || DEFAULT_VISUALIZATION_CONFIG.mainLayout.lighting.directionalPosition
        }
      };
    }
    
    // VisionFlow (Bots) settings
    if (vis.graphs?.visionflow) {
      const vf = vis.graphs.visionflow;
      if (vf.nodes?.baseColor) {
        migrated.botsVisualization = {
          ...migrated.botsVisualization,
          colors: {
            ...migrated.botsVisualization?.colors,
            coordination: {
              ...DEFAULT_VISUALIZATION_CONFIG.botsVisualization.colors.coordination,
              coordinator: vf.nodes.baseColor
            }
          }
        };
      }
      if (vf.edges?.color) {
        migrated.flowingEdges = {
          ...migrated.flowingEdges,
          material: {
            ...migrated.flowingEdges?.material,
            defaultColor: vf.edges.color
          }
        };
      }
    }
    
    // Logseq (Graph) settings
    if (vis.graphs?.logseq) {
      const logseq = vis.graphs.logseq;
      if (logseq.nodes) {
        migrated.graphManager = {
          ...migrated.graphManager,
          material: {
            baseColor: logseq.nodes.baseColor || DEFAULT_VISUALIZATION_CONFIG.graphManager.material.baseColor,
            emissiveColor: logseq.nodes.baseColor || DEFAULT_VISUALIZATION_CONFIG.graphManager.material.emissiveColor,
            opacity: logseq.nodes.opacity ?? DEFAULT_VISUALIZATION_CONFIG.graphManager.material.opacity,
            glowStrength: vis.animations?.pulseStrength || DEFAULT_VISUALIZATION_CONFIG.graphManager.material.glowStrength,
            pulseSpeed: DEFAULT_VISUALIZATION_CONFIG.graphManager.material.pulseSpeed,
            hologramStrength: DEFAULT_VISUALIZATION_CONFIG.graphManager.material.hologramStrength,
            rimPower: DEFAULT_VISUALIZATION_CONFIG.graphManager.material.rimPower
          }
        };
      }
    }
    
    // Hologram settings
    if (vis.hologram) {
      migrated.hologramManager = {
        defaults: {
          ...DEFAULT_VISUALIZATION_CONFIG.hologramManager.defaults,
          color: vis.hologram.ringColor || vis.hologram.color || DEFAULT_VISUALIZATION_CONFIG.hologramManager.defaults.color,
          opacity: vis.hologram.ringOpacity ?? DEFAULT_VISUALIZATION_CONFIG.hologramManager.defaults.opacity,
          rotationSpeed: vis.hologram.ringRotationSpeed ?? DEFAULT_VISUALIZATION_CONFIG.hologramManager.defaults.rotationSpeed,
          sphereSizes: vis.hologram.sphereSizes || DEFAULT_VISUALIZATION_CONFIG.hologramManager.defaults.sphereSizes,
          triangleSphereSize: vis.hologram.triangleSphereSize || DEFAULT_VISUALIZATION_CONFIG.hologramManager.defaults.triangleSphereSize,
          triangleSphereOpacity: vis.hologram.triangleSphereOpacity || DEFAULT_VISUALIZATION_CONFIG.hologramManager.defaults.triangleSphereOpacity
        }
      };
    }
  }
  
  return migrated;
}

export const VisualizationConfigProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const settings = useSettingsStore(state => state.settings);
  const updateSettings = useSettingsStore(state => state.updateSettings);
  
  const [config, setConfig] = useState<VisualizationConfig>(() => {
    // Initialize with default config
    return getVisualizationConfig();
  });
  
  const [isUsingLegacySettings, setIsUsingLegacySettings] = useState(true);
  
  // Sync with legacy settings on mount and when settings change
  useEffect(() => {
    if (settings && isUsingLegacySettings) {
      const migrated = migrateSettingsToConfig(settings);
      const newConfig = mergeConfigs(DEFAULT_VISUALIZATION_CONFIG, migrated);
      setConfig(newConfig);
      updateVisualizationConfig(migrated);
    }
  }, [settings, isUsingLegacySettings]);
  
  const getConfigValue = useCallback((path: string): any => {
    return getValueByPath(config, path);
  }, [config]);
  
  const updateConfig = useCallback((path: string, value: any) => {
    setConfig(prev => {
      const updated = JSON.parse(JSON.stringify(prev)); // Deep clone
      setValueByPath(updated, path, value);
      updateVisualizationConfig(updated);
      
      // If still using legacy settings, update them too
      if (isUsingLegacySettings) {
        // Map back to legacy settings structure
        const legacyPath = mapToLegacyPath(path);
        if (legacyPath) {
          updateSettings(settings => {
            const newSettings = JSON.parse(JSON.stringify(settings));
            setValueByPath(newSettings, legacyPath, value);
            return newSettings;
          });
        }
      }
      
      return updated;
    });
  }, [isUsingLegacySettings, updateSettings]);
  
  const batchUpdateConfig = useCallback((updates: Record<string, any>) => {
    setConfig(prev => {
      let updated = JSON.parse(JSON.stringify(prev)); // Deep clone
      
      Object.entries(updates).forEach(([path, value]) => {
        setValueByPath(updated, path, value);
      });
      
      updateVisualizationConfig(updated);
      
      // If still using legacy settings, batch update them too
      if (isUsingLegacySettings) {
        const legacyUpdates: Record<string, any> = {};
        Object.entries(updates).forEach(([path, value]) => {
          const legacyPath = mapToLegacyPath(path);
          if (legacyPath) {
            legacyUpdates[legacyPath] = value;
          }
        });
        
        if (Object.keys(legacyUpdates).length > 0) {
          updateSettings(settings => {
            const newSettings = JSON.parse(JSON.stringify(settings));
            Object.entries(legacyUpdates).forEach(([path, value]) => {
              setValueByPath(newSettings, path, value);
            });
            return newSettings;
          });
        }
      }
      
      return updated;
    });
  }, [isUsingLegacySettings, updateSettings]);
  
  const resetConfig = useCallback(() => {
    setConfig(DEFAULT_VISUALIZATION_CONFIG);
    updateVisualizationConfig({});
  }, []);
  
  const applyConfigPreset = useCallback((presetId: string) => {
    const presetConfig = applyPreset(presetId);
    if (presetConfig) {
      const newConfig = mergeConfigs(config, presetConfig);
      setConfig(newConfig);
      updateVisualizationConfig(presetConfig);
    }
  }, [config]);
  
  const migrateFromLegacySettings = useCallback(() => {
    setIsUsingLegacySettings(false);
    // The current config already has migrated values, just stop syncing
  }, []);
  
  const value: VisualizationConfigContextValue = {
    config,
    updateConfig,
    batchUpdateConfig,
    resetConfig,
    applyConfigPreset,
    getConfigValue,
    isUsingLegacySettings,
    migrateFromLegacySettings
  };
  
  return (
    <VisualizationConfigContext.Provider value={value}>
      {children}
    </VisualizationConfigContext.Provider>
  );
};

// Helper to map new config paths to legacy settings paths
function mapToLegacyPath(configPath: string): string | null {
  const mappings: Record<string, string> = {
    'mainLayout.scene.backgroundColor': 'visualisation.scene.backgroundColor',
    'mainLayout.lighting.ambientIntensity': 'visualisation.lighting.ambientIntensity',
    'mainLayout.lighting.directionalIntensity': 'visualisation.lighting.directionalIntensity',
    'botsVisualization.colors.coordination.coordinator': 'visualisation.graphs.visionflow.nodes.baseColor',
    'graphManager.material.baseColor': 'visualisation.graphs.logseq.nodes.baseColor',
    'graphManager.material.opacity': 'visualisation.graphs.logseq.nodes.opacity',
    'hologramManager.defaults.color': 'visualisation.hologram.ringColor',
    'hologramManager.defaults.opacity': 'visualisation.hologram.ringOpacity',
    'flowingEdges.material.defaultColor': 'visualisation.graphs.visionflow.edges.color',
    // Add more mappings as needed
  };
  
  return mappings[configPath] || null;
}