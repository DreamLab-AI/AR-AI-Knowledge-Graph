/**
 * Test file for parallel graph functionality
 * Ensures Logseq and VisionFlow graphs can run independently
 */

import { parallelGraphCoordinator } from '../services/parallelGraphCoordinator';
import { graphDataManager } from '../managers/graphDataManager';
import { mcpWebSocketService } from '../../swarm/services/MCPWebSocketService';
import { swarmPhysicsWorker } from '../../swarm/workers/swarmPhysicsWorker';

describe('Parallel Graph Functionality', () => {
  beforeEach(() => {
    // Initialize the coordinator
    parallelGraphCoordinator.initialize();
  });

  afterEach(() => {
    // Cleanup
    parallelGraphCoordinator.dispose();
  });

  test('Graph types are correctly set', () => {
    // Verify graph data manager is set to Logseq
    expect(graphDataManager.getGraphType()).toBe('logseq');
    
    // Note: We can't directly test private properties of MCPWebSocketService
    // but we can verify it behaves correctly for VisionFlow data
  });

  test('Both graphs can be enabled independently', () => {
    // Enable Logseq graph
    parallelGraphCoordinator.setLogseqEnabled(true);
    let state = parallelGraphCoordinator.getState();
    expect(state.logseq.enabled).toBe(true);
    expect(state.visionflow.enabled).toBe(false);

    // Enable VisionFlow graph
    parallelGraphCoordinator.setVisionFlowEnabled(true);
    state = parallelGraphCoordinator.getState();
    expect(state.logseq.enabled).toBe(true);
    expect(state.visionflow.enabled).toBe(true);

    // Disable Logseq graph
    parallelGraphCoordinator.setLogseqEnabled(false);
    state = parallelGraphCoordinator.getState();
    expect(state.logseq.enabled).toBe(false);
    expect(state.visionflow.enabled).toBe(true);
  });

  test('State change listeners work correctly', () => {
    const mockListener = jest.fn();
    const unsubscribe = parallelGraphCoordinator.onStateChange(mockListener);

    // Should be called immediately with current state
    expect(mockListener).toHaveBeenCalledTimes(1);

    // Enable Logseq - should trigger listener
    parallelGraphCoordinator.setLogseqEnabled(true);
    expect(mockListener).toHaveBeenCalledTimes(2);

    // Enable VisionFlow - should trigger listener
    parallelGraphCoordinator.setVisionFlowEnabled(true);
    expect(mockListener).toHaveBeenCalledTimes(3);

    // Unsubscribe and verify no more calls
    unsubscribe();
    parallelGraphCoordinator.setLogseqEnabled(false);
    expect(mockListener).toHaveBeenCalledTimes(3);
  });

  test('Position maps are independent', async () => {
    // Mock some data
    const logseqPositions = await parallelGraphCoordinator.getLogseqPositions();
    const visionflowPositions = parallelGraphCoordinator.getVisionFlowPositions();

    // Verify they return independent maps
    expect(logseqPositions).toBeInstanceOf(Map);
    expect(visionflowPositions).toBeInstanceOf(Map);
    expect(logseqPositions).not.toBe(visionflowPositions);
  });
});

// Integration test ideas (would require more setup):
// 1. Test that binary data from WebSocket only updates Logseq graph
// 2. Test that MCP updates only affect VisionFlow graph
// 3. Test that physics simulations run independently
// 4. Test that graph renderers can display both graphs simultaneously