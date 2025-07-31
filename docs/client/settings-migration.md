# Settings Migration Utility

This document describes the settings migration utility that handles the transition from the legacy flat settings structure to the new multi-graph architecture.

## Overview

The settings migration utility (`client/src/features/settings/utils/settingsMigration.ts`) provides automatic migration and compatibility functions for transitioning between settings structures.

## Migration Functions

### `migrateToMultiGraphSettings(settings: Settings): Settings`

Automatically migrates settings from the old flat structure to the new multi-graph structure.

**Migration Process:**
1. **Detection**: Checks if settings are already migrated by looking for `visualisation.graphs.logseq` and `visualisation.graphs.visionflow`
2. **Migration**: If not migrated:
   - Creates the `graphs` namespace structure
   - Copies legacy settings to the `logseq` graph
   - Initializes `visionflow` graph with default green theme
   - Removes deprecated flat structure fields
3. **Cleanup**: Removes legacy fields to prevent confusion

**Example Migration:**
```typescript
// Before migration
const oldSettings = {
  visualisation: {
    nodes: { baseColor: '#4B5EFF', nodeSize: 8 },
    edges: { color: '#F59E0B', baseWidth: 2 },
    physics: { enabled: true, springStrength: 0.1 },
    labels: { fontSize: 14, textColor: '#FFFFFF' }
  }
};

// After migration
const newSettings = {
  visualisation: {
    graphs: {
      logseq: {
        nodes: { baseColor: '#4B5EFF', nodeSize: 8 },
        edges: { color: '#F59E0B', baseWidth: 2 },
        physics: { enabled: true, springStrength: 0.1 },
        labels: { fontSize: 14, textColor: '#FFFFFF' }
      },
      visionflow: {
        nodes: { baseColor: '#10B981', nodeSize: 10 },
        edges: { color: '#34D399', baseWidth: 3 },
        physics: { enabled: true, springStrength: 0.15 },
        labels: { fontSize: 16, textColor: '#ECFDF5' }
      }
    },
    // Global settings remain unchanged
    rendering: { ... },
    animations: { ... },
    bloom: { ... },
    hologram: { ... }
  }
};
```

### `getGraphSettings(settings: Settings, graphName: 'logseq' | 'visionflow'): GraphSettings`

Retrieves settings for a specific graph with fallback to legacy settings for backward compatibility.

**Fallback Logic:**
1. First checks for settings in the new multi-graph structure
2. Falls back to legacy flat structure if new structure doesn't exist
3. Finally falls back to default settings for the specified graph

**Usage Example:**
```typescript
// Get settings for the logseq graph
const logseqSettings = getGraphSettings(settings, 'logseq');

// Access specific settings
const nodeColor = logseqSettings.nodes.baseColor;
const physicsEnabled = logseqSettings.physics.enabled;
```

## Integration with Settings Store

The migration utility is integrated into the settings store to ensure automatic migration when settings are loaded:

```typescript
// In settingsStore.ts
const loadedSettings = await settingsService.getSettings();
const migratedSettings = migrateToMultiGraphSettings(loadedSettings);
```

## Viewport Settings Support

The migration works seamlessly with viewport settings patterns defined in `viewportSettings.ts`. This configuration determines which settings require immediate viewport updates without waiting for the debounced save.

### Viewport Settings Patterns

```typescript
// From client/src/features/settings/config/viewportSettings.ts
export const VIEWPORT_SETTINGS_PATTERNS = [
  // Legacy visualization settings
  'visualisation.nodes',
  'visualisation.edges',
  'visualisation.physics',
  'visualisation.rendering',
  'visualisation.animations',
  'visualisation.labels',
  'visualisation.bloom',
  'visualisation.hologram',
  'visualisation.camera',
  
  // NEW: Graph-specific visualization settings (post-migration)
  'visualisation.graphs.*.nodes',
  'visualisation.graphs.*.edges',
  'visualisation.graphs.*.physics',
  'visualisation.graphs.*.rendering',
  'visualisation.graphs.*.animations',
  'visualisation.graphs.*.labels',
  'visualisation.graphs.*.bloom',
  'visualisation.graphs.*.hologram',
  'visualisation.graphs.*.camera',
  
  // XR settings that affect viewport
  'xr.mode',
  'xr.quality',
  'xr.render_scale',
  // ... other XR settings
];
```

### Viewport Settings Functions

The viewport settings module provides utilities for detecting real-time settings:

```typescript
// Check if a settings path requires immediate viewport update
function isViewportSetting(path: string): boolean {
  // Returns true for patterns like:
  // - 'visualisation.graphs.logseq.nodes.baseColor'
  // - 'visualisation.graphs.visionflow.physics.enabled'
  // - 'visualisation.rendering.backgroundColor'
}

// Extract viewport-related paths from settings update
function getViewportPaths(paths: string[]): string[] {
  // Filters array to only viewport-affecting paths
}
```

### Migration Impact on Viewport Settings

The migration from flat to multi-graph structure affects viewport settings in several ways:

1. **Pattern Expansion**: New wildcard patterns support graph-specific settings
2. **Legacy Support**: Old patterns remain for backward compatibility
3. **Performance**: Same real-time update behavior for both structures
4. **Consistency**: Both legacy and new paths trigger immediate viewport updates

**Example Usage:**
```typescript
// Both of these trigger immediate viewport updates:
settingsStore.set('visualisation.nodes.baseColor', '#FF0000');           // Legacy
settingsStore.set('visualisation.graphs.logseq.nodes.baseColor', '#0000FF'); // New
```

## Best Practices

1. **Always Use Migration**: When loading settings, always run them through `migrateToMultiGraphSettings`
2. **Use Helper Functions**: Use `getGraphSettings` during the transition period for backward compatibility
3. **Update Components Gradually**: Components can be updated to use the new structure while maintaining compatibility
4. **Clean Up Legacy Code**: Once migration is complete and stable, remove legacy compatibility code

## Migration Timeline

1. **Phase 1**: Automatic migration on load (current)
2. **Phase 2**: Update all components to use new structure
3. **Phase 3**: Remove legacy field support
4. **Phase 4**: Remove migration utility (once all users have migrated)

## Testing Migration

To test the migration:

```typescript
import { migrateToMultiGraphSettings, getGraphSettings } from './settingsMigration';

// Test with legacy settings
const legacySettings = { /* ... old structure ... */ };
const migrated = migrateToMultiGraphSettings(legacySettings);

// Verify migration
console.assert(migrated.visualisation.graphs.logseq !== undefined);
console.assert(migrated.visualisation.graphs.visionflow !== undefined);
console.assert(migrated.visualisation.nodes === undefined);
```