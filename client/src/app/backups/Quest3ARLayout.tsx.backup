import React, { useEffect } from 'react';
import GraphViewport from '../features/graph/components/GraphViewport';
import { VoiceButton } from '../components/VoiceButton';
import { VoiceIndicator } from '../components/VoiceIndicator';
import { createLogger } from '../utils/logger';
import { useXRCore } from '../features/xr/providers/XRCoreProvider';
import { useApplicationMode } from '../contexts/ApplicationModeContext';

const logger = createLogger('Quest3ARLayout');

/**
 * Specialized layout for Quest 3 AR mode
 * - No control panels or traditional UI
 * - Full-screen AR viewport
 * - Voice interaction only
 * - Optimized for passthrough AR experience
 */
const Quest3ARLayout: React.FC = () => {
  const { isSessionActive, sessionType } = useXRCore();
  const { setMode } = useApplicationMode();

  // Ensure XR mode is active when this layout is used
  useEffect(() => {
    if (isSessionActive && sessionType === 'immersive-ar') {
      setMode('xr');
      logger.info('Quest 3 AR Layout activated - entering XR mode');
    }
  }, [isSessionActive, sessionType, setMode]);

  return (
    <div style={{
      width: '100vw',
      height: '100vh',
      position: 'relative',
      overflow: 'hidden',
      backgroundColor: 'transparent', // For AR passthrough
      margin: 0,
      padding: 0
    }}>
      {/* Full-screen AR viewport */}
      <div style={{
        width: '100%',
        height: '100%',
        position: 'absolute',
        top: 0,
        left: 0,
        zIndex: 1
      }}>
        <GraphViewport />
      </div>

      {/* Minimal AR-optimized voice controls */}
      <div style={{
        position: 'fixed',
        bottom: '40px',
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 1000,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: '16px',
        pointerEvents: 'auto' // Ensure interaction works in AR
      }}>
        <VoiceButton
          size="lg"
          variant="primary"
          className="bg-blue-500 bg-opacity-90 backdrop-blur-md border-2 border-white border-opacity-30 shadow-lg"
        />
        <VoiceIndicator
          className="max-w-sm text-center bg-black bg-opacity-70 backdrop-blur-md rounded-xl p-3 border border-white border-opacity-20 text-white"
          showTranscription={true}
          showStatus={true}
        />
      </div>

      {/* AR session status indicator */}
      {isSessionActive && sessionType === 'immersive-ar' && (
        <div style={{
          position: 'fixed',
          top: '20px',
          right: '20px',
          zIndex: 1000,
          backgroundColor: 'rgba(0, 255, 0, 0.8)',
          color: 'black',
          padding: '8px 12px',
          borderRadius: '20px',
          fontSize: '14px',
          fontWeight: 'bold',
          backdropFilter: 'blur(10px)',
          border: '1px solid rgba(255, 255, 255, 0.3)',
          pointerEvents: 'none'
        }}>
          Quest 3 AR Active
        </div>
      )}

      {/* Debug info for AR session (only in debug mode) */}
      {process.env.NODE_ENV === 'development' && isSessionActive && (
        <div style={{
          position: 'fixed',
          top: '20px',
          left: '20px',
          zIndex: 999,
          backgroundColor: 'rgba(0, 0, 0, 0.8)',
          color: 'white',
          padding: '12px',
          borderRadius: '8px',
          fontSize: '12px',
          fontFamily: 'monospace',
          backdropFilter: 'blur(10px)',
          border: '1px solid rgba(255, 255, 255, 0.2)',
          pointerEvents: 'none',
          maxWidth: '300px'
        }}>
          <div>Session Type: {sessionType}</div>
          <div>AR Layout: Active</div>
          <div>Voice Controls: Available</div>
        </div>
      )}
    </div>
  );
};

export default Quest3ARLayout;