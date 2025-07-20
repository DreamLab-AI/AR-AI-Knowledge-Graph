/**
 * Performance monitoring overlay component for dual graph visualization
 * Shows real-time FPS, memory, and graph metrics
 */

import React, { useEffect, useState, useRef } from 'react';
import { useFrame, useThree } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import { dualGraphPerformanceMonitor, type PerformanceMetrics } from '../../utils/dualGraphPerformanceMonitor';
import { useSettingsStore } from '../../store/settingsStore';
import { debugState } from '../../utils/debugState';

interface PerformanceOverlayProps {
  position?: [number, number, number];
  logseqNodeCount?: number;
  logseqEdgeCount?: number;
  visionflowNodeCount?: number;
  visionflowEdgeCount?: number;
}

export const PerformanceOverlay: React.FC<PerformanceOverlayProps> = ({
  position = [-20, 15, 0],
  logseqNodeCount = 0,
  logseqEdgeCount = 0,
  visionflowNodeCount = 0,
  visionflowEdgeCount = 0
}) => {
  const [metrics, setMetrics] = useState<PerformanceMetrics | null>(null);
  const [isMinimized, setIsMinimized] = useState(false);
  const [showDetails, setShowDetails] = useState(false);
  const { gl } = useThree();
  const settings = useSettingsStore(state => state.settings);
  const updateInterval = useRef<number>(0);
  
  // Initialize performance monitor with WebGL context
  useEffect(() => {
    dualGraphPerformanceMonitor.initializeWebGL(gl);
    return () => {
      dualGraphPerformanceMonitor.dispose();
    };
  }, [gl]);

  // Update graph metrics when counts change
  useEffect(() => {
    dualGraphPerformanceMonitor.updateGraphMetrics('logseq', {
      nodeCount: logseqNodeCount,
      edgeCount: logseqEdgeCount,
      instancedRendering: true // We know Logseq uses instanced rendering
    });
  }, [logseqNodeCount, logseqEdgeCount]);

  useEffect(() => {
    dualGraphPerformanceMonitor.updateGraphMetrics('visionflow', {
      nodeCount: visionflowNodeCount,
      edgeCount: visionflowEdgeCount,
      instancedRendering: visionflowNodeCount > 50 // VisionFlow uses instancing for >50 nodes
    });
  }, [visionflowNodeCount, visionflowEdgeCount]);

  // Performance tracking in render loop
  useFrame((state, delta) => {
    // Track frame performance
    dualGraphPerformanceMonitor.beginFrame();
    
    // Update metrics periodically (every 30 frames)
    updateInterval.current++;
    if (updateInterval.current >= 30) {
      updateInterval.current = 0;
      dualGraphPerformanceMonitor.endFrame(gl);
      setMetrics(dualGraphPerformanceMonitor.getMetrics());
    } else {
      dualGraphPerformanceMonitor.endFrame();
    }
  });

  // Don't render if debug is disabled or settings hide it
  if (!debugState.isEnabled() || settings?.visualisation?.performance?.hideOverlay) {
    return null;
  }

  const score = metrics ? dualGraphPerformanceMonitor.getPerformanceScore() : 0;
  const scoreColor = score >= 80 ? '#2ECC71' : score >= 60 ? '#F1C40F' : '#E74C3C';

  return (
    <Html position={position} style={{ pointerEvents: 'auto' }}>
      <div
        style={{
          background: 'rgba(0, 0, 0, 0.9)',
          border: '2px solid #333',
          borderRadius: '8px',
          padding: isMinimized ? '5px 10px' : '10px',
          fontFamily: 'monospace',
          fontSize: '12px',
          color: '#fff',
          minWidth: isMinimized ? 'auto' : '300px',
          userSelect: 'none',
          cursor: 'move'
        }}
        onClick={() => setIsMinimized(!isMinimized)}
      >
        {isMinimized ? (
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <span style={{ color: scoreColor }}>⚡ {metrics?.fps || 0} FPS</span>
            <span>📊 Score: {score}/100</span>
          </div>
        ) : (
          <>
            <div style={{ 
              display: 'flex', 
              justifyContent: 'space-between', 
              alignItems: 'center',
              marginBottom: '10px',
              borderBottom: '1px solid #444',
              paddingBottom: '5px'
            }}>
              <h3 style={{ margin: 0, fontSize: '14px' }}>⚡ Performance Monitor</h3>
              <div style={{ display: 'flex', gap: '10px' }}>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setShowDetails(!showDetails);
                  }}
                  style={{
                    background: 'transparent',
                    border: '1px solid #666',
                    color: '#fff',
                    borderRadius: '4px',
                    padding: '2px 8px',
                    cursor: 'pointer',
                    fontSize: '11px'
                  }}
                >
                  {showDetails ? 'Simple' : 'Details'}
                </button>
                <span style={{ color: scoreColor, fontWeight: 'bold' }}>
                  Score: {score}/100
                </span>
              </div>
            </div>

            {metrics && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '5px' }}>
                {/* Basic Metrics */}
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px' }}>
                  <div>
                    <div style={{ color: '#888', fontSize: '10px' }}>FPS</div>
                    <div style={{ 
                      fontSize: '20px', 
                      fontWeight: 'bold',
                      color: metrics.fps >= 50 ? '#2ECC71' : metrics.fps >= 30 ? '#F1C40F' : '#E74C3C'
                    }}>
                      {metrics.fps}
                    </div>
                  </div>
                  <div>
                    <div style={{ color: '#888', fontSize: '10px' }}>Frame Time</div>
                    <div style={{ fontSize: '16px' }}>
                      {metrics.frameTime.toFixed(1)}ms
                    </div>
                    <div style={{ fontSize: '10px', color: '#666' }}>
                      ({metrics.frameTimeMin.toFixed(1)}-{metrics.frameTimeMax.toFixed(1)})
                    </div>
                  </div>
                </div>

                {/* Memory Bar */}
                <div>
                  <div style={{ color: '#888', fontSize: '10px', marginBottom: '2px' }}>
                    Memory: {metrics.memory.used}MB / {metrics.memory.limit}MB
                  </div>
                  <div style={{ 
                    background: '#333', 
                    height: '6px', 
                    borderRadius: '3px',
                    overflow: 'hidden'
                  }}>
                    <div style={{
                      background: metrics.memory.percent > 80 ? '#E74C3C' : '#2ECC71',
                      width: `${metrics.memory.percent}%`,
                      height: '100%',
                      transition: 'width 0.3s'
                    }} />
                  </div>
                </div>

                {/* Graph Stats */}
                <div style={{ 
                  display: 'grid', 
                  gridTemplateColumns: '1fr 1fr', 
                  gap: '10px',
                  marginTop: '5px',
                  paddingTop: '5px',
                  borderTop: '1px solid #444'
                }}>
                  <div>
                    <div style={{ color: '#00CED1', fontSize: '11px', fontWeight: 'bold' }}>
                      Logseq Graph
                    </div>
                    <div style={{ fontSize: '10px' }}>
                      {metrics.graphMetrics.logseq.nodeCount} nodes, {metrics.graphMetrics.logseq.edgeCount} edges
                    </div>
                    {metrics.graphMetrics.logseq.instancedRendering && (
                      <div style={{ fontSize: '10px', color: '#2ECC71' }}>✓ Instanced</div>
                    )}
                  </div>
                  <div>
                    <div style={{ color: '#F1C40F', fontSize: '11px', fontWeight: 'bold' }}>
                      VisionFlow Graph
                    </div>
                    <div style={{ fontSize: '10px' }}>
                      {metrics.graphMetrics.visionflow.nodeCount} nodes, {metrics.graphMetrics.visionflow.edgeCount} edges
                    </div>
                    {metrics.graphMetrics.visionflow.instancedRendering && (
                      <div style={{ fontSize: '10px', color: '#2ECC71' }}>✓ Instanced</div>
                    )}
                  </div>
                </div>

                {/* Detailed Stats */}
                {showDetails && (
                  <div style={{
                    marginTop: '10px',
                    paddingTop: '10px',
                    borderTop: '1px solid #444',
                    fontSize: '10px'
                  }}>
                    <div style={{ marginBottom: '5px', fontWeight: 'bold' }}>WebGL Stats:</div>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '5px' }}>
                      <div>Draw Calls: {metrics.webgl.drawCalls}</div>
                      <div>Triangles: {metrics.webgl.triangles.toLocaleString()}</div>
                      <div>Programs: {metrics.webgl.programs}</div>
                      <div>Textures: {metrics.webgl.textures}</div>
                      <div>Geometries: {metrics.webgl.geometries}</div>
                      <div>Points: {metrics.webgl.points}</div>
                    </div>
                    
                    <div style={{ marginTop: '10px' }}>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          dualGraphPerformanceMonitor.logReport();
                        }}
                        style={{
                          background: '#444',
                          border: 'none',
                          color: '#fff',
                          borderRadius: '4px',
                          padding: '5px 10px',
                          cursor: 'pointer',
                          fontSize: '11px',
                          width: '100%'
                        }}
                      >
                        Log Detailed Report to Console
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </div>
    </Html>
  );
};