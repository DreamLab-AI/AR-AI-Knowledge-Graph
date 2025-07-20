/**
 * React hook for managing parallel graphs
 * Provides easy access to both Logseq and VisionFlow graph states
 */

import { useEffect, useState, useCallback } from 'react';
import { parallelGraphCoordinator, ParallelGraphState } from '../services/parallelGraphCoordinator';
import { createLogger } from '../../../utils/logger';

const logger = createLogger('useParallelGraphs');

export interface UseParallelGraphsOptions {
  enableLogseq?: boolean;
  enableVisionFlow?: boolean;
  autoConnect?: boolean;
}

export interface UseParallelGraphsReturn {
  state: ParallelGraphState;
  isLogseqEnabled: boolean;
  isVisionFlowEnabled: boolean;
  enableLogseq: (enabled: boolean) => void;
  enableVisionFlow: (enabled: boolean) => void;
  logseqPositions: Map<string, { x: number; y: number; z: number }>;
  visionFlowPositions: Map<string, { x: number; y: number; z: number }>;
  refreshPositions: () => void;
}

export function useParallelGraphs(options: UseParallelGraphsOptions = {}): UseParallelGraphsReturn {
  const {
    enableLogseq = false,
    enableVisionFlow = false,
    autoConnect = true
  } = options;

  const [state, setState] = useState<ParallelGraphState>(parallelGraphCoordinator.getState());
  const [logseqPositions, setLogseqPositions] = useState<Map<string, { x: number; y: number; z: number }>>(new Map());
  const [visionFlowPositions, setVisionFlowPositions] = useState<Map<string, { x: number; y: number; z: number }>>(new Map());

  // Initialize coordinator on mount
  useEffect(() => {
    if (autoConnect) {
      parallelGraphCoordinator.initialize().catch(error => {
        logger.error('Failed to initialize parallel graph coordinator:', error);
      });
    }
  }, [autoConnect]);

  // Subscribe to state changes
  useEffect(() => {
    const unsubscribe = parallelGraphCoordinator.onStateChange((newState) => {
      setState(newState);
      
      // Update positions when state changes
      if (newState.logseq.enabled) {
        parallelGraphCoordinator.getLogseqPositions().then(positions => {
          setLogseqPositions(positions);
        });
      }
      
      if (newState.visionflow.enabled) {
        setVisionFlowPositions(parallelGraphCoordinator.getVisionFlowPositions());
      }
    });

    return unsubscribe;
  }, []);

  // Set initial enabled states
  useEffect(() => {
    if (enableLogseq !== state.logseq.enabled) {
      parallelGraphCoordinator.setLogseqEnabled(enableLogseq);
    }
    
    if (enableVisionFlow !== state.visionflow.enabled) {
      parallelGraphCoordinator.setVisionFlowEnabled(enableVisionFlow);
    }
  }, [enableLogseq, enableVisionFlow, state.logseq.enabled, state.visionflow.enabled]);

  // Control functions
  const handleEnableLogseq = useCallback((enabled: boolean) => {
    parallelGraphCoordinator.setLogseqEnabled(enabled);
  }, []);

  const handleEnableVisionFlow = useCallback((enabled: boolean) => {
    parallelGraphCoordinator.setVisionFlowEnabled(enabled);
  }, []);

  // Manual position refresh
  const refreshPositions = useCallback(() => {
    if (state.logseq.enabled) {
      parallelGraphCoordinator.getLogseqPositions().then(positions => {
        setLogseqPositions(positions);
      });
    }
    
    if (state.visionflow.enabled) {
      setVisionFlowPositions(parallelGraphCoordinator.getVisionFlowPositions());
    }
  }, [state.logseq.enabled, state.visionflow.enabled]);

  return {
    state,
    isLogseqEnabled: state.logseq.enabled,
    isVisionFlowEnabled: state.visionflow.enabled,
    enableLogseq: handleEnableLogseq,
    enableVisionFlow: handleEnableVisionFlow,
    logseqPositions,
    visionFlowPositions,
    refreshPositions
  };
}