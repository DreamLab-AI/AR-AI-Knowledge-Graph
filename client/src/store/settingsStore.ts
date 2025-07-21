import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'
import { defaultSettings } from '../features/settings/config/defaultSettings'
import { Settings, SettingsPath } from '../features/settings/config/settings'
import { createLogger, createErrorMetadata } from '../utils/logger'
import { debugState } from '../utils/debugState'
import { deepMerge } from '../utils/deepMerge';
import { settingsService } from '../services/settingsService';
import { produce } from 'immer';
import { toast } from '../features/design-system/components/Toast';
import { migrateToMultiGraphSettings } from '../features/settings/utils/settingsMigration';

const logger = createLogger('SettingsStore')

// Debounce utility
let saveTimeoutId: ReturnType<typeof setTimeout> | null = null;

// Shared debounced save function
const debouncedSaveToServer = async (settings: Settings, initialized: boolean) => {
  if (!initialized || settings.system?.persistSettings === false) {
    return;
  }

  try {
    const headers: Record<string, string> = {};

    // Add authentication headers if available
    try {
      const { nostrAuth } = await import('../services/nostrAuthService');
      if (nostrAuth.isAuthenticated()) {
        const user = nostrAuth.getCurrentUser();
        const token = nostrAuth.getSessionToken();
        if (user && token) {
          headers['X-Nostr-Pubkey'] = user.pubkey;
          headers['Authorization'] = `Bearer ${token}`;
          if (debugState.isEnabled()) {
            logger.info('Using Nostr authentication for settings sync');
          }
        }
      }
    } catch (error) {
      logger.warn('Error getting Nostr authentication:', createErrorMetadata(error));
    }

    const updatedSettings = await settingsService.saveSettings(settings, headers);
    if (updatedSettings) {
      if (debugState.isEnabled()) {
        logger.info('Settings saved to server successfully');
      }
      toast({ title: "Settings Saved", description: "Your settings have been synced with the server." });
    }
  } catch (error) {
    const errorMeta = createErrorMetadata(error);
    logger.error('Failed to save settings to server:', errorMeta);
    toast({
      variant: "destructive",
      title: "Save Failed",
      description: `Could not save settings to server. ${errorMeta.message || 'Check console.'}`
    });
  }
};

// Schedule debounced save
const scheduleSave = (settings: Settings, initialized: boolean) => {
  if (saveTimeoutId) {
    clearTimeout(saveTimeoutId);
  }
  saveTimeoutId = setTimeout(() => debouncedSaveToServer(settings, initialized), 300);
};

interface SettingsState {
  settings: Settings
  initialized: boolean
  authenticated: boolean
  user: { isPowerUser: boolean; pubkey: string } | null
  isPowerUser: boolean // Direct access to power user state
  subscribers: Map<string, Set<() => void>>

