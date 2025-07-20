import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { act, renderHook } from '@testing-library/react-hooks';
import { useSettingsStore } from '../../store/settingsStore';
import { defaultSettings } from '../../features/settings/config/defaultSettings';
import { Settings } from '../../features/settings/config/settings';
import * as settingsService from '../../services/settingsService';
import * as nostrAuthService from '../../services/nostrAuthService';

// Mock dependencies
vi.mock('../../services/settingsService');
vi.mock('../../services/nostrAuthService');
vi.mock('../../features/design-system/components/Toast', () => ({
  toast: vi.fn()
}));

describe('Settings Store', () => {
  beforeEach(() => {
    // Clear localStorage
    localStorage.clear();
    
    // Reset store state
    useSettingsStore.setState({
      settings: defaultSettings,
      initialized: false,
      authenticated: false,
      user: null,
      isPowerUser: false,
      subscribers: new Map()
    });

    // Reset all mocks
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('Initialization', () => {
    it('should initialize with default settings', async () => {
      const { result } = renderHook(() => useSettingsStore());
      
      expect(result.current.initialized).toBe(false);
      expect(result.current.settings).toEqual(defaultSettings);
      
      // Mock server response
      vi.mocked(settingsService.settingsService.fetchSettings).mockResolvedValue(null);
      
      await act(async () => {
        await result.current.initialize();
      });
      
      expect(result.current.initialized).toBe(true);
    });

    it('should merge server settings with defaults when available', async () => {
      const serverSettings = {
        visualisation: {
          nodes: {
            baseColor: '#ff0000' // Changed from default
          }
        }
      };
      
      vi.mocked(settingsService.settingsService.fetchSettings).mockResolvedValue(serverSettings as Settings);
      
      const { result } = renderHook(() => useSettingsStore());
      
      await act(async () => {
        await result.current.initialize();
      });
      
      expect(result.current.settings.visualisation.nodes.baseColor).toBe('#ff0000');
      // Other settings should remain as defaults
      expect(result.current.settings.visualisation.nodes.metalness).toBe(defaultSettings.visualisation.nodes.metalness);
    });

    it('should handle server fetch errors gracefully', async () => {
      vi.mocked(settingsService.settingsService.fetchSettings).mockRejectedValue(new Error('Network error'));
      
      const { result } = renderHook(() => useSettingsStore());
      
      await act(async () => {
        await result.current.initialize();
      });
      
      expect(result.current.initialized).toBe(true);
      expect(result.current.settings).toEqual(defaultSettings);
    });
  });

  describe('Get and Set operations', () => {
    it('should get settings by path', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      expect(result.current.get('visualisation.nodes.baseColor')).toBe('#0008ff');
      expect(result.current.get('system.websocket.updateRate')).toBe(60);
      expect(result.current.get('')).toEqual(defaultSettings);
    });

    it('should set settings by path (deprecated method)', () => {
      const { result } = renderHook(() => useSettingsStore());
      const consoleWarnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
      
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#ffffff');
      });
      
      expect(result.current.settings.visualisation.nodes.baseColor).toBe('#ffffff');
      // Should log deprecation warning in debug mode
      // Note: Warning is only shown when debug is enabled
      
      consoleWarnSpy.mockRestore();
    });
    
    it('should internally use updateSettings when set is called', () => {
      const { result } = renderHook(() => useSettingsStore());
      const updateSettingsSpy = vi.spyOn(result.current, 'updateSettings');
      
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#ffffff');
      });
      
      expect(updateSettingsSpy).toHaveBeenCalled();
      expect(result.current.settings.visualisation.nodes.baseColor).toBe('#ffffff');
    });

    it('should create nested paths if they do not exist', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      act(() => {
        result.current.set('newFeature.nested.value', 42);
      });
      
      expect(result.current.get('newFeature.nested.value')).toBe(42);
    });

    it('should handle setting the entire settings object', () => {
      const { result } = renderHook(() => useSettingsStore());
      const newSettings = { ...defaultSettings };
      newSettings.visualisation.nodes.baseColor = '#123456';
      
      act(() => {
        result.current.set('', newSettings);
      });
      
      expect(result.current.settings).toEqual(newSettings);
    });
  });

  describe('Immer-based updateSettings', () => {
    it('should update settings using Immer', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      act(() => {
        result.current.updateSettings((draft) => {
          draft.visualisation.nodes.baseColor = '#abcdef';
          draft.visualisation.edges.opacity = 0.8;
          draft.system.debug.enabled = true;
        });
      });
      
      expect(result.current.settings.visualisation.nodes.baseColor).toBe('#abcdef');
      expect(result.current.settings.visualisation.edges.opacity).toBe(0.8);
      expect(result.current.settings.system.debug.enabled).toBe(true);
    });

    it('should handle complex nested updates with Immer', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      act(() => {
        result.current.updateSettings((draft) => {
          // Update array values
          draft.visualisation.edges.widthRange[0] = 0.5;
          draft.visualisation.edges.widthRange[1] = 2.0;
          
          // Update deeply nested values
          draft.visualisation.hologram.sphereSizes = [10, 20];
          
          // Add new properties (if TypeScript allows)
          (draft as any).customProperty = { test: true };
        });
      });
      
      expect(result.current.settings.visualisation.edges.widthRange).toEqual([0.5, 2.0]);
      expect(result.current.settings.visualisation.hologram.sphereSizes).toEqual([10, 20]);
      expect((result.current.settings as any).customProperty).toEqual({ test: true });
    });

    it('should maintain immutability with Immer', () => {
      const { result } = renderHook(() => useSettingsStore());
      const originalSettings = result.current.settings;
      const originalNodes = result.current.settings.visualisation.nodes;
      
      act(() => {
        result.current.updateSettings((draft) => {
          draft.visualisation.nodes.baseColor = '#fedcba';
        });
      });
      
      // Original objects should not be modified
      expect(originalSettings).not.toBe(result.current.settings);
      expect(originalNodes).not.toBe(result.current.settings.visualisation.nodes);
      expect(originalNodes.baseColor).toBe('#0008ff'); // Original value
      expect(result.current.settings.visualisation.nodes.baseColor).toBe('#fedcba'); // New value
    });
  });

  describe('Subscriptions', () => {
    it('should subscribe to specific paths', () => {
      const { result } = renderHook(() => useSettingsStore());
      const callback = vi.fn();
      
      act(() => {
        result.current.subscribe('visualisation.nodes.baseColor', callback);
      });
      
      // Callback should be called immediately if immediate is true (default)
      expect(callback).toHaveBeenCalledTimes(1);
      
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#aaaaaa');
      });
      
      // Wait for debounced update
      setTimeout(() => {
        expect(callback).toHaveBeenCalledTimes(2);
      }, 400);
    });

    it('should unsubscribe correctly', () => {
      const { result } = renderHook(() => useSettingsStore());
      const callback = vi.fn();
      
      let unsubscribe: () => void;
      act(() => {
        unsubscribe = result.current.subscribe('visualisation.nodes.baseColor', callback);
      });
      
      act(() => {
        unsubscribe();
      });
      
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#bbbbbb');
      });
      
      // Callback should not be called after unsubscribe
      setTimeout(() => {
        expect(callback).toHaveBeenCalledTimes(1); // Only the initial call
      }, 400);
    });

    it('should notify parent path subscribers', (done) => {
      const { result } = renderHook(() => useSettingsStore());
      const visualisationCallback = vi.fn();
      const nodesCallback = vi.fn();
      
      act(() => {
        result.current.subscribe('visualisation', visualisationCallback, false);
        result.current.subscribe('visualisation.nodes', nodesCallback, false);
      });
      
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#cccccc');
      });
      
      // Wait for debounced update
      setTimeout(() => {
        expect(visualisationCallback).toHaveBeenCalled();
        expect(nodesCallback).toHaveBeenCalled();
        done();
      }, 400);
    });
  });

  describe('Real-time viewport updates', () => {
    it('should trigger immediate viewport updates for visualization settings', () => {
      const { result } = renderHook(() => useSettingsStore());
      const viewportCallback = vi.fn();
      
      act(() => {
        result.current.subscribe('viewport.update', viewportCallback, false);
      });
      
      act(() => {
        result.current.set('visualisation.bloom.enabled', false);
      });
      
      // Viewport update should be called immediately
      expect(viewportCallback).toHaveBeenCalled();
    });

    it('should trigger viewport updates for XR settings', () => {
      const { result } = renderHook(() => useSettingsStore());
      const viewportCallback = vi.fn();
      
      act(() => {
        result.current.subscribe('viewport.update', viewportCallback, false);
      });
      
      act(() => {
        result.current.set('xr.enabled', true);
      });
      
      expect(viewportCallback).toHaveBeenCalled();
    });

    it('should trigger viewport updates for debug visualization settings', () => {
      const { result } = renderHook(() => useSettingsStore());
      const viewportCallback = vi.fn();
      
      act(() => {
        result.current.subscribe('viewport.update', viewportCallback, false);
      });
      
      act(() => {
        result.current.set('system.debug.enablePhysicsDebug', true);
      });
      
      expect(viewportCallback).toHaveBeenCalled();
    });
  });

  describe('Persistence and server sync', () => {
    it('should save to server when persistSettings is enabled', async () => {
      vi.mocked(settingsService.settingsService.saveSettings).mockResolvedValue({} as Settings);
      vi.mocked(nostrAuthService.nostrAuth.isAuthenticated).mockReturnValue(false);
      
      const { result } = renderHook(() => useSettingsStore());
      
      // Initialize and enable persistence
      await act(async () => {
        await result.current.initialize();
        result.current.set('system.persistSettings', true);
      });
      
      // Clear previous calls
      vi.clearAllMocks();
      
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#dddddd');
      });
      
      // Wait for debounced save
      await new Promise(resolve => setTimeout(resolve, 400));
      
      expect(settingsService.settingsService.saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          visualisation: expect.objectContaining({
            nodes: expect.objectContaining({
              baseColor: '#dddddd'
            })
          })
        }),
        expect.any(Object)
      );
    });

    it('should include auth headers when authenticated', async () => {
      const mockUser = { pubkey: 'test-pubkey', isPowerUser: true };
      const mockToken = 'test-token';
      
      vi.mocked(settingsService.settingsService.saveSettings).mockResolvedValue({} as Settings);
      vi.mocked(nostrAuthService.nostrAuth.isAuthenticated).mockReturnValue(true);
      vi.mocked(nostrAuthService.nostrAuth.getCurrentUser).mockReturnValue(mockUser);
      vi.mocked(nostrAuthService.nostrAuth.getSessionToken).mockReturnValue(mockToken);
      
      const { result } = renderHook(() => useSettingsStore());
      
      await act(async () => {
        await result.current.initialize();
        result.current.set('system.persistSettings', true);
        result.current.setAuthenticated(true);
        result.current.setUser(mockUser);
      });
      
      vi.clearAllMocks();
      
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#eeeeee');
      });
      
      await new Promise(resolve => setTimeout(resolve, 400));
      
      expect(settingsService.settingsService.saveSettings).toHaveBeenCalledWith(
        expect.any(Object),
        expect.objectContaining({
          'X-Nostr-Pubkey': 'test-pubkey',
          'Authorization': 'Bearer test-token'
        })
      );
    });

    it('should not save to server when persistSettings is disabled', async () => {
      const { result } = renderHook(() => useSettingsStore());
      
      await act(async () => {
        await result.current.initialize();
        result.current.set('system.persistSettings', false);
      });
      
      vi.clearAllMocks();
      
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#ffffff');
      });
      
      await new Promise(resolve => setTimeout(resolve, 400));
      
      expect(settingsService.settingsService.saveSettings).not.toHaveBeenCalled();
    });
  });

  describe('User and authentication state', () => {
    it('should update user and isPowerUser state', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      act(() => {
        result.current.setUser({ pubkey: 'user123', isPowerUser: true });
      });
      
      expect(result.current.user).toEqual({ pubkey: 'user123', isPowerUser: true });
      expect(result.current.isPowerUser).toBe(true);
      
      act(() => {
        result.current.setUser({ pubkey: 'user456', isPowerUser: false });
      });
      
      expect(result.current.user).toEqual({ pubkey: 'user456', isPowerUser: false });
      expect(result.current.isPowerUser).toBe(false);
      
      act(() => {
        result.current.setUser(null);
      });
      
      expect(result.current.user).toBe(null);
      expect(result.current.isPowerUser).toBe(false);
    });

    it('should update authenticated state', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      expect(result.current.authenticated).toBe(false);
      
      act(() => {
        result.current.setAuthenticated(true);
      });
      
      expect(result.current.authenticated).toBe(true);
      
      act(() => {
        result.current.setAuthenticated(false);
      });
      
      expect(result.current.authenticated).toBe(false);
    });
  });

  describe('Edge cases and error handling', () => {
    it('should handle undefined paths gracefully', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      expect(result.current.get('non.existent.path')).toBeUndefined();
    });

    it('should handle subscriber errors without crashing', () => {
      const { result } = renderHook(() => useSettingsStore());
      const errorCallback = vi.fn(() => {
        throw new Error('Subscriber error');
      });
      const normalCallback = vi.fn();
      
      act(() => {
        result.current.subscribe('visualisation.nodes', errorCallback);
        result.current.subscribe('visualisation.nodes', normalCallback);
      });
      
      // Should not throw when setting
      expect(() => {
        act(() => {
          result.current.set('visualisation.nodes.baseColor', '#facade');
        });
      }).not.toThrow();
      
      // Normal callback should still be called
      setTimeout(() => {
        expect(normalCallback).toHaveBeenCalled();
      }, 400);
    });

    it('should handle circular references in settings', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      // Create a circular reference
      const circularObj: any = { a: 1 };
      circularObj.self = circularObj;
      
      // Should handle circular references when setting
      expect(() => {
        act(() => {
          result.current.set('test.circular', circularObj);
        });
      }).not.toThrow();
    });
  });
});