/**
 * Debug configuration and initialization
 */

import { debugControl, replaceGlobalConsole, DebugCategory } from './console';
import { createLogger } from './logger';

const logger = createLogger('DebugConfig');

/**
 * Debug configuration from environment variables
 */
export interface DebugConfig {
  enabled: boolean;
  categories: DebugCategory[];
  replaceConsole: boolean;
  preset?: 'minimal' | 'standard' | 'verbose' | 'off';
}

/**
 * Parse debug configuration from environment
 */
export function parseDebugConfig(): DebugConfig {
  // Vite uses import.meta.env instead of process.env
  const env = import.meta.env || {};
  const isDev = env.DEV || env.MODE === 'development';
  
  const config: DebugConfig = {
    enabled: env.VITE_DEBUG === 'true' || isDev,
    categories: [],
    replaceConsole: env.VITE_DEBUG_REPLACE_CONSOLE === 'true',
    preset: env.VITE_DEBUG_PRESET as any,
  };

  // Parse categories from comma-separated list
  if (env.VITE_DEBUG_CATEGORIES) {
    const cats = env.VITE_DEBUG_CATEGORIES.split(',').map((c: string) => c.trim());
    config.categories = cats.filter(c => Object.values(DebugCategory).includes(c as any)) as DebugCategory[];
  }

  return config;
}

/**
 * Initialize debug system based on configuration
 */
export function initializeDebugSystem(): void {
  const config = parseDebugConfig();
  
  logger.info('Initializing debug system', config);

  // Apply preset if specified
  if (config.preset && debugControl.presets[config.preset]) {
    debugControl.presets[config.preset]();
    logger.info(`Applied debug preset: ${config.preset}`);
  } else if (config.enabled) {
    // Manual configuration
    debugControl.enable();
    
    // Set up categories
    if (config.categories.length > 0) {
      debugControl.disableAllCategories();
      config.categories.forEach(cat => {
        debugControl.enableCategory(cat);
      });
      logger.info('Enabled debug categories:', config.categories);
    }
  }

  // Replace global console if requested
  if (config.replaceConsole && process.env.NODE_ENV === 'development') {
    replaceGlobalConsole(true);
  }

  // Log final state
  logger.info('Debug system initialized', {
    enabled: debugControl.isEnabled(),
    categories: debugControl.getEnabledCategories(),
  });
}

/**
 * Environment variable reference (for Vite):
 * 
 * VITE_DEBUG=true                  Enable debug mode
 * VITE_DEBUG_PRESET=verbose        Use a preset (minimal, standard, verbose, off)
 * VITE_DEBUG_CATEGORIES=voice,websocket  Enable specific categories
 * VITE_DEBUG_REPLACE_CONSOLE=true  Replace global console with gated version
 */