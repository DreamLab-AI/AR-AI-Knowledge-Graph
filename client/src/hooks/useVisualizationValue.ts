/**
 * Hook to easily access and update visualization config values
 * Provides a simple interface similar to useState for config values
 */

import { useCallback } from 'react';
import { useVisualizationConfig } from '../providers/VisualizationConfigProvider';

/**
 * Hook to get and set a specific visualization config value
 * @param path - Dot-notation path to the config value (e.g., 'mainLayout.camera.fov')
 * @param defaultValue - Optional default value if the config value doesn't exist
 * @returns [value, setValue] tuple similar to useState
 */
export function useVisualizationValue<T = any>(
  path: string,
  defaultValue?: T
): [T, (value: T) => void] {
  const { getConfigValue, updateConfig } = useVisualizationConfig();
  
  const value = getConfigValue(path) ?? defaultValue;
  
  const setValue = useCallback((newValue: T) => {
    updateConfig(path, newValue);
  }, [path, updateConfig]);
  
  return [value, setValue];
}

/**
 * Hook to get multiple visualization config values at once
 * @param paths - Array of dot-notation paths
 * @returns Object with path as key and value as value
 */
export function useVisualizationValues<T extends Record<string, any>>(
  paths: string[]
): T {
  const { getConfigValue } = useVisualizationConfig();
  
  const values = {} as T;
  paths.forEach(path => {
    values[path as keyof T] = getConfigValue(path);
  });
  
  return values;
}

/**
 * Hook to get a whole section of the config
 * @param section - Top-level section name (e.g., 'mainLayout', 'botsVisualization')
 * @returns The config section object
 */
export function useVisualizationSection<T = any>(section: keyof VisualizationConfig): T {
  const { config } = useVisualizationConfig();
  return config[section] as T;
}

// Re-export the main hook for convenience
export { useVisualizationConfig } from '../providers/VisualizationConfigProvider';