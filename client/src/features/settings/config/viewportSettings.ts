/**
 * Configuration defining which settings require immediate viewport updates
 * These settings affect real-time rendering and need to bypass the debounced save
 */

export const VIEWPORT_SETTINGS_PATTERNS = [
  // Visualization settings that affect rendering
  'visualisation.nodes',
  'visualisation.edges',
  'visualisation.physics',
  'visualisation.rendering',
  'visualisation.animations',
  'visualisation.labels',
  'visualisation.bloom',
  'visualisation.hologram',
  'visualisation.camera',
  
  // Graph-specific visualization settings
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
  'xr.enable_hand_tracking',
  'xr.hand_mesh_enabled',
  'xr.hand_ray_enabled',
  'xr.enable_light_estimation',
  'xr.enable_plane_detection',
  'xr.enable_passthrough_portal',
  
  // Debug visualization settings
  'system.debug.enabled',
  'system.debug.showAxesHelper',
  'system.debug.showStats',
];

/**
 * Check if a settings path requires immediate viewport update
 * @param path The settings path being updated
 * @returns true if the path requires immediate viewport update
 */
export function isViewportSetting(path: string): boolean {
  return VIEWPORT_SETTINGS_PATTERNS.some(pattern => {
    // Convert pattern with wildcards to regex
    const regexPattern = pattern
      .replace(/\./g, '\\.')  // Escape dots
      .replace(/\*/g, '[^.]+'); // Replace * with non-dot characters
    
    const regex = new RegExp(`^${regexPattern}($|\\.)`);
    return regex.test(path);
  });
}

/**
 * Extract all viewport-related paths from a settings update
 * @param paths Array of settings paths being updated
 * @returns Array of paths that require viewport updates
 */
export function getViewportPaths(paths: string[]): string[] {
  return paths.filter(isViewportSetting);
}