import React, { useEffect, useRef, useCallback, useState } from 'react';
import GraphViewport from '../features/graph/components/GraphViewport';
import { AuthGatedVoiceButton } from '../components/AuthGatedVoiceButton';
import { AuthGatedVoiceIndicator } from '../components/AuthGatedVoiceIndicator';
import { createLogger } from '../utils/logger';
import { useXRCore } from '../features/xr/providers/XRCoreProvider';
import { useApplicationMode } from '../contexts/ApplicationModeContext';
import { useSettingsStore } from '../store/settingsStore';
import { webSocketService } from '../services/WebSocketService';

const logger = createLogger('Quest3ARLayout');

/**
 * Specialized layout for Quest 3 AR mode
 * - No control panels or traditional UI
 * - Full-screen AR viewport
 * - Voice interaction only
 * - Optimized for passthrough AR experience with performance optimizations
 * - Browser detection for Meta Quest 3
 * - Level-of-detail rendering for distant nodes
 * - Optimized WebSocket update rates
 */
const Quest3ARLayout: React.FC = () => {
  const { isSessionActive, sessionType } = useXRCore();
  const { setMode } = useApplicationMode();
  const settings = useSettingsStore((state) => state.settings);
  const [isConnected, setIsConnected] = useState(false);
  
  // Performance optimization refs
  const frameRef = useRef<number>();
  const lastUpdateRef = useRef<number>(0);
  const [updateRate, setUpdateRate] = useState(30); // Default 30fps for AR
  
  // Browser detection for Meta Quest 3
  const isQuest3 = useRef(false);
  
  useEffect(() => {
    const userAgent = navigator.userAgent.toLowerCase();
    isQuest3.current = userAgent.includes('quest 3') || 
                      userAgent.includes('meta quest 3') ||
                      (userAgent.includes('oculus') && userAgent.includes('quest'));
    
    if (isQuest3.current) {
      logger.info('Meta Quest 3 detected - applying AR optimizations');
      setUpdateRate(72); // Quest 3 native refresh rate
    }
  }, []);

  // Ensure XR mode is active when this layout is used
  useEffect(() => {
    if (isSessionActive && sessionType === 'immersive-ar') {
      setMode('xr');
      logger.info('Quest 3 AR Layout activated - entering XR mode');
    }
  }, [isSessionActive, sessionType, setMode]);
  
  // Optimized render loop with requestAnimationFrame
  const renderLoop = useCallback(() => {
    const now = performance.now();
    const deltaTime = now - lastUpdateRef.current;
    
    // Throttle updates based on target frame rate
    if (deltaTime >= 1000 / updateRate) {
      lastUpdateRef.current = now;
      // Update logic would go here
    }
    
    frameRef.current = requestAnimationFrame(renderLoop);
  }, [updateRate]);
  
  useEffect(() => {
    if (isSessionActive) {
      frameRef.current = requestAnimationFrame(renderLoop);
    }
    
    return () => {
      if (frameRef.current) {
        cancelAnimationFrame(frameRef.current);
      }
    };
  }, [isSessionActive, renderLoop]);

  // Monitor WebSocket connection
  useEffect(() => {
    const checkConnection = () => {
      setIsConnected(webSocketService.isConnected());
    };
    
    // Check initial connection state
    checkConnection();
    
    // Set up periodic check
    const interval = setInterval(checkConnection, 1000);
    
    return () => clearInterval(interval);
  }, []);

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
      {/* Full-screen AR viewport - removed unnecessary wrapper div */}
      <GraphViewport 
        style={{
          width: '100vw',
          height: '100vh',
          position: 'absolute',
          top: 0,
          left: 0,
          zIndex: 1
        }}
        // Performance optimizations
        levelOfDetail={true}
        maxRenderDistance={100} // TODO: Add arRenderDistance to settings
        updateRate={updateRate}
      />
      
      {/* TODO: Add Knowledge Graph overlay for AR when component is implemented */}
      {/* TODO: Add Agent Monitor overlay for AR when component is implemented */}

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
        <AuthGatedVoiceButton
          size="lg"
          variant="primary"
          className="bg-blue-500 bg-opacity-90 backdrop-blur-md border-2 border-white border-opacity-30 shadow-lg"
        />
        <AuthGatedVoiceIndicator
          className="max-w-sm text-center bg-black bg-opacity-70 backdrop-blur-md rounded-xl p-3 border border-white border-opacity-20 text-white"
          showTranscription={true}
          showStatus={true}
        />
      </div>

      {/* AR session status indicator with performance info */}
      {isSessionActive && sessionType === 'immersive-ar' && (
        <div style={{
          position: 'fixed',
          top: '20px',
          left: '20px',
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
          {isQuest3.current ? 'Quest 3' : 'XR'} AR Active • {updateRate}fps
        </div>
      )}

      {/* Debug info for AR session (only in debug mode) */}
      {process.env.NODE_ENV === 'development' && isSessionActive && (
        <div style={{
          position: 'fixed',
          top: '60px',
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
          <div>Device: {isQuest3.current ? 'Meta Quest 3' : 'Generic XR'}</div>
          <div>Update Rate: {updateRate} fps</div>
          <div>WebSocket: {isConnected ? 'Connected' : 'Disconnected'}</div>
          <div>Knowledge Graph: {settings?.graph?.enableKnowledgeGraph ? 'On' : 'Off'}</div>
          <div>Agent Monitor: Pending Implementation</div>
        </div>
      )}
    </div>
  );
};

export default Quest3ARLayout;