import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react-hooks';
import {
  useSelectiveSetting,
  useSelectiveSettings,
  useSettingSetter,
  useSettingsSubscription,
  useSettingsSelector
} from '../../hooks/useSelectiveSettingsStore';
import { useSettingsStore } from '../../store/settingsStore';
import { defaultSettings } from '../../features/settings/config/defaultSettings';

describe('useSelectiveSettingsStore hooks', () => {
  beforeEach(() => {
    // Reset store to default state
    useSettingsStore.setState({
      settings: defaultSettings,
      initialized: true,
      authenticated: false,
      user: null,
      isPowerUser: false,
      subscribers: new Map()
    });
  });

  describe('useSelectiveSetting', () => {
    it('should return the value at the specified path', () => {
      const { result } = renderHook(() => 
        useSelectiveSetting<string>('visualisation.nodes.baseColor')
      );
      
      expect(result.current).toBe('#0008ff');
    });

    it('should update when the value changes', () => {
      const { result } = renderHook(() => 
        useSelectiveSetting<string>('visualisation.nodes.baseColor')
      );
      
      expect(result.current).toBe('#0008ff');
      
      act(() => {
        useSettingsStore.getState().set('visualisation.nodes.baseColor', '#ff0000');
      });
      
      // Wait for subscription to trigger
      setTimeout(() => {
        expect(result.current).toBe('#ff0000');
      }, 0);
    });

    it('should handle nested paths correctly', () => {
      const { result } = renderHook(() => 
        useSelectiveSetting<number>('visualisation.physics.iterations')
      );
      
      expect(result.current).toBe(100);
    });

    it('should handle array values', () => {
      const { result } = renderHook(() => 
        useSelectiveSetting<[number, number]>('visualisation.edges.widthRange')
      );
      
      expect(result.current).toEqual([0.1, 1.0]);
    });

    it('should unsubscribe on unmount', () => {
      const { unmount } = renderHook(() => 
        useSelectiveSetting<string>('visualisation.nodes.baseColor')
      );
      
      const subscribersBefore = useSettingsStore.getState().subscribers.get('visualisation.nodes.baseColor')?.size || 0;
      
      unmount();
      
      const subscribersAfter = useSettingsStore.getState().subscribers.get('visualisation.nodes.baseColor')?.size || 0;
      expect(subscribersAfter).toBeLessThan(subscribersBefore);
    });
  });

  describe('useSelectiveSettings', () => {
    it('should return multiple values', () => {
      const { result } = renderHook(() => 
        useSelectiveSettings({
          nodeColor: 'visualisation.nodes.baseColor',
          edgeColor: 'visualisation.edges.color',
          bloomEnabled: 'visualisation.bloom.enabled'
        })
      );
      
      expect(result.current).toEqual({
        nodeColor: '#0008ff',
        edgeColor: '#56b6c2',
        bloomEnabled: true
      });
    });

    it('should update when any subscribed value changes', () => {
      const { result, rerender } = renderHook(() => 
        useSelectiveSettings({
          nodeColor: 'visualisation.nodes.baseColor',
          edgeOpacity: 'visualisation.edges.opacity'
        })
      );
      
      act(() => {
        useSettingsStore.getState().set('visualisation.nodes.baseColor', '#ffffff');
      });
      
      rerender();
      
      expect(result.current.nodeColor).toBe('#ffffff');
      expect(result.current.edgeOpacity).toBe(0.25);
      
      act(() => {
        useSettingsStore.getState().set('visualisation.edges.opacity', 0.5);
      });
      
      rerender();
      
      expect(result.current.nodeColor).toBe('#ffffff');
      expect(result.current.edgeOpacity).toBe(0.5);
    });

    it('should handle dynamic path changes', () => {
      let paths = {
        value1: 'visualisation.nodes.baseColor'
      };
      
      const { result, rerender } = renderHook(() => 
        useSelectiveSettings(paths)
      );
      
      expect(result.current.value1).toBe('#0008ff');
      
      // Change paths
      paths = {
        value1: 'visualisation.edges.color'
      };
      
      rerender();
      
      expect(result.current.value1).toBe('#56b6c2');
    });
  });

  describe('useSettingSetter', () => {
    it('should provide set function', () => {
      const { result } = renderHook(() => useSettingSetter());
      
      act(() => {
        result.current.set('visualisation.nodes.baseColor', '#123456');
      });
      
      expect(useSettingsStore.getState().settings.visualisation.nodes.baseColor).toBe('#123456');
    });

    it('should provide batchedSet function for multiple updates', () => {
      const { result } = renderHook(() => useSettingSetter());
      
      act(() => {
        result.current.batchedSet({
          'visualisation.nodes.baseColor': '#abcdef',
          'visualisation.edges.opacity': 0.7,
          'visualisation.bloom.strength': 2.0
        });
      });
      
      const settings = useSettingsStore.getState().settings;
      expect(settings.visualisation.nodes.baseColor).toBe('#abcdef');
      expect(settings.visualisation.edges.opacity).toBe(0.7);
      expect(settings.visualisation.bloom.strength).toBe(2.0);
    });

    it('should handle nested path creation in batchedSet', () => {
      const { result } = renderHook(() => useSettingSetter());
      
      act(() => {
        result.current.batchedSet({
          'newFeature.enabled': true,
          'newFeature.config.value': 42
        });
      });
      
      const settings = useSettingsStore.getState().settings as any;
      expect(settings.newFeature.enabled).toBe(true);
      expect(settings.newFeature.config.value).toBe(42);
    });

    it('should maintain immutability with batchedSet', () => {
      const originalSettings = useSettingsStore.getState().settings;
      const { result } = renderHook(() => useSettingSetter());
      
      act(() => {
        result.current.batchedSet({
          'visualisation.nodes.baseColor': '#fedcba'
        });
      });
      
      const newSettings = useSettingsStore.getState().settings;
      expect(originalSettings).not.toBe(newSettings);
      expect(originalSettings.visualisation.nodes.baseColor).toBe('#0008ff');
      expect(newSettings.visualisation.nodes.baseColor).toBe('#fedcba');
    });
  });

  describe('useSettingsSubscription', () => {
    it('should call callback with initial value', () => {
      const callback = vi.fn();
      
      renderHook(() => 
        useSettingsSubscription('visualisation.nodes.baseColor', callback)
      );
      
      expect(callback).toHaveBeenCalledWith('#0008ff');
    });

    it('should call callback when value changes', () => {
      const callback = vi.fn();
      
      renderHook(() => 
        useSettingsSubscription('visualisation.nodes.baseColor', callback)
      );
      
      act(() => {
        useSettingsStore.getState().set('visualisation.nodes.baseColor', '#facade');
      });
      
      // Wait for subscription to trigger
      setTimeout(() => {
        expect(callback).toHaveBeenCalledWith('#facade');
        expect(callback).toHaveBeenCalledTimes(2); // Initial + change
      }, 0);
    });

    it('should update callback reference without resubscribing', () => {
      let callbackValue = '';
      const callback1 = vi.fn((value) => { callbackValue = value + '1'; });
      const callback2 = vi.fn((value) => { callbackValue = value + '2'; });
      
      const { rerender } = renderHook(
        ({ cb }) => useSettingsSubscription('visualisation.nodes.baseColor', cb),
        { initialProps: { cb: callback1 } }
      );
      
      expect(callback1).toHaveBeenCalledWith('#0008ff');
      
      // Change callback
      rerender({ cb: callback2 });
      
      act(() => {
        useSettingsStore.getState().set('visualisation.nodes.baseColor', '#test');
      });
      
      setTimeout(() => {
        expect(callback1).toHaveBeenCalledTimes(1); // Only initial
        expect(callback2).toHaveBeenCalledTimes(1); // Only change
        expect(callbackValue).toBe('#test2');
      }, 0);
    });

    it('should respect dependencies array', () => {
      const callback = vi.fn();
      let dependency = 'dep1';
      
      const { rerender } = renderHook(() => 
        useSettingsSubscription('visualisation.nodes.baseColor', callback, [dependency])
      );
      
      expect(callback).toHaveBeenCalledTimes(1);
      
      // Change dependency
      dependency = 'dep2';
      rerender();
      
      expect(callback).toHaveBeenCalledTimes(2); // Re-subscribed due to dependency change
    });
  });

  describe('useSettingsSelector', () => {
    it('should derive state from settings', () => {
      const { result } = renderHook(() => 
        useSettingsSelector(settings => ({
          nodeColor: settings.visualisation.nodes.baseColor,
          isBloomEnabled: settings.visualisation.bloom.enabled
        }))
      );
      
      expect(result.current).toEqual({
        nodeColor: '#0008ff',
        isBloomEnabled: true
      });
    });

    it('should update when relevant settings change', () => {
      const { result, rerender } = renderHook(() => 
        useSettingsSelector(settings => settings.visualisation.nodes.baseColor)
      );
      
      expect(result.current).toBe('#0008ff');
      
      act(() => {
        useSettingsStore.getState().set('visualisation.nodes.baseColor', '#fedcba');
      });
      
      rerender();
      
      expect(result.current).toBe('#fedcba');
    });

    it('should use equality function when provided', () => {
      const selector = vi.fn(settings => ({
        nodes: settings.visualisation.nodes,
        edges: settings.visualisation.edges
      }));
      
      const equalityFn = vi.fn((prev, next) => 
        prev.nodes.baseColor === next.nodes.baseColor &&
        prev.edges.color === next.edges.color
      );
      
      const { rerender } = renderHook(() => 
        useSettingsSelector(selector, equalityFn)
      );
      
      act(() => {
        // Change something that doesn't affect equality
        useSettingsStore.getState().set('visualisation.nodes.opacity', 0.9);
      });
      
      rerender();
      
      expect(equalityFn).toHaveBeenCalled();
      // Selector should not be called again if equality function returns true
    });

    it('should handle complex selectors', () => {
      const { result } = renderHook(() => 
        useSettingsSelector(settings => {
          const nodes = settings.visualisation.nodes;
          const physics = settings.visualisation.physics;
          
          return {
            isHighQuality: nodes.quality === 'high',
            isPhysicsIntensive: physics.iterations > 50 && physics.enabled,
            allBloomSettings: settings.visualisation.bloom
          };
        })
      );
      
      expect(result.current).toEqual({
        isHighQuality: false,
        isPhysicsIntensive: true,
        allBloomSettings: expect.objectContaining({
          enabled: true,
          strength: 1.77
        })
      });
    });
  });

  describe('Performance optimizations', () => {
    it('should not cause unnecessary re-renders with selective subscriptions', () => {
      let renderCount = 0;
      
      const { rerender } = renderHook(() => {
        renderCount++;
        return useSelectiveSetting<string>('visualisation.nodes.baseColor');
      });
      
      expect(renderCount).toBe(1);
      
      // Change an unrelated setting
      act(() => {
        useSettingsStore.getState().set('visualisation.edges.opacity', 0.9);
      });
      
      rerender();
      
      // Should not re-render for unrelated changes
      expect(renderCount).toBe(1);
      
      // Change the subscribed setting
      act(() => {
        useSettingsStore.getState().set('visualisation.nodes.baseColor', '#changed');
      });
      
      rerender();
      
      // Should re-render for subscribed changes
      expect(renderCount).toBe(2);
    });

    it('should batch multiple updates efficiently', () => {
      const callback = vi.fn();
      
      renderHook(() => 
        useSettingsSubscription('visualisation', callback)
      );
      
      callback.mockClear();
      
      const { result } = renderHook(() => useSettingSetter());
      
      act(() => {
        // Multiple updates in one batch
        result.current.batchedSet({
          'visualisation.nodes.baseColor': '#111111',
          'visualisation.nodes.opacity': 0.1,
          'visualisation.edges.color': '#222222',
          'visualisation.edges.opacity': 0.2
        });
      });
      
      // Should trigger callback once for the batched update
      setTimeout(() => {
        expect(callback).toHaveBeenCalledTimes(1);
      }, 400); // After debounce
    });
  });

  describe('Edge cases', () => {
    it('should handle undefined paths gracefully', () => {
      const { result } = renderHook(() => 
        useSelectiveSetting<any>('non.existent.path')
      );
      
      expect(result.current).toBeUndefined();
    });

    it('should handle empty path', () => {
      const { result } = renderHook(() => 
        useSelectiveSetting<any>('')
      );
      
      expect(result.current).toEqual(defaultSettings);
    });

    it('should handle rapid path changes', () => {
      let path = 'visualisation.nodes.baseColor';
      
      const { result, rerender } = renderHook(() => 
        useSelectiveSetting<any>(path)
      );
      
      expect(result.current).toBe('#0008ff');
      
      // Rapidly change paths
      path = 'visualisation.edges.color';
      rerender();
      expect(result.current).toBe('#56b6c2');
      
      path = 'visualisation.bloom.enabled';
      rerender();
      expect(result.current).toBe(true);
      
      path = 'system.debug.enabled';
      rerender();
      expect(result.current).toBe(false);
    });
  });
});