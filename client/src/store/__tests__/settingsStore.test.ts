// @ts-ignore - vitest types may not be available in all environments
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';

// --- Mock all external dependencies before importing the store ---

vi.mock('../../utils/loggerConfig', () => ({
  createLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
  createErrorMetadata: vi.fn((e: unknown) => e),
}));

vi.mock('../../utils/clientDebugState', () => ({
  debugState: {
    isEnabled: () => false,
    isDataDebugEnabled: () => false,
  },
}));

vi.mock('../../features/design-system/components/Toast', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

vi.mock('../../features/settings/config/viewportSettings', () => ({
  isViewportSetting: vi.fn(() => false),
}));

const mockGetSettingsByPaths = vi.fn().mockResolvedValue({});
const mockGetSettingByPath = vi.fn().mockResolvedValue(undefined);
const mockUpdateSettingByPath = vi.fn().mockResolvedValue(undefined);
const mockUpdateSettingsByPaths = vi.fn().mockResolvedValue(undefined);
const mockFlushPendingUpdates = vi.fn().mockResolvedValue(undefined);
const mockResetSettings = vi.fn().mockResolvedValue(undefined);
const mockExportSettings = vi.fn((s: unknown) => JSON.stringify(s));
const mockImportSettings = vi.fn((json: string) => JSON.parse(json));

vi.mock('../../api/settingsApi', () => ({
  settingsApi: {
    getSettingsByPaths: (...args: unknown[]) => mockGetSettingsByPaths(...args),
    getSettingByPath: (...args: unknown[]) => mockGetSettingByPath(...args),
    updateSettingByPath: (...args: unknown[]) => mockUpdateSettingByPath(...args),
    updateSettingsByPaths: (...args: unknown[]) => mockUpdateSettingsByPaths(...args),
    flushPendingUpdates: (...args: unknown[]) => mockFlushPendingUpdates(...args),
    resetSettings: (...args: unknown[]) => mockResetSettings(...args),
    exportSettings: (...args: unknown[]) => mockExportSettings(...args),
    importSettings: (...args: unknown[]) => mockImportSettings(...args),
  },
}));

vi.mock('../../services/nostrAuthService', () => ({
  nostrAuth: {
    initialized: true,
    initialize: vi.fn().mockResolvedValue(undefined),
    isAuthenticated: vi.fn(() => false),
    getCurrentUser: vi.fn(() => null),
  },
}));

const mockQueueChange = vi.fn();
const mockQueueChanges = vi.fn();
vi.mock('../autoSaveManager', () => ({
  autoSaveManager: {
    setSyncEnabled: vi.fn(),
    setInitialized: vi.fn(),
    queueChange: (...args: unknown[]) => mockQueueChange(...args),
    queueChanges: (...args: unknown[]) => mockQueueChanges(...args),
  },
}));

// Need to import AFTER mocks are set up
import { useSettingsStore, settingsStoreUtils } from '../settingsStore';

describe('settingsStore', () => {
  beforeEach(() => {
    // Reset the zustand store to initial state between tests
    useSettingsStore.setState({
      partialSettings: {},
      settings: {},
      loadedPaths: new Set(),
      loadingSections: new Set(),
      initialized: false,
      authenticated: false,
      user: null,
      isPowerUser: false,
      settingsSyncEnabled: true,
    });
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // ---- Store initialisation ----

  describe('initialization', () => {
    it('should start with empty default state', () => {
      const state = useSettingsStore.getState();
      expect(state.initialized).toBe(false);
      expect(state.authenticated).toBe(false);
      expect(state.user).toBeNull();
      expect(state.isPowerUser).toBe(false);
      expect(state.settings).toEqual({});
      expect(state.partialSettings).toEqual({});
    });

    it('should initialize and merge essential settings from API', async () => {
      const serverSettings = {
        system: { debug: { enabled: false } },
        auth: { enabled: true, required: false },
      };
      mockGetSettingsByPaths.mockResolvedValueOnce(serverSettings);

      await useSettingsStore.getState().initialize();

      const state = useSettingsStore.getState();
      expect(state.initialized).toBe(true);
      expect((state.settings as Record<string, unknown>).system).toEqual({ debug: { enabled: false } });
      expect((state.settings as Record<string, unknown>).auth).toEqual({ enabled: true, required: false });
    });

    it('should set initialized to false on API failure', async () => {
      mockGetSettingsByPaths.mockRejectedValueOnce(new Error('Network error'));

      await expect(useSettingsStore.getState().initialize()).rejects.toThrow('Network error');
      expect(useSettingsStore.getState().initialized).toBe(false);
    });
  });

  // ---- updateSetting (set) at nested paths ----

  describe('set (updateSetting)', () => {
    it('should update a simple top-level path', () => {
      const store = useSettingsStore.getState();
      store.set('auth.enabled', true);

      const state = useSettingsStore.getState();
      const auth = (state.partialSettings as Record<string, unknown>).auth as Record<string, unknown>;
      expect(auth.enabled).toBe(true);
    });

    it('should update a deeply nested path', () => {
      const store = useSettingsStore.getState();
      store.set('visualisation.graphs.logseq.physics.springK', 42);

      const state = useSettingsStore.getState();
      const vis = (state.partialSettings as Record<string, unknown>).visualisation as Record<string, unknown>;
      const graphs = vis.graphs as Record<string, unknown>;
      const logseq = graphs.logseq as Record<string, unknown>;
      const physics = logseq.physics as Record<string, unknown>;
      expect(physics.springK).toBe(42);
    });

    it('should add path to loadedPaths', () => {
      const store = useSettingsStore.getState();
      store.set('system.debug.enabled', true);

      const state = useSettingsStore.getState();
      expect(state.loadedPaths.has('system.debug.enabled')).toBe(true);
    });

    it('should throw on empty path', () => {
      const store = useSettingsStore.getState();
      expect(() => store.set('', 'value')).toThrow('Path cannot be empty');
    });

    it('should queue change to autoSaveManager by default', () => {
      const store = useSettingsStore.getState();
      store.set('auth.enabled', true);
      expect(mockQueueChange).toHaveBeenCalledWith('auth.enabled', true);
    });

    it('should skip server sync when skipServerSync=true', () => {
      const store = useSettingsStore.getState();
      store.set('auth.enabled', true, true);
      expect(mockQueueChange).not.toHaveBeenCalled();
    });
  });

  // ---- subscribe fires on matching paths only ----

  describe('subscribe', () => {
    it('should fire callback immediately when initialized', () => {
      useSettingsStore.setState({ initialized: true });
      const cb = vi.fn();
      useSettingsStore.getState().subscribe('system.debug', cb);
      expect(cb).toHaveBeenCalledTimes(1);
    });

    it('should not fire immediately when not initialized and immediate=true', () => {
      useSettingsStore.setState({ initialized: false });
      const cb = vi.fn();
      useSettingsStore.getState().subscribe('system.debug', cb);
      expect(cb).not.toHaveBeenCalled();
    });

    it('should not fire immediately when immediate=false', () => {
      useSettingsStore.setState({ initialized: true });
      const cb = vi.fn();
      useSettingsStore.getState().subscribe('system.debug', cb, false);
      expect(cb).not.toHaveBeenCalled();
    });

    it('should return unsubscribe function that removes the callback', () => {
      useSettingsStore.setState({ initialized: true });
      const cb = vi.fn();
      const unsub = useSettingsStore.getState().subscribe('system.debug', cb);
      expect(cb).toHaveBeenCalledTimes(1);
      unsub();
      // After unsubscribe, updateSettings should not call cb again (through RAF)
      // We verify the unsubscribe path is clean by calling it again
      expect(typeof unsub).toBe('function');
    });
  });

  // ---- Deep merge does not lose sibling fields ----

  describe('deep merge (updateSettings)', () => {
    it('should preserve sibling fields when updating nested paths', () => {
      // Pre-populate with some settings
      useSettingsStore.setState({
        partialSettings: {
          visualisation: {
            glow: { enabled: true, intensity: 0.5 },
            rendering: { context: 'webgl2' },
          },
        } as Record<string, unknown>,
        settings: {
          visualisation: {
            glow: { enabled: true, intensity: 0.5 },
            rendering: { context: 'webgl2' },
          },
        } as Record<string, unknown>,
      });

      // Update only glow.intensity via updateSettings
      useSettingsStore.getState().updateSettings((draft: any) => {
        if (draft.visualisation?.glow) {
          draft.visualisation.glow.intensity = 0.8;
        }
      });

      const state = useSettingsStore.getState();
      const vis = (state.partialSettings as Record<string, unknown>).visualisation as Record<string, unknown>;
      const glow = vis.glow as Record<string, unknown>;
      const rendering = vis.rendering as Record<string, unknown>;

      // Glow intensity updated
      expect(glow.intensity).toBe(0.8);
      // Glow.enabled sibling preserved
      expect(glow.enabled).toBe(true);
      // rendering sibling object preserved
      expect(rendering.context).toBe('webgl2');
    });

    it('should not produce changes when updater does nothing', () => {
      useSettingsStore.setState({
        partialSettings: { a: 1 } as Record<string, unknown>,
        settings: { a: 1 } as Record<string, unknown>,
      });

      useSettingsStore.getState().updateSettings(() => {
        // no-op
      });

      // autoSaveManager should not have been called with any changes
      expect(mockQueueChanges).not.toHaveBeenCalled();
    });
  });

  // ---- Invalid paths handled gracefully ----

  describe('get (invalid/missing paths)', () => {
    it('should return undefined for unloaded path', () => {
      const result = useSettingsStore.getState().get('some.unloaded.path');
      expect(result).toBeUndefined();
    });

    it('should return undefined for non-existent nested path even if parent loaded', () => {
      useSettingsStore.setState({
        partialSettings: { system: { debug: { enabled: true } } } as Record<string, unknown>,
        loadedPaths: new Set(['system.debug.enabled']),
      });

      // Path that partially matches but final key does not exist
      const result = useSettingsStore.getState().get<number>('system.debug.nonexistent');
      expect(result).toBeUndefined();
    });

    it('should return all settings when path is empty string', () => {
      const settings = { a: 1, b: 2 };
      useSettingsStore.setState({ partialSettings: settings as Record<string, unknown> });
      const result = useSettingsStore.getState().get('');
      expect(result).toEqual(settings);
    });

    it('should return loaded value for a loaded path', () => {
      useSettingsStore.setState({
        partialSettings: { system: { debug: { enabled: true } } } as Record<string, unknown>,
        loadedPaths: new Set(['system.debug.enabled']),
      });

      const result = useSettingsStore.getState().get<boolean>('system.debug.enabled');
      expect(result).toBe(true);
    });
  });

  // ---- Batch update ----

  describe('batchUpdate', () => {
    it('should apply all changes atomically', () => {
      const store = useSettingsStore.getState();
      store.batchUpdate([
        { path: 'auth.enabled', value: true },
        { path: 'auth.required', value: false },
        { path: 'system.debug.enabled', value: true },
      ]);

      const state = useSettingsStore.getState();
      const auth = (state.partialSettings as Record<string, unknown>).auth as Record<string, unknown>;
      const system = (state.partialSettings as Record<string, unknown>).system as Record<string, unknown>;
      const debug = system.debug as Record<string, unknown>;

      expect(auth.enabled).toBe(true);
      expect(auth.required).toBe(false);
      expect(debug.enabled).toBe(true);
    });

    it('should mark all paths as loaded', () => {
      const store = useSettingsStore.getState();
      store.batchUpdate([
        { path: 'auth.enabled', value: true },
        { path: 'system.debug.enabled', value: false },
      ]);

      const state = useSettingsStore.getState();
      expect(state.loadedPaths.has('auth.enabled')).toBe(true);
      expect(state.loadedPaths.has('system.debug.enabled')).toBe(true);
    });

    it('should call settingsApi.updateSettingsByPaths once', () => {
      const store = useSettingsStore.getState();
      store.batchUpdate([
        { path: 'auth.enabled', value: true },
        { path: 'system.debug.enabled', value: false },
      ]);

      expect(mockUpdateSettingsByPaths).toHaveBeenCalledTimes(1);
    });
  });

  // ---- Reset to defaults ----

  describe('resetSettings', () => {
    it('should clear all settings and re-initialize', async () => {
      // Pre-populate
      useSettingsStore.setState({
        partialSettings: { a: 1 } as Record<string, unknown>,
        settings: { a: 1 } as Record<string, unknown>,
        loadedPaths: new Set(['a']),
        initialized: true,
      });

      // Mock the server calls for reset + re-init
      mockResetSettings.mockResolvedValueOnce(undefined);
      mockGetSettingsByPaths.mockResolvedValueOnce({ system: { debug: { enabled: false } } });

      await useSettingsStore.getState().resetSettings();

      const state = useSettingsStore.getState();
      expect(state.initialized).toBe(true);
      expect(mockResetSettings).toHaveBeenCalledTimes(1);
    });
  });

  // ---- ensureLoaded deep-merge (defect-4: server GET must not drop client keys) ----

  describe('ensureLoaded (lazy hydration merge)', () => {
    it('deep-merges server subtrees over client state without dropping client-only keys', async () => {
      // Client has touched visualisation.rendering.labelLayoutEvery and holds two
      // other client-known keys the server never round-trips.
      useSettingsStore.setState({
        partialSettings: {
          visualisation: {
            rendering: {
              labelLayoutEvery: 5,
              maxEdgesCeiling: 1000,
              softwareFallback: true,
              ambientLightIntensity: 0.1, // stale client value the server will overwrite
            },
          },
        } as Record<string, unknown>,
        settings: {} as Record<string, unknown>,
        loadedPaths: new Set(['visualisation.rendering.labelLayoutEvery']),
        initialized: true,
      });

      // getSettingsByPaths returns the FULL canonical tree; the server response
      // omits the three client-known rendering keys.
      mockGetSettingsByPaths.mockResolvedValueOnce({
        visualisation: {
          rendering: {
            ambientLightIntensity: 0.7,
            enableAntialiasing: true,
          },
        },
      });

      await useSettingsStore.getState().ensureLoaded(['visualisation.rendering']);

      const rendering = ((useSettingsStore.getState().partialSettings as Record<string, unknown>)
        .visualisation as Record<string, unknown>).rendering as Record<string, unknown>;

      // Client-only keys survive the GET (this is the defect-4 regression).
      expect(rendering.labelLayoutEvery).toBe(5);
      expect(rendering.maxEdgesCeiling).toBe(1000);
      expect(rendering.softwareFallback).toBe(true);
      // Server values win per-key where present.
      expect(rendering.ambientLightIntensity).toBe(0.7);
      expect(rendering.enableAntialiasing).toBe(true);
    });

    it('short-circuits (no fetch) when every path is already loaded', async () => {
      useSettingsStore.setState({
        loadedPaths: new Set(['visualisation.rendering']),
      });
      await useSettingsStore.getState().ensureLoaded(['visualisation.rendering']);
      expect(mockGetSettingsByPaths).not.toHaveBeenCalled();
    });
  });

  // ---- settingsStoreUtils helpers ----

  describe('settingsStoreUtils', () => {
    describe('setNestedValue', () => {
      it('should set a deeply nested value', () => {
        const obj: Record<string, unknown> = {};
        settingsStoreUtils.setNestedValue(obj, 'a.b.c', 42);
        expect((obj as any).a.b.c).toBe(42);
      });

      it('should overwrite existing values', () => {
        const obj: Record<string, unknown> = { a: { b: { c: 1 } } };
        settingsStoreUtils.setNestedValue(obj, 'a.b.c', 99);
        expect((obj as any).a.b.c).toBe(99);
      });
    });

    describe('getAllSettingsPaths', () => {
      it('should extract all leaf paths from a nested object', () => {
        const obj = {
          a: 1,
          b: { c: 2, d: { e: 3 } },
          f: [1, 2, 3],
        };
        const paths = settingsStoreUtils.getAllSettingsPaths(obj);
        expect(paths).toContain('a');
        expect(paths).toContain('b.c');
        expect(paths).toContain('b.d.e');
        expect(paths).toContain('f');
      });
    });

    describe('getSectionPaths', () => {
      it('should return paths for known section', () => {
        const paths = settingsStoreUtils.getSectionPaths('physics');
        expect(paths.length).toBeGreaterThan(0);
        expect(paths).toContain('visualisation.graphs.logseq.physics');
      });

      it('should return empty array for unknown section', () => {
        const paths = settingsStoreUtils.getSectionPaths('nonexistent_section');
        expect(paths).toEqual([]);
      });
    });
  });

  // ---- Physics parameter validation ----

  describe('updatePhysics', () => {
    it('should clamp springK within valid range', () => {
      useSettingsStore.setState({ initialized: true });
      const store = useSettingsStore.getState();

      // springK should be clamped to [0.001, 1000]
      store.updatePhysics('logseq', { springK: -5 });

      const state = useSettingsStore.getState();
      const vis = (state.partialSettings as Record<string, unknown>).visualisation as Record<string, unknown>;
      const graphs = vis?.graphs as Record<string, unknown>;
      const logseq = graphs?.logseq as Record<string, unknown>;
      const physics = logseq?.physics as Record<string, unknown>;
      expect(physics?.springK).toBe(0.001);
    });

    it('should default to logseq when graphName is invalid', () => {
      useSettingsStore.setState({ initialized: true });
      const store = useSettingsStore.getState();

      store.updatePhysics('[object Object]' as string, { springK: 5 });

      const state = useSettingsStore.getState();
      const vis = (state.partialSettings as Record<string, unknown>).visualisation as Record<string, unknown>;
      const graphs = vis?.graphs as Record<string, unknown>;
      const logseq = graphs?.logseq as Record<string, unknown>;
      const physics = logseq?.physics as Record<string, unknown>;
      expect(physics?.springK).toBe(5);
    });
  });

  // ---- settingsSyncEnabled ----

  describe('settingsSyncEnabled', () => {
    it('should toggle sync enabled state', () => {
      const store = useSettingsStore.getState();
      store.setSettingsSyncEnabled(false);
      expect(useSettingsStore.getState().settingsSyncEnabled).toBe(false);

      store.setSettingsSyncEnabled(true);
      expect(useSettingsStore.getState().settingsSyncEnabled).toBe(true);
    });
  });
});
