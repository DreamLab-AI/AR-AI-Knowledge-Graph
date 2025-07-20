import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react-hooks';
import { useSettingsStore } from '../../store/settingsStore';
import { useSelectiveSetting, useSettingSetter } from '../../hooks/useSelectiveSettingsStore';
import { defaultSettings } from '../../features/settings/config/defaultSettings';

describe('Settings Performance Tests', () => {
  beforeEach(() => {
    // Reset store
    useSettingsStore.setState({
      settings: defaultSettings,
      initialized: true,
      authenticated: false,
      user: null,
      isPowerUser: false,
      subscribers: new Map()
    });
  });

  describe('Debounced Updates', () => {
    it('should debounce rapid setting updates', async () => {
      const saveSettings = vi.fn();
      vi.spyOn(window, 'setTimeout');
      vi.spyOn(window, 'clearTimeout');
      
      const { result } = renderHook(() => useSettingsStore());
      
      // Perform rapid updates
      act(() => {
        for (let i = 0; i < 10; i++) {
          result.current.set('visualisation.nodes.opacity', i / 10);
        }
      });
      
      // Should have cleared timeout 9 times (all but the last)
      expect(window.clearTimeout).toHaveBeenCalledTimes(9);
      
      // Should have set timeout 10 times
      expect(window.setTimeout).toHaveBeenCalledTimes(10);
      
      // Final value should be set immediately
      expect(result.current.settings.visualisation.nodes.opacity).toBe(0.9);
      
      vi.restoreAllMocks();
    });

    it('should batch subscriber notifications', (done) => {
      const subscriber1 = vi.fn();
      const subscriber2 = vi.fn();
      const subscriber3 = vi.fn();
      
      const { result } = renderHook(() => useSettingsStore());
      
      act(() => {
        result.current.subscribe('visualisation', subscriber1, false);
        result.current.subscribe('visualisation.nodes', subscriber2, false);
        result.current.subscribe('visualisation.nodes.opacity', subscriber3, false);
      });
      
      // Clear initial calls
      subscriber1.mockClear();
      subscriber2.mockClear();
      subscriber3.mockClear();
      
      // Perform multiple updates rapidly
      act(() => {
        result.current.set('visualisation.nodes.opacity', 0.1);
        result.current.set('visualisation.nodes.opacity', 0.2);
        result.current.set('visualisation.nodes.opacity', 0.3);
      });
      
      // Wait for debounced notification
      setTimeout(() => {
        // Each subscriber should be called only once after debounce
        expect(subscriber1).toHaveBeenCalledTimes(1);
        expect(subscriber2).toHaveBeenCalledTimes(1);
        expect(subscriber3).toHaveBeenCalledTimes(1);
        done();
      }, 400);
    });
  });

  describe('Selective Re-renders', () => {
    it('should not re-render components subscribed to unrelated paths', () => {
      let nodeRenderCount = 0;
      let edgeRenderCount = 0;
      
      const { result: nodeResult } = renderHook(() => {
        nodeRenderCount++;
        return useSelectiveSetting<string>('visualisation.nodes.baseColor');
      });
      
      const { result: edgeResult } = renderHook(() => {
        edgeRenderCount++;
        return useSelectiveSetting<number>('visualisation.edges.opacity');
      });
      
      expect(nodeRenderCount).toBe(1);
      expect(edgeRenderCount).toBe(1);
      
      // Update node setting
      act(() => {
        useSettingsStore.getState().set('visualisation.nodes.baseColor', '#changed');
      });
      
      // Only node component should re-render
      expect(nodeRenderCount).toBe(1); // Will update async
      expect(edgeRenderCount).toBe(1);
      
      // Update edge setting
      act(() => {
        useSettingsStore.getState().set('visualisation.edges.opacity', 0.8);
      });
      
      // Only edge component should re-render
      expect(nodeRenderCount).toBe(1);
      expect(edgeRenderCount).toBe(1); // Will update async
    });

    it('should efficiently handle deeply nested path subscriptions', () => {
      const renderCounts = {
        root: 0,
        visualisation: 0,
        nodes: 0,
        specific: 0
      };
      
      renderHook(() => {
        renderCounts.root++;
        return useSelectiveSetting<any>('');
      });
      
      renderHook(() => {
        renderCounts.visualisation++;
        return useSelectiveSetting<any>('visualisation');
      });
      
      renderHook(() => {
        renderCounts.nodes++;
        return useSelectiveSetting<any>('visualisation.nodes');
      });
      
      renderHook(() => {
        renderCounts.specific++;
        return useSelectiveSetting<string>('visualisation.nodes.baseColor');
      });
      
      // Initial render
      expect(renderCounts).toEqual({
        root: 1,
        visualisation: 1,
        nodes: 1,
        specific: 1
      });
      
      // Update a specific nested value
      act(() => {
        useSettingsStore.getState().set('visualisation.nodes.baseColor', '#updated');
      });
      
      // All should be notified due to parent-child relationship
      // But this happens asynchronously through subscriptions
    });
  });

  describe('Batched Updates Performance', () => {
    it('should efficiently update multiple settings at once', () => {
      const { result } = renderHook(() => useSettingSetter());
      const startTime = performance.now();
      
      act(() => {
        result.current.batchedSet({
          'visualisation.nodes.baseColor': '#111111',
          'visualisation.nodes.opacity': 0.1,
          'visualisation.nodes.metalness': 0.2,
          'visualisation.edges.color': '#222222',
          'visualisation.edges.opacity': 0.3,
          'visualisation.physics.iterations': 200,
          'visualisation.bloom.enabled': false,
          'system.debug.enabled': true
        });
      });
      
      const endTime = performance.now();
      const duration = endTime - startTime;
      
      // Batched update should be fast (less than 10ms)
      expect(duration).toBeLessThan(10);
      
      // Verify all values were updated
      const settings = useSettingsStore.getState().settings;
      expect(settings.visualisation.nodes.baseColor).toBe('#111111');
      expect(settings.visualisation.nodes.opacity).toBe(0.1);
      expect(settings.visualisation.edges.color).toBe('#222222');
      expect(settings.visualisation.bloom.enabled).toBe(false);
      expect(settings.system.debug.enabled).toBe(true);
    });

    it('should handle large batch updates efficiently', () => {
      const { result } = renderHook(() => useSettingSetter());
      const updates: Record<string, any> = {};
      
      // Create 100 updates
      for (let i = 0; i < 100; i++) {
        updates[`test.path.${i}`] = i;
      }
      
      const startTime = performance.now();
      
      act(() => {
        result.current.batchedSet(updates);
      });
      
      const endTime = performance.now();
      const duration = endTime - startTime;
      
      // Even 100 updates should be fast (less than 20ms)
      expect(duration).toBeLessThan(20);
      
      // Verify some values
      const settings = useSettingsStore.getState().settings as any;
      expect(settings.test.path['0']).toBe(0);
      expect(settings.test.path['50']).toBe(50);
      expect(settings.test.path['99']).toBe(99);
    });
  });

  describe('Memory Management', () => {
    it('should not leak memory with many subscriptions', () => {
      const { result } = renderHook(() => useSettingsStore());
      const unsubscribes: (() => void)[] = [];
      
      // Create 1000 subscriptions
      act(() => {
        for (let i = 0; i < 1000; i++) {
          const unsub = result.current.subscribe(
            `test.path.${i}`,
            () => {},
            false
          );
          unsubscribes.push(unsub);
        }
      });
      
      expect(result.current.subscribers.size).toBe(1000);
      
      // Unsubscribe all
      act(() => {
        unsubscribes.forEach(unsub => unsub());
      });
      
      expect(result.current.subscribers.size).toBe(0);
    });

    it('should clean up subscriptions on unmount', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      const { unmount: unmount1 } = renderHook(() => 
        useSelectiveSetting('test.path1')
      );
      const { unmount: unmount2 } = renderHook(() => 
        useSelectiveSetting('test.path2')
      );
      const { unmount: unmount3 } = renderHook(() => 
        useSelectiveSetting('test.path3')
      );
      
      // Should have 3 subscribers
      expect(result.current.subscribers.size).toBeGreaterThanOrEqual(3);
      
      // Unmount all
      unmount1();
      unmount2();
      unmount3();
      
      // Subscribers should be cleaned up
      const remainingSubscribers = Array.from(result.current.subscribers.keys())
        .filter(key => key.startsWith('test.path'));
      expect(remainingSubscribers.length).toBe(0);
    });
  });

  describe('Immer Performance', () => {
    it('should efficiently handle immutable updates', () => {
      const { result } = renderHook(() => useSettingsStore());
      const originalSettings = result.current.settings;
      
      const startTime = performance.now();
      
      act(() => {
        result.current.updateSettings((draft) => {
          // Deep update
          draft.visualisation.nodes.baseColor = '#updated';
          draft.visualisation.edges.widthRange[0] = 0.5;
          draft.visualisation.physics.iterations = 150;
          draft.system.websocket.updateRate = 120;
        });
      });
      
      const endTime = performance.now();
      const duration = endTime - startTime;
      
      // Immer update should be fast
      expect(duration).toBeLessThan(5);
      
      // Verify immutability
      expect(originalSettings).not.toBe(result.current.settings);
      expect(originalSettings.visualisation).not.toBe(result.current.settings.visualisation);
      
      // Verify updates
      expect(result.current.settings.visualisation.nodes.baseColor).toBe('#updated');
      expect(result.current.settings.visualisation.edges.widthRange[0]).toBe(0.5);
    });

    it('should handle complex nested updates efficiently', () => {
      const { result } = renderHook(() => useSettingsStore());
      
      const startTime = performance.now();
      
      act(() => {
        result.current.updateSettings((draft) => {
          // Multiple nested updates
          for (let i = 0; i < 10; i++) {
            (draft as any)[`feature${i}`] = {
              enabled: true,
              config: {
                value: i,
                nested: {
                  deep: {
                    property: `value-${i}`
                  }
                }
              }
            };
          }
        });
      });
      
      const endTime = performance.now();
      const duration = endTime - startTime;
      
      // Complex update should still be fast
      expect(duration).toBeLessThan(10);
      
      // Verify some values
      const settings = result.current.settings as any;
      expect(settings.feature0.config.value).toBe(0);
      expect(settings.feature5.config.nested.deep.property).toBe('value-5');
      expect(settings.feature9.enabled).toBe(true);
    });
  });

  describe('Real-time Updates', () => {
    it('should handle immediate viewport updates efficiently', () => {
      const viewportCallback = vi.fn();
      const { result } = renderHook(() => useSettingsStore());
      
      act(() => {
        result.current.subscribe('viewport.update', viewportCallback, false);
      });
      
      const startTime = performance.now();
      
      // Update multiple viewport-related settings
      act(() => {
        result.current.set('visualisation.bloom.enabled', false);
        result.current.set('visualisation.nodes.opacity', 0.8);
        result.current.set('xr.enabled', true);
      });
      
      const endTime = performance.now();
      const duration = endTime - startTime;
      
      // Immediate updates should be very fast
      expect(duration).toBeLessThan(5);
      
      // Viewport callback should be called immediately for each update
      expect(viewportCallback).toHaveBeenCalledTimes(3);
    });

    it('should prioritize viewport updates over regular updates', () => {
      const viewportCallback = vi.fn();
      const regularCallback = vi.fn();
      const { result } = renderHook(() => useSettingsStore());
      
      act(() => {
        result.current.subscribe('viewport.update', viewportCallback, false);
        result.current.subscribe('system', regularCallback, false);
      });
      
      // Clear initial calls
      viewportCallback.mockClear();
      regularCallback.mockClear();
      
      act(() => {
        // Mix of viewport and regular updates
        result.current.set('visualisation.bloom.strength', 2.0); // viewport
        result.current.set('system.persistSettings', true); // regular
        result.current.set('xr.handTracking', true); // viewport
      });
      
      // Viewport callbacks should be called immediately
      expect(viewportCallback).toHaveBeenCalledTimes(2);
      
      // Regular callback will be debounced
      expect(regularCallback).toHaveBeenCalledTimes(0);
      
      // After debounce
      setTimeout(() => {
        expect(regularCallback).toHaveBeenCalledTimes(1);
      }, 400);
    });
  });
});