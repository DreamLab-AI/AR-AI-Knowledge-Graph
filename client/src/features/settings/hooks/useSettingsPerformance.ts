import { useEffect, useRef, useCallback } from 'react';
import { useSettingsStore } from '@/store/settingsStore';

interface PerformanceMetrics {
  renderCount: number;
  lastRenderTime: number;
  averageRenderTime: number;
  peakRenderTime: number;
  memoryUsage?: number;
  settingChanges: number;
  searchTime?: number;
}

interface UseSettingsPerformanceOptions {
  enableLogging?: boolean;
  enableMemoryTracking?: boolean;
  sampleRate?: number; // Sample every N renders
}

export function useSettingsPerformance(
  componentName: string,
  options: UseSettingsPerformanceOptions = {}
) {
  const {
    enableLogging = false,
    enableMemoryTracking = false,
    sampleRate = 10,
  } = options;

  const metricsRef = useRef<PerformanceMetrics>({
    renderCount: 0,
    lastRenderTime: 0,
    averageRenderTime: 0,
    peakRenderTime: 0,
    settingChanges: 0,
  });

  const renderStartRef = useRef<number>(0);
  const renderTimesRef = useRef<number[]>([]);

  // Track render start
  useEffect(() => {
    renderStartRef.current = performance.now();
  });

  // Track render completion
  useEffect(() => {
    const renderTime = performance.now() - renderStartRef.current;
    const metrics = metricsRef.current;

    metrics.renderCount++;
    metrics.lastRenderTime = renderTime;

    // Update peak time
    if (renderTime > metrics.peakRenderTime) {
      metrics.peakRenderTime = renderTime;
    }

    // Update average (keep last 100 samples)
    renderTimesRef.current.push(renderTime);
    if (renderTimesRef.current.length > 100) {
      renderTimesRef.current.shift();
    }
    metrics.averageRenderTime =
      renderTimesRef.current.reduce((a, b) => a + b, 0) / renderTimesRef.current.length;

    // Memory tracking (if enabled and available)
    if (enableMemoryTracking && 'memory' in performance) {
      const memoryInfo = (performance as any).memory;
      metrics.memoryUsage = memoryInfo.usedJSHeapSize / 1048576; // Convert to MB
    }

    // Log metrics periodically
    if (enableLogging && metrics.renderCount % sampleRate === 0) {
      console.log(`[${componentName}] Performance Metrics:`, {
        ...metrics,
        averageRenderTime: `${metrics.averageRenderTime.toFixed(2)}ms`,
        lastRenderTime: `${metrics.lastRenderTime.toFixed(2)}ms`,
        peakRenderTime: `${metrics.peakRenderTime.toFixed(2)}ms`,
        memoryUsage: metrics.memoryUsage ? `${metrics.memoryUsage.toFixed(2)}MB` : 'N/A',
      });
    }
  });

  // Track setting changes
  useEffect(() => {
    const unsubscribe = useSettingsStore.subscribe(() => {
      metricsRef.current.settingChanges++;
    });
    return unsubscribe;
  }, []);

  // Measure search performance
  const measureSearch = useCallback((searchFn: () => void) => {
    const start = performance.now();
    searchFn();
    const searchTime = performance.now() - start;
    metricsRef.current.searchTime = searchTime;

    if (enableLogging) {
      console.log(`[${componentName}] Search completed in ${searchTime.toFixed(2)}ms`);
    }
  }, [componentName, enableLogging]);

  // Get current metrics
  const getMetrics = useCallback((): PerformanceMetrics => {
    return { ...metricsRef.current };
  }, []);

  // Reset metrics
  const resetMetrics = useCallback(() => {
    metricsRef.current = {
      renderCount: 0,
      lastRenderTime: 0,
      averageRenderTime: 0,
      peakRenderTime: 0,
      settingChanges: 0,
    };
    renderTimesRef.current = [];
  }, []);

  // Performance warnings
  useEffect(() => {
    const checkInterval = setInterval(() => {
      const metrics = metricsRef.current;

      // Warn if average render time is high
      if (metrics.averageRenderTime > 16.67 && enableLogging) {
        console.warn(
          `[${componentName}] Average render time (${metrics.averageRenderTime.toFixed(
            2
          )}ms) exceeds 60fps threshold`
        );
      }

      // Warn if memory usage is high
      if (metrics.memoryUsage && metrics.memoryUsage > 500 && enableLogging) {
        console.warn(
          `[${componentName}] High memory usage: ${metrics.memoryUsage.toFixed(2)}MB`
        );
      }
    }, 5000); // Check every 5 seconds

    return () => clearInterval(checkInterval);
  }, [componentName, enableLogging]);

  return {
    measureSearch,
    getMetrics,
    resetMetrics,
  };
}

// Performance optimization utilities
export const performanceUtils = {
  // Debounce function optimized for search
  debounceSearch: (fn: (...args: any[]) => void, delay: number = 300) => {
    let timeoutId: NodeJS.Timeout;
    return (...args: any[]) => {
      clearTimeout(timeoutId);
      timeoutId = setTimeout(() => fn(...args), delay);
    };
  },

  // Throttle function for scroll events
  throttleScroll: (fn: (...args: any[]) => void, limit: number = 100) => {
    let inThrottle: boolean;
    return (...args: any[]) => {
      if (!inThrottle) {
        fn(...args);
        inThrottle = true;
        setTimeout(() => (inThrottle = false), limit);
      }
    };
  },

  // Memoize expensive computations
  memoizeComputation: <T extends (...args: any[]) => any>(
    fn: T,
    cacheSize: number = 10
  ): T => {
    const cache = new Map<string, ReturnType<T>>();
    return ((...args: Parameters<T>) => {
      const key = JSON.stringify(args);
      if (cache.has(key)) {
        return cache.get(key)!;
      }
      const result = fn(...args);
      cache.set(key, result);
      // Limit cache size
      if (cache.size > cacheSize) {
        const firstKey = cache.keys().next().value;
        cache.delete(firstKey);
      }
      return result;
    }) as T;
  },
};