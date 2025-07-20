/**
 * Performance testing component for dual graph visualization
 * Tests with various node counts and scenarios to identify bottlenecks
 */

import React, { useState, useEffect, useRef } from 'react';
import { Canvas } from '@react-three/fiber';
import { dualGraphPerformanceMonitor } from '../../utils/dualGraphPerformanceMonitor';
import { PerformanceOverlay } from '../../components/performance/PerformanceOverlay';
import EnhancedGraphManager from '../../features/graph/components/EnhancedGraphManager';
import SwarmVisualizationEnhanced from '../../features/swarm/components/SwarmVisualizationEnhanced';
import { GraphData } from '../../features/graph/managers/graphDataManager';
import { createLogger } from '../../utils/logger';

const logger = createLogger('DualGraphPerformanceTest');

interface TestScenario {
  name: string;
  logseqNodes: number;
  logseqEdges: number;
  visionflowNodes: number;
  visionflowEdges: number;
  duration: number; // Test duration in seconds
}

const TEST_SCENARIOS: TestScenario[] = [
  { name: 'Small Scale', logseqNodes: 50, logseqEdges: 75, visionflowNodes: 20, visionflowEdges: 30, duration: 10 },
  { name: 'Medium Scale', logseqNodes: 200, logseqEdges: 350, visionflowNodes: 100, visionflowEdges: 150, duration: 15 },
  { name: 'Large Scale', logseqNodes: 500, logseqEdges: 800, visionflowNodes: 200, visionflowEdges: 300, duration: 15 },
  { name: 'Very Large', logseqNodes: 1000, logseqEdges: 1500, visionflowNodes: 500, visionflowEdges: 750, duration: 20 },
  { name: 'Extreme Scale', logseqNodes: 2000, logseqEdges: 3000, visionflowNodes: 1000, visionflowEdges: 1500, duration: 30 },
];

interface TestResult {
  scenario: string;
  avgFps: number;
  minFps: number;
  maxFps: number;
  avgFrameTime: number;
  maxMemoryUsed: number;
  avgDrawCalls: number;
  performanceScore: number;
  recommendations: string[];
  timestamp: Date;
}

