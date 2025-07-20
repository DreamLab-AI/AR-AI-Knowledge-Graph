import React from 'react';
import { createLogger } from './logger';

const logger = createLogger('PerformanceMonitor');

interface PerformanceMetrics {
  renderCount: number;
  renderTime: number;
  lastRenderTimestamp: number;
  averageRenderTime: number;
  peakRenderTime: number;
}

class PerformanceMonitor {
  private metrics: Map<string, PerformanceMetrics> = new Map();
  private enabled: boolean = process.env.NODE_ENV === 'development';

  /**
   * Start measuring performance for a component
   */
  startMeasure(componentName: string): () => void {
    if (!this.enabled) return () => {};
    
    const startTime = performance.now();
    
    return () => {
      const endTime = performance.now();
      const renderTime = endTime - startTime;
      
      this.updateMetrics(componentName, renderTime);
    };
  }

  /**
   * Update metrics for a component
   */
  private updateMetrics(componentName: string, renderTime: number): void {
    const existing = this.metrics.get(componentName) || {
      renderCount: 0,
      renderTime: 0,
      lastRenderTimestamp: 0,
      averageRenderTime: 0,
      peakRenderTime: 0
    };
    
    const newCount = existing.renderCount + 1;
    const totalTime = existing.renderTime + renderTime;
    const averageTime = totalTime / newCount;
    const peakTime = Math.max(existing.peakRenderTime, renderTime);
    
    const updated: PerformanceMetrics = {
      renderCount: newCount,
      renderTime: totalTime,
      lastRenderTimestamp: Date.now(),
      averageRenderTime: averageTime,
      peakRenderTime: peakTime
    };
    
    this.metrics.set(componentName, updated);
    
    // Log slow renders
    if (renderTime > 16.67) { // More than one frame (60fps)
      logger.warn(`Slow render detected in ${componentName}: ${renderTime.toFixed(2)}ms`);
    }
  }

  /**
   * Get metrics for a specific component
   */
  getMetrics(componentName: string): PerformanceMetrics | undefined {
    return this.metrics.get(componentName);
  }

  /**
   * Get all metrics
   */
  getAllMetrics(): Map<string, PerformanceMetrics> {
    return new Map(this.metrics);
  }

  /**
   * Clear metrics for a component
   */
  clearMetrics(componentName?: string): void {
    if (componentName) {
      this.metrics.delete(componentName);
    } else {
      this.metrics.clear();
    }
  }

  /**
   * Generate a performance report
   */
  generateReport(): string {
    const sortedMetrics = Array.from(this.metrics.entries())
      .sort((a, b) => b[1].averageRenderTime - a[1].averageRenderTime);
    
    let report = '\\n=== Performance Report ===\\n';
    
    for (const [component, metrics] of sortedMetrics) {
      report += `\\n${component}:\\n`;
      report += `  Render Count: ${metrics.renderCount}\\n`;
      report += `  Average Time: ${metrics.averageRenderTime.toFixed(2)}ms\\n`;
      report += `  Peak Time: ${metrics.peakRenderTime.toFixed(2)}ms\\n`;
      report += `  Total Time: ${metrics.renderTime.toFixed(2)}ms\\n`;
    }
    
    return report;
  }

  /**
   * Log the performance report
   */
  logReport(): void {
    if (!this.enabled) return;
    logger.info(this.generateReport());
  }

  /**
   * Enable or disable performance monitoring
   */
  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
  }
}

// Singleton instance
export const performanceMonitor = new PerformanceMonitor();

/**
 * React hook for performance monitoring
 */
export function usePerformanceMonitor(componentName: string): void {
  if (process.env.NODE_ENV !== 'development') return;
  
  const endMeasure = performanceMonitor.startMeasure(componentName);
  
  // Measure after render
  setTimeout(endMeasure, 0);
}

/**
 * HOC for performance monitoring
 */
export function withPerformanceMonitor<P extends object>(
  Component: React.ComponentType<P>,
  componentName?: string
): React.ComponentType<P> {
  const displayName = componentName || Component.displayName || Component.name || 'Unknown';
  
  const WrappedComponent = (props: P) => {
    usePerformanceMonitor(displayName);
    return <Component {...props} />;
  };
  
  WrappedComponent.displayName = `withPerformanceMonitor(${displayName})`;
  
  return WrappedComponent;
}

/**
 * Custom hook for render counting
 */
export function useRenderCount(componentName: string): number {
  const [renderCount, setRenderCount] = React.useState(0);
  
  React.useEffect(() => {
    setRenderCount(prev => prev + 1);
    
    if (process.env.NODE_ENV === 'development') {
      logger.debug(`${componentName} rendered ${renderCount + 1} times`);
    }
  });
  
  return renderCount;
}

// Export a global reference for console debugging
if (typeof window !== 'undefined' && process.env.NODE_ENV === 'development') {
  (window as any).performanceMonitor = performanceMonitor;
}