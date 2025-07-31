/**
 * Console wrapper that respects debug settings
 * Provides drop-in replacements for console methods with automatic gating
 */

import { debugState } from './debugState';
import { createLogger } from './logger';

// Default logger for console wrapper
const logger = createLogger('Console');

// Environment check - Vite uses import.meta.env
const isDevelopment = import.meta.env?.DEV || import.meta.env?.MODE === 'development';

/**
 * Debug categories for fine-grained control
 */
export enum DebugCategory {
  GENERAL = 'general',
  VOICE = 'voice',
  WEBSOCKET = 'websocket',
  PERFORMANCE = 'performance',
  DATA = 'data',
  RENDERING = '3d',
  AUTH = 'auth',
  ERROR = 'error',
}

/**
 * Category-specific debug settings
 */
class CategoryDebugState {
  private categories: Set<DebugCategory> = new Set();
  private readonly STORAGE_KEY = 'debug.categories';

  constructor() {
    this.loadFromStorage();
  }

  private loadFromStorage(): void {
    if (typeof window !== 'undefined') {
      try {
        const stored = localStorage.getItem(this.STORAGE_KEY);
        if (stored) {
          const categories = JSON.parse(stored) as DebugCategory[];
          this.categories = new Set(categories);
        }
      } catch (e) {
        // Ignore storage errors
      }
    }
  }

  private saveToStorage(): void {
    if (typeof window !== 'undefined') {
      try {
        const categories = Array.from(this.categories);
        localStorage.setItem(this.STORAGE_KEY, JSON.stringify(categories));
      } catch (e) {
        // Ignore storage errors
      }
    }
  }

  public enableCategory(category: DebugCategory): void {
    this.categories.add(category);
    this.saveToStorage();
  }

  public disableCategory(category: DebugCategory): void {
    this.categories.delete(category);
    this.saveToStorage();
  }

  public isCategoryEnabled(category: DebugCategory): boolean {
    // Error category is always enabled in development
    if (category === DebugCategory.ERROR && isDevelopment) return true;
    
    return debugState.isEnabled() && this.categories.has(category);
  }

  public enableAll(): void {
    Object.values(DebugCategory).forEach(cat => {
      this.categories.add(cat as DebugCategory);
    });
    this.saveToStorage();
  }

  public disableAll(): void {
    this.categories.clear();
    this.saveToStorage();
  }

  public getEnabledCategories(): DebugCategory[] {
    return Array.from(this.categories);
  }
}

// Singleton instance for category management
export const categoryDebugState = new CategoryDebugState();

/**
 * Options for console wrapper methods
 */
export interface ConsoleOptions {
  category?: DebugCategory;
  namespace?: string;
  force?: boolean; // Force output regardless of debug settings
}

/**
 * Creates a gated console method
 */
function createGatedMethod(
  method: 'log' | 'warn' | 'error' | 'info' | 'debug',
  defaultCategory: DebugCategory = DebugCategory.GENERAL
) {
  return function(messageOrOptions: any, ...args: any[]) {
    let message = messageOrOptions;
    let options: ConsoleOptions = { category: defaultCategory };

    // Check if first argument is options
    if (
      typeof messageOrOptions === 'object' &&
      messageOrOptions !== null &&
      'category' in messageOrOptions
    ) {
      options = messageOrOptions;
      message = args[0];
      args = args.slice(1);
    }

    // Force errors in development
    if (method === 'error' && isDevelopment) {
      options.force = true;
    }

    // Check if should output
    const shouldOutput = options.force || 
      (debugState.isEnabled() && 
       (!options.category || categoryDebugState.isCategoryEnabled(options.category)));

    if (!shouldOutput) return;

    // Use custom logger if namespace provided
    if (options.namespace) {
      const nsLogger = createLogger(options.namespace);
      nsLogger[method === 'log' ? 'info' : method](message, ...args);
    } else {
      // Fallback to native console
      console[method](message, ...args);
    }
  };
}

/**
 * Gated console replacement
 * Drop-in replacement for console with automatic debug gating
 */
