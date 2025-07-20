import { Settings, GraphSettings } from '../config/settings';
import { defaultSettings } from '../config/defaultSettings';

/**
 * Migrates settings from the old structure (visualisation.nodes/edges/labels/physics)
 * to the new multi-graph structure (visualisation.graphs.logseq/visionflow)
 */
export function migrateToMultiGraphSettings(settings: Settings): Settings {
  // Check if already migrated
  if (settings.visualisation.graphs?.logseq && settings.visualisation.graphs?.visionflow) {
    // Already migrated, just clean up legacy fields
    const migrated = { ...settings };
    delete migrated.visualisation.nodes;
    delete migrated.visualisation.edges;
    delete migrated.visualisation.labels;
    delete migrated.visualisation.physics;
    return migrated;
  }

  // Create migrated settings
  const migrated = { ...settings };

  // Initialize graphs structure if it doesn't exist
  if (!migrated.visualisation.graphs) {
    migrated.visualisation.graphs = {
      logseq: {} as GraphSettings,
      visionflow: {} as GraphSettings,
    };
  }

  // Helper to get legacy value or default
  const getLegacyOrDefault = (legacyValue: any, defaultValue: any) => {
    return legacyValue !== undefined ? legacyValue : defaultValue;
  };

  // Migrate settings to both graphs
  // For logseq, use the legacy settings if they exist
  migrated.visualisation.graphs.logseq = {
    nodes: getLegacyOrDefault(
      settings.visualisation.nodes,
      defaultSettings.visualisation.graphs.logseq.nodes
    ),
    edges: getLegacyOrDefault(
      settings.visualisation.edges,
      defaultSettings.visualisation.graphs.logseq.edges
    ),
    labels: getLegacyOrDefault(
      settings.visualisation.labels,
      defaultSettings.visualisation.graphs.logseq.labels
    ),
    physics: getLegacyOrDefault(
      settings.visualisation.physics,
      defaultSettings.visualisation.graphs.logseq.physics
    ),
  };

  // For visionflow, use the default green theme
  migrated.visualisation.graphs.visionflow = {
    nodes: defaultSettings.visualisation.graphs.visionflow.nodes,
    edges: defaultSettings.visualisation.graphs.visionflow.edges,
    labels: defaultSettings.visualisation.graphs.visionflow.labels,
    physics: defaultSettings.visualisation.graphs.visionflow.physics,
  };

  // Clean up legacy fields
  delete migrated.visualisation.nodes;
  delete migrated.visualisation.edges;
  delete migrated.visualisation.labels;
  delete migrated.visualisation.physics;

  return migrated;
}

/**
 * Gets settings for a specific graph, with fallback to legacy settings
 * This is useful during the transition period
 */
export function getGraphSettings(
  settings: Settings,
  graphName: 'logseq' | 'visionflow'
): GraphSettings {
  // If new structure exists, use it
  if (settings.visualisation.graphs?.[graphName]) {
    return settings.visualisation.graphs[graphName];
  }

  // Fallback to legacy settings for backward compatibility
  if (settings.visualisation.nodes || 
      settings.visualisation.edges || 
      settings.visualisation.labels || 
      settings.visualisation.physics) {
    return {
      nodes: settings.visualisation.nodes || defaultSettings.visualisation.graphs[graphName].nodes,
      edges: settings.visualisation.edges || defaultSettings.visualisation.graphs[graphName].edges,
      labels: settings.visualisation.labels || defaultSettings.visualisation.graphs[graphName].labels,
      physics: settings.visualisation.physics || defaultSettings.visualisation.graphs[graphName].physics,
    };
  }

  // Default to the graph-specific defaults
  return defaultSettings.visualisation.graphs[graphName];
}