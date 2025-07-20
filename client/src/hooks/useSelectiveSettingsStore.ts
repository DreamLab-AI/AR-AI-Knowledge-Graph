import { useCallback, useEffect, useRef } from 'react';
import { useSettingsStore } from '@/store/settingsStore';
import { SettingsPath } from '@/features/settings/config/settings';
import { createLogger } from '@/utils/logger';

const logger = createLogger('useSelectiveSettingsStore');

/**
 * Custom hook for selective subscriptions to settings store
 * This hook optimizes re-renders by only subscribing to specific paths
 */
export function useSelectiveSetting<T>(path: SettingsPath): T {
  const value = useSettingsStore(state => state.get<T>(path));
  const unsubscribeRef = useRef<(() => void) | null>(null);
  
  useEffect(() => {
    // Subscribe only to the specific path
    unsubscribeRef.current = useSettingsStore.getState().subscribe(
      path,
      () => {
        // Force re-render when this specific setting changes
        // This is handled by zustand's subscribe mechanism
      },
      true
    );
    
    return () => {
      if (unsubscribeRef.current) {
        unsubscribeRef.current();
      }
    };
  }, [path]);
  
  return value;
}

/**
 * Hook for subscribing to multiple settings paths
 * Returns an object with the current values
 */
export function useSelectiveSettings<T extends Record<string, any>>(
  paths: Record<keyof T, SettingsPath>
): T {
  const values = {} as T;
  
  // Get initial values
  for (const key in paths) {
    values[key] = useSettingsStore.getState().get(paths[key]);
  }
  
  // Subscribe to changes
  useEffect(() => {
    const unsubscribes: (() => void)[] = [];
    
    for (const key in paths) {
      const unsubscribe = useSettingsStore.getState().subscribe(
        paths[key],
        () => {
          // Force re-render when any subscribed setting changes
        },
        true
      );
      unsubscribes.push(unsubscribe);
    }
    
    return () => {
      unsubscribes.forEach(unsub => unsub());
    };
  }, [paths]);
  
  // Return current values
  for (const key in paths) {
    values[key] = useSettingsStore(state => state.get(paths[key]));
  }
  
  return values;
}

/**
 * Hook for setting values with optimized updates
 * Returns a setter function that batches updates
 */
export function useSettingSetter() {
  const set = useSettingsStore(state => state.set); // Deprecated - kept for backward compatibility
  const updateSettings = useSettingsStore(state => state.updateSettings);
  
  const batchedSet = useCallback((updates: Record<SettingsPath, any>) => {
    updateSettings((draft) => {
      for (const [path, value] of Object.entries(updates)) {
        const pathParts = path.split('.');
        let current = draft as any;
        
        for (let i = 0; i < pathParts.length - 1; i++) {
          const part = pathParts[i];
          if (!current[part]) {
            current[part] = {};
          }
          current = current[part];
        }
        
        current[pathParts[pathParts.length - 1]] = value;
      }
    });
  }, [updateSettings]);
  
  return {
    set, // Deprecated - use updateSettings or batchedSet instead
    batchedSet,
    updateSettings // Expose updateSettings directly for single updates
  };
}

/**
 * Hook for subscribing to settings changes with a callback
 * Useful for side effects when settings change
 */
export function useSettingsSubscription(
  path: SettingsPath,
  callback: (value: any) => void,
  dependencies: React.DependencyList = []
) {
  const callbackRef = useRef(callback);
  
  // Update callback ref when it changes
  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);
  
  useEffect(() => {
    const handleChange = () => {
      const value = useSettingsStore.getState().get(path);
      callbackRef.current(value);
    };
    
    // Call immediately with current value
    handleChange();
    
    // Subscribe to changes
    const unsubscribe = useSettingsStore.getState().subscribe(
      path,
      handleChange,
      false
    );
    
    return unsubscribe;
  }, [path, ...dependencies]);
}

/**
 * Hook for getting settings with a selector function
 * This allows for derived state from settings
 */
export function useSettingsSelector<T>(
  selector: (settings: any) => T,
  equalityFn?: (prev: T, next: T) => boolean
): T {
  return useSettingsStore(state => selector(state.settings), equalityFn);
}