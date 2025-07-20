/**
 * Performance monitoring integration for graph components
 * Adds performance tracking hooks to existing graph managers
 */

import { useEffect } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import { dualGraphPerformanceMonitor } from '../../../utils/dualGraphPerformanceMonitor';
import { dualGraphOptimizer } from '../../../utils/dualGraphOptimizations';
import { debugState } from '../../../utils/debugState';

interface PerformanceIntegrationProps {
  logseqNodeCount: number;
  logseqEdgeCount: number;
  visionflowNodeCount: number;
  visionflowEdgeCount: number;
  onPerformanceUpdate?: (metrics: any) => void;
}

export const PerformanceIntegration: React.FC<PerformanceIntegrationProps> = ({
  logseqNodeCount,
  logseqEdgeCount,
  visionflowNodeCount,
  visionflowEdgeCount,
  onPerformanceUpdate
}) => {
  const { camera, gl } = useThree();

  // Initialize performance monitoring
  useEffect(() => {
    if (debugState.isEnabled()) {
      dualGraphPerformanceMonitor.initializeWebGL(gl);
      dualGraphOptimizer.initializeOptimizations(camera, gl);
    }
  }, [camera, gl]);

  // Update graph metrics when counts change
  useEffect(() => {
    dualGraphPerformanceMonitor.mark('logseq-update');
    dualGraphPerformanceMonitor.updateGraphMetrics('logseq', {
      nodeCount: logseqNodeCount,
      edgeCount: logseqEdgeCount,
      updateTime: dualGraphPerformanceMonitor.measure('logseq-update'),
      instancedRendering: true
    });
  }, [logseqNodeCount, logseqEdgeCount]);

  useEffect(() => {
    dualGraphPerformanceMonitor.mark('visionflow-update');
    dualGraphPerformanceMonitor.updateGraphMetrics('visionflow', {
      nodeCount: visionflowNodeCount,
      edgeCount: visionflowEdgeCount,
      updateTime: dualGraphPerformanceMonitor.measure('visionflow-update'),
      instancedRendering: visionflowNodeCount > 50
    });
  }, [visionflowNodeCount, visionflowEdgeCount]);

  // Performance tracking in render loop
  useFrame((state, delta) => {
    if (!debugState.isEnabled()) return;

    // Track frame performance
    dualGraphPerformanceMonitor.beginFrame();
    
    // Optimize frame calculations
    dualGraphOptimizer.optimizeFrame(camera);
    
    // End frame and update metrics
    dualGraphPerformanceMonitor.endFrame(gl);
    
    // Call performance update callback if provided
    if (onPerformanceUpdate) {
      const metrics = dualGraphPerformanceMonitor.getMetrics();
      onPerformanceUpdate(metrics);
    }
  });

  return null; // This is a utility component with no visual output
};