import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react-hooks';
import { useSettingsStore } from '../../store/settingsStore';
import {
  useSelectiveSetting,
  useSelectiveSettings,
  useSettingSetter,
  useSettingsSubscription
} from '../../hooks/useSelectiveSettingsStore';
import { defaultSettings } from '../../features/settings/config/defaultSettings';
import * as settingsService from '../../services/settingsService';
import * as nostrAuthService from '../../services/nostrAuthService';

// Mock dependencies
vi.mock('../../services/settingsService');
vi.mock('../../services/nostrAuthService');
vi.mock('../../features/design-system/components/Toast', () => ({
  toast: vi.fn()
}));

describe('Settings Integration Tests', () => {
  beforeEach(() => {
    localStorage.clear();
    
    // Reset store
    useSettingsStore.setState({
      settings: defaultSettings,
      initialized: false,
      authenticated: false,
      user: null,
      isPowerUser: false,
      subscribers: new Map()
    });
    
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('Full Settings Flow', () => {
    it('should handle complete settings lifecycle', async () => {
      // 1. Initialize from server
      const serverSettings = {
        visualisation: {
          nodes: {
            baseColor: '#server-color'
          }
        }
      };
      
      vi.mocked(settingsService.settingsService.fetchSettings).mockResolvedValue(serverSettings as any);
      vi.mocked(settingsService.settingsService.saveSettings).mockResolvedValue({} as any);
      
      const { result: storeResult } = renderHook(() => useSettingsStore());
      
      await act(async () => {
        await storeResult.current.initialize();
      });
      
      expect(storeResult.current.settings.visualisation.nodes.baseColor).toBe('#server-color');
      
      // 2. Component subscribes to specific setting
      const { result: selectiveResult } = renderHook(() => 
        useSelectiveSetting<string>('visualisation.nodes.baseColor')
      );
      
      expect(selectiveResult.current).toBe('#server-color');
      
      // 3. Update setting through setter hook
      const { result: setterResult } = renderHook(() => useSettingSetter());
      
      await act(async () => {
        setterResult.current.set('visualisation.nodes.baseColor', '#user-color');
      });
      
      // 4. Verify update propagated
      expect(selectiveResult.current).toBe('#user-color');
      expect(storeResult.current.settings.visualisation.nodes.baseColor).toBe('#user-color');
      
      // 5. Wait for debounced save
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 400));
      });
      
      // 6. Verify save was called
      expect(settingsService.settingsService.saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          visualisation: expect.objectContaining({
            nodes: expect.objectContaining({
              baseColor: '#user-color'
            })
          })
        }),
        expect.any(Object)
      );
    });

    it('should coordinate multiple components with different subscriptions', async () => {
      const { result: storeResult } = renderHook(() => useSettingsStore());
      
      await act(async () => {
        await storeResult.current.initialize();
      });
      
      // Component 1: Subscribes to multiple settings
      const { result: multiResult } = renderHook(() => 
        useSelectiveSettings({
          nodeColor: 'visualisation.nodes.baseColor',
          edgeColor: 'visualisation.edges.color',
          bloomEnabled: 'visualisation.bloom.enabled'
        })
      );
      
      // Component 2: Subscribes to single setting
      const { result: singleResult } = renderHook(() => 
        useSelectiveSetting<boolean>('visualisation.bloom.enabled')
      );
      
      // Component 3: Uses subscription with callback
      const callbackFn = vi.fn();
      renderHook(() => 
        useSettingsSubscription('visualisation.bloom', callbackFn)
      );
      
      // Initial values
      expect(multiResult.current.bloomEnabled).toBe(true);
      expect(singleResult.current).toBe(true);
      
      // Update through setter
      const { result: setterResult } = renderHook(() => useSettingSetter());
      
      act(() => {
        setterResult.current.batchedSet({
          'visualisation.bloom.enabled': false,
          'visualisation.bloom.strength': 3.0,
          'visualisation.nodes.baseColor': '#updated'
        });
      });
      
      // All components should reflect the change
      expect(multiResult.current.bloomEnabled).toBe(false);
      expect(multiResult.current.nodeColor).toBe('#updated');
      expect(singleResult.current).toBe(false);
      
      // Callback should be triggered
      expect(callbackFn).toHaveBeenCalledWith(
        expect.objectContaining({
          enabled: false,
          strength: 3.0
        })
      );
    });
  });

  describe('Authentication Integration', () => {
    it('should handle authenticated settings sync', async () => {
      const mockUser = { pubkey: 'test-pubkey', isPowerUser: true };
      const mockToken = 'test-token';
      
      vi.mocked(nostrAuthService.nostrAuth.isAuthenticated).mockReturnValue(true);
      vi.mocked(nostrAuthService.nostrAuth.getCurrentUser).mockReturnValue(mockUser);
      vi.mocked(nostrAuthService.nostrAuth.getSessionToken).mockReturnValue(mockToken);
      vi.mocked(settingsService.settingsService.saveSettings).mockResolvedValue({} as any);
      
      const { result } = renderHook(() => useSettingsStore());
      
      // Set up authentication
      await act(async () => {
        await result.current.initialize();
        result.current.setAuthenticated(true);
        result.current.setUser(mockUser);
      });
      
      // Verify power user state
      expect(result.current.isPowerUser).toBe(true);
      
      // Update a setting
      act(() => {
        result.current.set('visualisation.nodes.quality', 'high');
      });
      
      // Wait for save
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 400));
      });
      
      // Verify authenticated headers were included
      expect(settingsService.settingsService.saveSettings).toHaveBeenCalledWith(
        expect.any(Object),
        expect.objectContaining({
          'X-Nostr-Pubkey': 'test-pubkey',
          'Authorization': 'Bearer test-token'
        })
      );
    });

    it('should handle unauthenticated users', async () => {
      vi.mocked(nostrAuthService.nostrAuth.isAuthenticated).mockReturnValue(false);
      vi.mocked(settingsService.settingsService.saveSettings).mockResolvedValue({} as any);
      
      const { result } = renderHook(() => useSettingsStore());
      
      await act(async () => {
        await result.current.initialize();
      });
      
      expect(result.current.authenticated).toBe(false);
      expect(result.current.isPowerUser).toBe(false);
      
      // Update setting
      act(() => {
        result.current.set('visualisation.nodes.opacity', 0.5);
      });
      
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 400));
      });
      
      // Should save without auth headers
      expect(settingsService.settingsService.saveSettings).toHaveBeenCalledWith(
        expect.any(Object),
        expect.not.objectContaining({
          'X-Nostr-Pubkey': expect.any(String),
          'Authorization': expect.any(String)
        })
      );
    });
  });

  describe('Real-time Updates Integration', () => {
    it('should coordinate viewport updates across components', () => {
      const viewportCallbacks = {
        component1: vi.fn(),
        component2: vi.fn(),
        component3: vi.fn()
      };
      
      const { result } = renderHook(() => useSettingsStore());
      
      // Multiple components subscribe to viewport updates
      act(() => {
        result.current.subscribe('viewport.update', viewportCallbacks.component1, false);
        result.current.subscribe('viewport.update', viewportCallbacks.component2, false);
        result.current.subscribe('viewport.update', viewportCallbacks.component3, false);
      });
      
      // Update a visualization setting
      act(() => {
        result.current.set('visualisation.bloom.radius', 0.8);
      });
      
      // All viewport callbacks should be called immediately
      expect(viewportCallbacks.component1).toHaveBeenCalled();
      expect(viewportCallbacks.component2).toHaveBeenCalled();
      expect(viewportCallbacks.component3).toHaveBeenCalled();
    });

    it('should handle mixed real-time and debounced updates', () => {
      const callbacks = {
        viewport: vi.fn(),
        regular: vi.fn(),
        specific: vi.fn()
      };
      
      const { result } = renderHook(() => useSettingsStore());
      
      act(() => {
        result.current.subscribe('viewport.update', callbacks.viewport, false);
        result.current.subscribe('system', callbacks.regular, false);
        result.current.subscribe('visualisation.bloom.strength', callbacks.specific, false);
      });
      
      // Clear initial calls
      Object.values(callbacks).forEach(cb => cb.mockClear());
      
      // Perform updates
      act(() => {
        // Real-time update
        result.current.set('visualisation.bloom.strength', 2.5);
        // Regular update
        result.current.set('system.persistSettings', false);
      });
      
      // Viewport should be called immediately
      expect(callbacks.viewport).toHaveBeenCalledTimes(1);
      
      // Others are debounced
      expect(callbacks.regular).toHaveBeenCalledTimes(0);
      expect(callbacks.specific).toHaveBeenCalledTimes(0);
    });
  });

  describe('Error Recovery Integration', () => {
    it('should recover from server save failures', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      vi.mocked(settingsService.settingsService.saveSettings).mockRejectedValue(new Error('Network error'));
      
      const { result } = renderHook(() => useSettingsStore());
      
      await act(async () => {
        await result.current.initialize();
      });
      
      // Update setting
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#error-test');
      });
      
      // Wait for save attempt
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 400));
      });
      
      // Should not crash, setting should still be updated locally
      expect(result.current.settings.visualisation.nodes.baseColor).toBe('#error-test');
      
      // Error should be logged
      expect(consoleSpy).toHaveBeenCalled();
      
      consoleSpy.mockRestore();
    });

    it('should handle subscription errors gracefully', () => {
      const { result } = renderHook(() => useSettingsStore());
      const errorCallback = vi.fn(() => { throw new Error('Subscriber error'); });
      const normalCallback = vi.fn();
      
      act(() => {
        result.current.subscribe('visualisation', errorCallback, false);
        result.current.subscribe('visualisation', normalCallback, false);
      });
      
      // Update should not crash despite error
      expect(() => {
        act(() => {
          result.current.set('visualisation.nodes.opacity', 0.7);
        });
      }).not.toThrow();
      
      // Wait for debounced callbacks
      setTimeout(() => {
        expect(normalCallback).toHaveBeenCalled();
      }, 400);
    });
  });

  describe('Performance Integration', () => {
    it('should handle high-frequency updates efficiently', async () => {
      const { result: storeResult } = renderHook(() => useSettingsStore());
      const { result: setterResult } = renderHook(() => useSettingSetter());
      
      await act(async () => {
        await storeResult.current.initialize();
      });
      
      // Track render counts
      let renderCount = 0;
      const { result: selectiveResult } = renderHook(() => {
        renderCount++;
        return useSelectiveSetting<number>('visualisation.physics.iterations');
      });
      
      const initialRenderCount = renderCount;
      
      // Perform 100 rapid updates
      const startTime = performance.now();
      
      act(() => {
        for (let i = 0; i < 100; i++) {
          setterResult.current.set('visualisation.physics.iterations', i);
        }
      });
      
      const endTime = performance.now();
      const duration = endTime - startTime;
      
      // Should be very fast despite many updates
      expect(duration).toBeLessThan(50);
      
      // Value should be the last one
      expect(selectiveResult.current).toBe(99);
      
      // Should not cause excessive re-renders (debounced)
      expect(renderCount).toBe(initialRenderCount);
    });

    it('should efficiently handle complex state shapes', () => {
      const { result: storeResult } = renderHook(() => useSettingsStore());
      const { result: setterResult } = renderHook(() => useSettingSetter());
      
      // Create a complex update
      const complexUpdate: Record<string, any> = {};
      
      // Add nested paths
      for (let i = 0; i < 20; i++) {
        complexUpdate[`visualisation.custom.feature${i}.enabled`] = true;
        complexUpdate[`visualisation.custom.feature${i}.config.value`] = i;
        complexUpdate[`visualisation.custom.feature${i}.config.nested.deep`] = `value-${i}`;
      }
      
      const startTime = performance.now();
      
      act(() => {
        setterResult.current.batchedSet(complexUpdate);
      });
      
      const endTime = performance.now();
      const duration = endTime - startTime;
      
      // Should handle complex updates efficiently
      expect(duration).toBeLessThan(20);
      
      // Verify some values
      const settings = storeResult.current.settings as any;
      expect(settings.visualisation.custom.feature0.enabled).toBe(true);
      expect(settings.visualisation.custom.feature10.config.value).toBe(10);
      expect(settings.visualisation.custom.feature19.config.nested.deep).toBe('value-19');
    });
  });

  describe('Persistence Integration', () => {
    it('should persist settings across sessions', async () => {
      const { result: storeResult1 } = renderHook(() => useSettingsStore());
      
      // First session - update settings
      await act(async () => {
        await storeResult1.current.initialize();
        storeResult1.current.set('visualisation.nodes.baseColor', '#persisted');
        storeResult1.current.set('visualisation.bloom.enabled', false);
      });
      
      // Simulate new session by creating new store instance
      useSettingsStore.setState({
        settings: defaultSettings,
        initialized: false,
        authenticated: false,
        user: null,
        isPowerUser: false,
        subscribers: new Map()
      });
      
      const { result: storeResult2 } = renderHook(() => useSettingsStore());
      
      // Initialize should load from localStorage
      await act(async () => {
        await storeResult2.current.initialize();
      });
      
      // Settings should be restored
      expect(storeResult2.current.settings.visualisation.nodes.baseColor).toBe('#persisted');
      expect(storeResult2.current.settings.visualisation.bloom.enabled).toBe(false);
    });
  });
});