export const gatedConsole = {
  log: createGatedMethod('log'),
  error: createGatedMethod('error', DebugCategory.ERROR),
  warn: createGatedMethod('warn', DebugCategory.ERROR),
  info: createGatedMethod('info'),
  debug: createGatedMethod('debug'),
  
  // Convenience methods for categories
  voice: {
    log: (...args: any[]) => gatedConsole.log({ category: DebugCategory.VOICE }, ...args),
    error: (...args: any[]) => gatedConsole.error({ category: DebugCategory.VOICE }, ...args),
    warn: (...args: any[]) => gatedConsole.warn({ category: DebugCategory.VOICE }, ...args),
  },
  
  websocket: {
    log: (...args: any[]) => gatedConsole.log({ category: DebugCategory.WEBSOCKET }, ...args),
    error: (...args: any[]) => gatedConsole.error({ category: DebugCategory.WEBSOCKET }, ...args),
    warn: (...args: any[]) => gatedConsole.warn({ category: DebugCategory.WEBSOCKET }, ...args),
  },
  
  perf: {
    log: (...args: any[]) => gatedConsole.log({ category: DebugCategory.PERFORMANCE }, ...args),
    warn: (...args: any[]) => gatedConsole.warn({ category: DebugCategory.PERFORMANCE }, ...args),
  },
  
  data: {
    log: (...args: any[]) => gatedConsole.log({ category: DebugCategory.DATA }, ...args),
    warn: (...args: any[]) => gatedConsole.warn({ category: DebugCategory.DATA }, ...args),
  },
};

/**
 * Utility to replace global console in development
 * This allows existing console calls to be automatically gated
 */
export function replaceGlobalConsole(enable: boolean = true): void {
  if (!isDevelopment) {
    logger.warn('replaceGlobalConsole should only be used in development');
    return;
  }

  if (enable) {
    // Store original methods
    (window as any).__originalConsole = {
      log: console.log,
      error: console.error,
      warn: console.warn,
      info: console.info,
      debug: console.debug,
    };

    // Replace with gated versions
    console.log = gatedConsole.log;
    console.error = gatedConsole.error;
    console.warn = gatedConsole.warn;
    console.info = gatedConsole.info;
    console.debug = gatedConsole.debug;
    
    logger.info('Global console replaced with gated version');
  } else {
    // Restore original methods
    const original = (window as any).__originalConsole;
    if (original) {
      console.log = original.log;
      console.error = original.error;
      console.warn = original.warn;
      console.info = original.info;
      console.debug = original.debug;
      
      delete (window as any).__originalConsole;
      logger.info('Global console restored');
    }
  }
}

/**
 * Debug control panel API
 * Exposed for UI controls or development console
 */
export const debugControl = {
  // Main toggle
  enable: () => debugState.enableDebug(true),
  disable: () => debugState.enableDebug(false),
  isEnabled: () => debugState.isEnabled(),
  
  // Category control
  enableCategory: (cat: DebugCategory) => categoryDebugState.enableCategory(cat),
  disableCategory: (cat: DebugCategory) => categoryDebugState.disableCategory(cat),
  enableAllCategories: () => categoryDebugState.enableAll(),
  disableAllCategories: () => categoryDebugState.disableAll(),
  getEnabledCategories: () => categoryDebugState.getEnabledCategories(),
  
  // Specific debug modes
  enableData: () => debugState.enableDataDebug(true),
  disableData: () => debugState.enableDataDebug(false),
  enablePerformance: () => debugState.enablePerformanceDebug(true),
  disablePerformance: () => debugState.enablePerformanceDebug(false),
  
  // Global console replacement
  replaceConsole: () => replaceGlobalConsole(true),
  restoreConsole: () => replaceGlobalConsole(false),
  
  // Debug presets
  presets: {
    minimal: () => {
      debugState.enableDebug(true);
      categoryDebugState.disableAll();
      categoryDebugState.enableCategory(DebugCategory.ERROR);
    },
    standard: () => {
      debugState.enableDebug(true);
      categoryDebugState.disableAll();
      categoryDebugState.enableCategory(DebugCategory.ERROR);
      categoryDebugState.enableCategory(DebugCategory.GENERAL);
    },
    verbose: () => {
      debugState.enableDebug(true);
      categoryDebugState.enableAll();
      debugState.enableDataDebug(true);
      debugState.enablePerformanceDebug(true);
    },
    off: () => {
      debugState.enableDebug(false);
    }
  }
};

// Export to window in development for easy console access
if (isDevelopment && typeof window !== 'undefined') {
  (window as any).debugControl = debugControl;
  logger.info('Debug control available at window.debugControl');
}