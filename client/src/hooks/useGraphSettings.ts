import { useSettingsStore } from '../store/settingsStore';
import { GraphSettings } from '../features/settings/config/settings';
import { getGraphSettings } from '../features/settings/utils/settingsMigration';

/**
 * Hook to get settings for a specific graph
 * @param graphName - Either 'logseq' or 'visionflow'
 * @returns The settings for the specified graph
 */
export function useGraphSettings(graphName: 'logseq' | 'visionflow'): GraphSettings {
  const settings = useSettingsStore((state) => state.settings);
  
  // Use the migration utility to handle both new and legacy structures
  return getGraphSettings(settings, graphName);
}

/**
 * Hook to update settings for a specific graph
 * @param graphName - Either 'logseq' or 'visionflow'
 * @returns A function to update the graph settings
 */
export function useUpdateGraphSettings(graphName: 'logseq' | 'visionflow') {
  const updateSettings = useSettingsStore((state) => state.updateSettings);
  
  return (updater: (draft: GraphSettings) => void) => {
    updateSettings((draft) => {
      // Ensure the graphs structure exists
      if (!draft.visualisation.graphs) {
        draft.visualisation.graphs = {
          logseq: {} as GraphSettings,
          visionflow: {} as GraphSettings,
        };
      }
      
      // Update the specific graph settings
      updater(draft.visualisation.graphs[graphName]);
    });
  };
}

/**
 * Hook to get a specific setting from a graph
 * @param graphName - Either 'logseq' or 'visionflow'
 * @param path - Path within the graph settings (e.g., 'nodes.baseColor')
 * @returns The value of the setting
 */
export function useGraphSetting<T>(
  graphName: 'logseq' | 'visionflow',
  path: string
): T {
  const graphSettings = useGraphSettings(graphName);
  
  // Navigate the settings object using the path
  let current: any = graphSettings;
  const pathParts = path.split('.');
  
  for (const part of pathParts) {
    if (current === undefined || current === null) {
      return undefined as unknown as T;
    }
    current = current[part];
  }
  
  return current as T;
}