export const DualGraphPerformanceTest: React.FC = () => {
  const [currentScenario, setCurrentScenario] = useState<TestScenario | null>(null);
  const [testResults, setTestResults] = useState<TestResult[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [testProgress, setTestProgress] = useState(0);
  const [currentTestData, setCurrentTestData] = useState<{
    logseq: GraphData;
    visionflow: { nodeCount: number; edgeCount: number };
  } | null>(null);
  
  const testTimeoutRef = useRef<NodeJS.Timeout>();
  const metricsCollectionRef = useRef<{
    fps: number[];
    frameTimes: number[];
    memoryUsage: number[];
    drawCalls: number[];
  }>({ fps: [], frameTimes: [], memoryUsage: [], drawCalls: [] });

  // Generate test data for a scenario
  const generateTestData = (scenario: TestScenario) => {
    // Generate Logseq graph data
    const logseqNodes = Array.from({ length: scenario.logseqNodes }, (_, i) => ({
      id: `logseq-node-${i}`,
      label: `Node ${i}`,
      position: {
        x: (Math.random() - 0.5) * 40,
        y: (Math.random() - 0.5) * 40,
        z: (Math.random() - 0.5) * 40
      },
      metadata: {
        type: ['file', 'function', 'class', 'variable'][Math.floor(Math.random() * 4)],
        size: 0.5 + Math.random() * 1.5
      }
    }));

    const logseqEdges = Array.from({ length: scenario.logseqEdges }, (_, i) => {
      const sourceIndex = Math.floor(Math.random() * logseqNodes.length);
      let targetIndex = Math.floor(Math.random() * logseqNodes.length);
      while (targetIndex === sourceIndex) {
        targetIndex = Math.floor(Math.random() * logseqNodes.length);
      }
      
      return {
        id: `logseq-edge-${i}`,
        source: logseqNodes[sourceIndex].id,
        target: logseqNodes[targetIndex].id,
        weight: Math.random()
      };
    });

    return {
      logseq: { nodes: logseqNodes, edges: logseqEdges },
      visionflow: { 
        nodeCount: scenario.visionflowNodes, 
        edgeCount: scenario.visionflowEdges 
      }
    };
  };

  // Start a performance test
  const startTest = async (scenario: TestScenario) => {
    setIsRunning(true);
    setCurrentScenario(scenario);
    setTestProgress(0);
    
    // Reset performance monitor
    dualGraphPerformanceMonitor.reset();
    metricsCollectionRef.current = { fps: [], frameTimes: [], memoryUsage: [], drawCalls: [] };
    
    // Generate test data
    const testData = generateTestData(scenario);
    setCurrentTestData(testData);
    
    logger.info(`Starting performance test: ${scenario.name}`, {
      logseqNodes: scenario.logseqNodes,
      visionflowNodes: scenario.visionflowNodes,
      duration: scenario.duration
    });

    // Collect metrics during test
    const startTime = Date.now();
    const metricsInterval = setInterval(() => {
      const metrics = dualGraphPerformanceMonitor.getMetrics();
      metricsCollectionRef.current.fps.push(metrics.fps);
      metricsCollectionRef.current.frameTimes.push(metrics.frameTime);
      metricsCollectionRef.current.memoryUsage.push(metrics.memory.used);
      metricsCollectionRef.current.drawCalls.push(metrics.webgl.drawCalls);
      
      const elapsed = (Date.now() - startTime) / 1000;
      setTestProgress((elapsed / scenario.duration) * 100);
    }, 500);

    // End test after duration
    testTimeoutRef.current = setTimeout(() => {
      clearInterval(metricsInterval);
      completeTest(scenario);
    }, scenario.duration * 1000);
  };

  // Complete a test and collect results
  const completeTest = (scenario: TestScenario) => {
    const metrics = metricsCollectionRef.current;
    const performanceScore = dualGraphPerformanceMonitor.getPerformanceScore();
    
    const result: TestResult = {
      scenario: scenario.name,
      avgFps: metrics.fps.reduce((a, b) => a + b, 0) / metrics.fps.length || 0,
      minFps: Math.min(...metrics.fps) || 0,
      maxFps: Math.max(...metrics.fps) || 0,
      avgFrameTime: metrics.frameTimes.reduce((a, b) => a + b, 0) / metrics.frameTimes.length || 0,
      maxMemoryUsed: Math.max(...metrics.memoryUsage) || 0,
      avgDrawCalls: metrics.drawCalls.reduce((a, b) => a + b, 0) / metrics.drawCalls.length || 0,
      performanceScore,
      recommendations: [], // Will be filled by analysis
      timestamp: new Date()
    };

    // Add specific recommendations based on results
    if (result.avgFps < 30) {
      result.recommendations.push('Low FPS detected - consider reducing node count or enabling more aggressive culling');
    }
    if (result.maxMemoryUsed > 500) {
      result.recommendations.push('High memory usage - implement geometry disposal and pooling');
    }
    if (result.avgDrawCalls > 300) {
      result.recommendations.push('High draw calls - ensure instanced rendering is enabled for both graphs');
    }

    setTestResults(prev => [...prev, result]);
    setIsRunning(false);
    setCurrentScenario(null);
    setTestProgress(0);
    
    logger.info(`Performance test completed: ${scenario.name}`, result);
  };

  // Run all test scenarios
  const runAllTests = async () => {
    for (const scenario of TEST_SCENARIOS) {
      await new Promise<void>((resolve) => {
        startTest(scenario);
        
        const checkComplete = () => {
          if (!isRunning) {
            resolve();
          } else {
            setTimeout(checkComplete, 1000);
          }
        };
        
        setTimeout(checkComplete, scenario.duration * 1000 + 1000);
      });
      
      // Wait between tests
      await new Promise(resolve => setTimeout(resolve, 2000));
    }
  };

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (testTimeoutRef.current) {
        clearTimeout(testTimeoutRef.current);
      }
    };
  }, []);

  // Export test results
  const exportResults = () => {
    const data = {
      testResults,
      systemInfo: {
        userAgent: navigator.userAgent,
        timestamp: new Date().toISOString(),
        performanceSupported: 'performance' in window && 'memory' in performance
      }
    };
    
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `dual-graph-performance-test-${new Date().toISOString().split('T')[0]}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div style={{ width: '100vw', height: '100vh', position: 'relative' }}>
      {/* Test Canvas */}
      <Canvas camera={{ position: [0, 0, 50], fov: 75 }}>
        <ambientLight intensity={0.3} />
        <pointLight position={[10, 10, 10]} />
        
        {currentTestData && (
          <>
            {/* Performance Overlay */}
            <PerformanceOverlay
              position={[-25, 20, 0]}
              logseqNodeCount={currentTestData.logseq.nodes.length}
              logseqEdgeCount={currentTestData.logseq.edges.length}
              visionflowNodeCount={currentTestData.visionflow.nodeCount}
              visionflowEdgeCount={currentTestData.visionflow.edgeCount}
            />
            
            {/* Note: In a real test, you'd inject the test data into the graph components */}
            {/* This is a simplified example - actual implementation would need data injection */}
          </>
        )}
      </Canvas>

      {/* Test Control Panel */}
      <div style={{
        position: 'absolute',
        top: '20px',
        right: '20px',
        background: 'rgba(0, 0, 0, 0.9)',
        border: '2px solid #333',
        borderRadius: '8px',
        padding: '20px',
        fontFamily: 'monospace',
        color: '#fff',
        width: '350px',
        maxHeight: '80vh',
        overflowY: 'auto'
      }}>
        <h2 style={{ margin: '0 0 15px 0', fontSize: '18px' }}>
          🧪 Dual Graph Performance Test
        </h2>

        {/* Current Test Status */}
        {currentScenario && (
          <div style={{ marginBottom: '15px', padding: '10px', background: '#1a1a1a', borderRadius: '4px' }}>
            <div style={{ fontWeight: 'bold', marginBottom: '5px' }}>
              Running: {currentScenario.name}
            </div>
            <div style={{ fontSize: '12px', color: '#888' }}>
              Logseq: {currentScenario.logseqNodes} nodes, {currentScenario.logseqEdges} edges<br/>
              VisionFlow: {currentScenario.visionflowNodes} nodes, {currentScenario.visionflowEdges} edges
            </div>
            <div style={{ marginTop: '10px' }}>
              <div style={{ 
                background: '#333', 
                height: '6px', 
                borderRadius: '3px',
                overflow: 'hidden'
              }}>
                <div style={{
                  background: '#2ECC71',
                  width: `${testProgress}%`,
                  height: '100%',
                  transition: 'width 0.3s'
                }} />
              </div>
              <div style={{ fontSize: '12px', marginTop: '5px' }}>
                Progress: {testProgress.toFixed(1)}%
              </div>
            </div>
          </div>
        )}

        {/* Test Scenarios */}
        <div style={{ marginBottom: '15px' }}>
          <h3 style={{ margin: '0 0 10px 0', fontSize: '14px' }}>Test Scenarios:</h3>
          {TEST_SCENARIOS.map((scenario, index) => (
            <button
              key={scenario.name}
              onClick={() => startTest(scenario)}
              disabled={isRunning}
              style={{
                width: '100%',
                margin: '2px 0',
                padding: '8px',
                background: isRunning ? '#555' : '#444',
                border: '1px solid #666',
                borderRadius: '4px',
                color: '#fff',
                cursor: isRunning ? 'not-allowed' : 'pointer',
                fontSize: '12px',
                textAlign: 'left'
              }}
            >
              <div style={{ fontWeight: 'bold' }}>{scenario.name}</div>
              <div style={{ fontSize: '10px', color: '#aaa' }}>
                Total: {scenario.logseqNodes + scenario.visionflowNodes} nodes, {scenario.duration}s
              </div>
            </button>
          ))}
        </div>

        {/* Run All Tests */}
        <button
          onClick={runAllTests}
          disabled={isRunning}
          style={{
            width: '100%',
            padding: '10px',
            background: isRunning ? '#555' : '#2ECC71',
            border: 'none',
            borderRadius: '4px',
            color: '#fff',
            cursor: isRunning ? 'not-allowed' : 'pointer',
            fontSize: '14px',
            fontWeight: 'bold',
            marginBottom: '15px'
          }}
        >
          {isRunning ? 'Test Running...' : 'Run All Tests'}
        </button>

        {/* Test Results */}
        {testResults.length > 0 && (
          <div>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
              <h3 style={{ margin: 0, fontSize: '14px' }}>Results:</h3>
              <button
                onClick={exportResults}
                style={{
                  padding: '4px 8px',
                  background: '#666',
                  border: 'none',
                  borderRadius: '4px',
                  color: '#fff',
                  cursor: 'pointer',
                  fontSize: '11px'
                }}
              >
                Export JSON
              </button>
            </div>
            
            <div style={{ maxHeight: '300px', overflowY: 'auto' }}>
              {testResults.map((result, index) => (
                <div
                  key={index}
                  style={{
                    margin: '5px 0',
                    padding: '8px',
                    background: '#1a1a1a',
                    borderRadius: '4px',
                    fontSize: '11px'
                  }}
                >
                  <div style={{ fontWeight: 'bold', color: result.performanceScore >= 80 ? '#2ECC71' : result.performanceScore >= 60 ? '#F1C40F' : '#E74C3C' }}>
                    {result.scenario} - Score: {result.performanceScore}/100
                  </div>
                  <div style={{ color: '#aaa' }}>
                    FPS: {result.avgFps.toFixed(1)} ({result.minFps}-{result.maxFps}) | 
                    Frame: {result.avgFrameTime.toFixed(1)}ms | 
                    Memory: {result.maxMemoryUsed}MB
                  </div>
                  {result.recommendations.length > 0 && (
                    <div style={{ marginTop: '5px', fontSize: '10px', color: '#F39C12' }}>
                      ⚠️ {result.recommendations.join(', ')}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};