  // Actions
  initialize: () => Promise<Settings>
  setAuthenticated: (authenticated: boolean) => void
  setUser: (user: { isPowerUser: boolean; pubkey: string } | null) => void
  get: <T>(path: SettingsPath) => T
  set: <T>(path: SettingsPath, value: T) => void // Deprecated - use updateSettings instead
  subscribe: (path: SettingsPath, callback: () => void, immediate?: boolean) => () => void;
  unsubscribe: (path: SettingsPath, callback: () => void) => void;
  updateSettings: (updater: (draft: Settings) => void) => void;
  notifyViewportUpdate: (path: SettingsPath) => void; // For real-time viewport updates
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set, get) => ({
      settings: defaultSettings,
      initialized: false,
      authenticated: false,
      user: null,
      isPowerUser: false,
      subscribers: new Map(),

      initialize: async () => {
        try {
          if (debugState.isEnabled()) {
            logger.info('Initializing settings')
          }

          // Load settings from localStorage via zustand persist
          const currentSettings = get().settings

          // Fetch settings from server if available
          try {
            // Use the settings service to fetch settings
            const serverSettings = await settingsService.fetchSettings()

            if (serverSettings) {
              if (debugState.isEnabled()) {
                logger.info('Fetched settings from server:', { serverSettings })
              }

              // Merge server settings with defaults and current settings using deep merge
              // This ensures all nested objects are properly merged
              const mergedSettings = deepMerge(defaultSettings, currentSettings, serverSettings)

              // Apply migration to multi-graph structure
              const migratedSettings = migrateToMultiGraphSettings(mergedSettings)

              if (debugState.isEnabled()) {
                logger.info('Deep merged and migrated settings:', { migratedSettings })
              }

              set({
                settings: migratedSettings,
                initialized: true
              })

              if (debugState.isEnabled()) {
                logger.info('Settings loaded from server and merged')
              }

              return mergedSettings
            }
          } catch (error) {
            logger.warn('Failed to fetch settings from server:', createErrorMetadata(error))
            // Continue with local settings if server fetch fails
          }

          // Apply migration to current settings
          const migratedSettings = migrateToMultiGraphSettings(currentSettings)

          // Mark as initialized
          set({
            settings: migratedSettings,
            initialized: true
          })

          if (debugState.isEnabled()) {
            logger.info('Settings initialized from local storage and migrated')
          }

          return migratedSettings
        } catch (error) {
          logger.error('Failed to initialize settings:', createErrorMetadata(error))

          // Fall back to default settings (already in the correct format)
          const migratedDefaults = migrateToMultiGraphSettings(defaultSettings)
          set({
            settings: migratedDefaults,
            initialized: true
          })

          return migratedDefaults
        }
      },

      setAuthenticated: (authenticated: boolean) => set({ authenticated }),

      setUser: (user: { isPowerUser: boolean; pubkey: string } | null) => set({
        user,
        isPowerUser: user?.isPowerUser || false
      }),

      notifyViewportUpdate: (path: SettingsPath) => {
        // This method will be called for settings that need immediate viewport updates
        const callbacks = get().subscribers.get('viewport.update')
        if (callbacks) {
          Array.from(callbacks).forEach(callback => {
            try {
              callback()
            } catch (error) {
              logger.error(`Error in viewport update subscriber:`, createErrorMetadata(error))
            }
          })
        }
      },

      get: <T>(path: SettingsPath): T => {
        const settings = get().settings

        if (!path || path === '') {
          return settings as unknown as T
        }

        // Navigate the settings object using the path
        let current: any = settings
        const pathParts = path.split('.')

        for (const part of pathParts) {
          if (current === undefined || current === null) {
            return undefined as unknown as T
          }
          current = current[part]
        }

        return current as T
      },

      // Deprecated method - kept for backward compatibility
      // Internally uses updateSettings for consistency
      set: <T>(path: SettingsPath, value: T) => {
        if (debugState.isEnabled()) {
          logger.warn(`Deprecated: set('${path}', value) called. Use updateSettings() instead.`);
        }

        const state = get();

        // Use updateSettings internally
        state.updateSettings((draft) => {
          // If setting the entire object
          if (!path || path === '') {
            Object.assign(draft, value);
            return;
          }

          // Navigate to the correct location and update
          const pathParts = path.split('.');
          let current: any = draft;

          // Navigate to the parent of the setting we want to update
          for (let i = 0; i < pathParts.length - 1; i++) {
            const part = pathParts[i];
            if (current[part] === undefined || current[part] === null) {
              current[part] = {};
            }
            current = current[part];
          }

          // Update the value
          const finalPart = pathParts[pathParts.length - 1];
          current[finalPart] = value;
        });
      },

      // Legacy set implementation (commented out for reference)
      /*
      set: <T>(path: SettingsPath, value: T) => {
        // Implementation moved to use updateSettings
      }
      */

      subscribe: (path: SettingsPath, callback: () => void, immediate: boolean = true) => {
        set(state => {
          const subscribers = new Map(state.subscribers)

          if (!subscribers.has(path)) {
            subscribers.set(path, new Set())
          }

          subscribers.get(path)!.add(callback)

          return { subscribers }
        })

        // Call callback immediately if requested and initialized
        if (immediate && get().initialized) {
          callback()
        }

        // Return unsubscribe function
        return () => get().unsubscribe(path, callback)
      },

      unsubscribe: (path: SettingsPath, callback: () => void) => {
        set(state => {
          const subscribers = new Map(state.subscribers)

          if (subscribers.has(path)) {
            const callbacks = subscribers.get(path)!
            callbacks.delete(callback)

            if (callbacks.size === 0) {
              subscribers.delete(path)
            }
          }

          return { subscribers }
        })
      },

      // Immer-based updateSettings - the preferred method for updating settings
      updateSettings: (updater) => {
        set((state) => produce(state, (draft) => {
          updater(draft.settings);
        }));

        // After state update, handle notifications and saving
        const state = get();

        // Notify all subscribers
        const allCallbacks = new Set<() => void>();
        state.subscribers.forEach(callbacks => {
          callbacks.forEach(cb => allCallbacks.add(cb));
        });

        Array.from(allCallbacks).forEach(callback => {
          try {
            callback();
          } catch (error) {
            logger.error('Error in settings subscriber during updateSettings:', createErrorMetadata(error));
          }
        });

        // Schedule save to server
        scheduleSave(state.settings, state.initialized);
      },

      // The subscribe and unsubscribe functions below were duplicated and are removed by this change.
    }),
    {
      name: 'graph-viz-settings',
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        settings: state.settings,
        authenticated: state.authenticated,
        user: state.user,
        isPowerUser: state.isPowerUser
      }),
      onRehydrateStorage: () => (state) => {
        if (state && state.settings) {
          // Apply migration when rehydrating from storage
          state.settings = migrateToMultiGraphSettings(state.settings);
          if (debugState.isEnabled()) {
            logger.info('Settings migrated during rehydration');
          }
        }
      }
    }
  )
)

// Export for testing and direct access
export const settingsStoreUtils = {
  debouncedSaveToServer,
  scheduleSave
};
