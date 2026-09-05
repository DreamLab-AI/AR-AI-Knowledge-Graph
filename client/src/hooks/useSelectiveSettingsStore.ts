import { useCallback, useEffect, useMemo, useRef } from 'react';
import { useSettingsStore } from '@/store/settingsStore';
import { SettingsPath } from '@/features/settings/config/settings';

/**
 * Read a single setting by path from the Zustand store.
 * Zustand's built-in selector memoization prevents unnecessary re-renders.
 *
 * Previous implementation layered manual caching, TTL, request deduplication,
 * debounced batch loading, and API fetching on top of the store. All of that
 * is redundant: the settings store already holds the canonical state and
 * Zustand only re-renders when the selected slice changes.
 */
export function useSelectiveSetting<T>(
  path: SettingsPath,
  _options?: {
    enableCache?: boolean;
    enableDeduplication?: boolean;
    fallbackToStore?: boolean;
  }
): T {
  return useSettingsStore(useCallback(
    (state) => state.get(path) as T,
    [path]
  ));
}

/**
 * Read multiple settings by path from the Zustand store.
 * Returns an object keyed by the same keys as the input `paths` map.
 */
export function useSelectiveSettings<T extends Record<string, any>>(
  paths: Record<keyof T, SettingsPath>,
  _options?: {
    enableBatchLoading?: boolean;
    enableCache?: boolean;
    fallbackToStore?: boolean;
  }
): T {
  const pathEntries = useMemo(
    () => Object.entries(paths) as [keyof T, SettingsPath][],
    // Deps intentionally narrowed; react-hooks/exhaustive-deps is not enforced in this config.
    [JSON.stringify(paths)]
  );

  return useSettingsStore(useCallback(
    (state) => {
      const result = {} as T;
      for (const [key, p] of pathEntries) {
        result[key] = state.get(p) as T[keyof T];
      }
      return result;
    },
    [pathEntries]
  ));
}

/**
 * Thin wrapper exposing store mutators.
 * `set` and `immediateSet` both write synchronously -- the previous debounce
 * layer added latency without benefit since the store already batches React
 * state updates through Zustand.
 */
export function useSettingSetter() {
  const updateSettings = useSettingsStore(state => state.updateSettings);
  const setByPath = useSettingsStore(state => state.setByPath);
  const batchUpdate = useSettingsStore(state => state.batchUpdate);

  const set = useCallback(
    (path: SettingsPath, value: any) => setByPath(path, value),
    [setByPath]
  );

  const batchedSet = useCallback(
    (updates: Record<SettingsPath, any>) => {
      const updateArray = Object.entries(updates).map(([path, value]) => ({
        path,
        value,
      }));
      batchUpdate(updateArray);
    },
    [batchUpdate]
  );

  const immediateSet = useCallback(
    (updater: (draft: any) => void) => updateSettings(updater),
    [updateSettings]
  );

  return useMemo(() => ({
    set,
    batchedSet,
    immediateSet,
    updateSettings,
  }), [set, batchedSet, immediateSet, updateSettings]);
}

/**
 * Subscribe to a setting path change and invoke a callback.
 * Uses the store's built-in subscribe mechanism.
 */
export function useSettingsSubscription(
  path: SettingsPath,
  callback: (value: any) => void,
  options: {
    immediate?: boolean;
    enableCache?: boolean;
    dependencies?: React.DependencyList;
  } = {}
) {
  const {
    immediate = true,
    dependencies = [],
  } = options;

  const depsKey = JSON.stringify(dependencies);

  const cbRef = useRef(callback);
  useEffect(() => { cbRef.current = callback; }, [callback]);

  useEffect(() => {
    let mounted = true;

    const handleChange = () => {
      const value = useSettingsStore.getState().get(path);
      if (mounted) cbRef.current(value);
    };

    if (immediate) handleChange();

    const unsubscribe = useSettingsStore.getState().subscribe(path, handleChange, false);
    return () => { mounted = false; unsubscribe(); };
    // Deps intentionally narrowed; react-hooks/exhaustive-deps is not enforced in this config.
  }, [path, immediate, depsKey]);
}

/**
 * Select a derived value from the settings object with shallow equality.
 */
export function useSettingsSelector<T>(
  selector: (settings: any) => T,
  _options?: {
    equalityFn?: (prev: T, next: T) => boolean;
    enableCache?: boolean;
    cacheTTL?: number;
  }
): T {
  return useSettingsStore(
    useCallback((state) => selector(state.settings), [selector])
  ) as T;
}
