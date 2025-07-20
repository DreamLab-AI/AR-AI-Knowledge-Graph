import { useEffect, useState } from 'react';
import { quest3AutoDetector, Quest3DetectionResult } from '../services/quest3AutoDetector';
import { useXRCore } from '../features/xr/providers/XRCoreProvider';
import { useApplicationMode } from '../contexts/ApplicationModeContext';
import { createLogger } from '../utils/logger';

const logger = createLogger('useQuest3Integration');

export interface Quest3IntegrationState {
  isQuest3Detected: boolean;
  isAutoStartEnabled: boolean;
  autoStartAttempted: boolean;
  autoStartSuccessful: boolean;
  detectionResult: Quest3DetectionResult | null;
  error: string | null;
}

export interface Quest3IntegrationOptions {
  enableAutoStart?: boolean;
  retryOnFailure?: boolean;
  maxRetries?: number;
}

/**
 * Hook for Quest 3 auto-detection and AR session management
 * Handles automatic detection, session start, and layout switching
 */
export const useQuest3Integration = (options: Quest3IntegrationOptions = {}) => {
  const {
    enableAutoStart = true,
    retryOnFailure = true,
    maxRetries = 3
  } = options;

  const [state, setState] = useState<Quest3IntegrationState>({
    isQuest3Detected: false,
    isAutoStartEnabled: enableAutoStart,
    autoStartAttempted: false,
    autoStartSuccessful: false,
    detectionResult: null,
    error: null
  });

  const [retryCount, setRetryCount] = useState(0);

  const { isSessionActive, sessionType, startSession } = useXRCore();
  const { setMode } = useApplicationMode();

  // Perform Quest 3 detection on mount
  useEffect(() => {
    const detectQuest3 = async () => {
      try {
        logger.info('Starting Quest 3 detection...');
        const result = await quest3AutoDetector.detectQuest3Environment();

        setState(prev => ({
          ...prev,
          isQuest3Detected: result.isQuest3,
          detectionResult: result,
          error: null
        }));

        if (result.shouldAutoStart) {
          logger.info('Quest 3 detected with AR support - auto-start conditions met');
        } else {
          logger.info('Quest 3 detection completed - auto-start not enabled', result);
        }

      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown detection error';
        logger.error('Quest 3 detection failed:', error);
        setState(prev => ({
          ...prev,
          error: errorMessage
        }));
      }
    };

    detectQuest3();
  }, []);

  // Auto-start AR session if conditions are met
  useEffect(() => {
    const autoStartAR = async () => {
      if (!enableAutoStart ||
          state.autoStartAttempted ||
          !state.detectionResult?.shouldAutoStart ||
          isSessionActive) {
        return;
      }

      try {
        setState(prev => ({ ...prev, autoStartAttempted: true, error: null }));

        logger.info('Attempting Quest 3 AR auto-start...');
        const success = await quest3AutoDetector.autoStartQuest3AR();

        if (success) {
          setState(prev => ({ ...prev, autoStartSuccessful: true }));
          // XR mode will be handled by the layout component
          logger.info('Quest 3 AR auto-start successful');
        } else {
          throw new Error('Auto-start returned false');
        }

      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Auto-start failed';
        logger.error('Quest 3 AR auto-start failed:', error);

        setState(prev => ({
          ...prev,
          autoStartSuccessful: false,
          error: errorMessage
        }));

        // Retry logic
        if (retryOnFailure && retryCount < maxRetries) {
          setTimeout(() => {
            setRetryCount(prev => prev + 1);
            setState(prev => ({ ...prev, autoStartAttempted: false }));
            logger.info(`Retrying Quest 3 AR auto-start (attempt ${retryCount + 2}/${maxRetries + 1})`);
          }, 2000);
        }
      }
    };

    autoStartAR();
  }, [
    enableAutoStart,
    state.detectionResult?.shouldAutoStart,
    state.autoStartAttempted,
    isSessionActive,
    retryOnFailure,
    retryCount,
    maxRetries,
    setMode
  ]);

  // Manual start function for fallback
  const manualStartQuest3AR = async (): Promise<boolean> => {
    try {
      if (!state.detectionResult?.supportsAR) {
        throw new Error('AR not supported on this device');
      }

      logger.info('Manual Quest 3 AR start requested');
      await startSession('immersive-ar');

      setState(prev => ({
        ...prev,
        autoStartSuccessful: true,
        error: null
      }));

      return true;
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Manual start failed';
      logger.error('Manual Quest 3 AR start failed:', error);

      setState(prev => ({
        ...prev,
        error: errorMessage
      }));

      return false;
    }
  };

  // Reset state (for testing/development)
  const resetDetection = () => {
    quest3AutoDetector.resetDetection();
    setState({
      isQuest3Detected: false,
      isAutoStartEnabled: enableAutoStart,
      autoStartAttempted: false,
      autoStartSuccessful: false,
      detectionResult: null,
      error: null
    });
    setRetryCount(0);
  };

  // Check if we should use Quest 3 AR layout
  const shouldUseQuest3Layout = isSessionActive &&
                                sessionType === 'immersive-ar' &&
                                state.isQuest3Detected;

  return {
    ...state,
    shouldUseQuest3Layout,
    isInARSession: isSessionActive && sessionType === 'immersive-ar',
    retryCount,
    maxRetries,
    manualStartQuest3AR,
    resetDetection
  };
};

export default useQuest3Integration;