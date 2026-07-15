import React, { useEffect, useState } from 'react';
import { VRGraphCanvas } from '../threejs/VRGraphCanvas';
import { useImmersiveData } from '../hooks/useImmersiveData';
import { createLogger } from '../../utils/loggerConfig';
import { createRemoteLogger, remoteLogger } from '../../services/remoteLogger';

const logger = createLogger('ImmersiveApp');
const remoteLog = createRemoteLogger('ImmersiveApp');

export interface ImmersiveAppProps {
  onExit?: () => void;
  initialData?: any;
}

export const ImmersiveApp: React.FC<ImmersiveAppProps> = ({ onExit, initialData }) => {
  const [isInitialized, setIsInitialized] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isDragging, setIsDragging] = useState(false);

  const {
    graphData,
    nodePositions,
    isLoading: dataLoading,
    error: dataError,
    updateNodePosition,
    selectNode,
    selectedNode
  } = useImmersiveData(initialData);

  useEffect(() => {
    const initializeImmersiveEnvironment = () => {
      try {
        logger.info('Initializing Three.js XR environment...');
        remoteLog.info('Initializing Three.js XR environment on ' + navigator.userAgent);

        const isQuest = /OculusBrowser|Quest/i.test(navigator.userAgent);
        if (isQuest) {
          remoteLog.info('Quest device detected!');
          remoteLogger.logXRInfo();
        }

        setIsInitialized(true);
        logger.info('Three.js XR environment initialized successfully');
        remoteLog.info('Three.js XR environment initialized successfully');

      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : 'Unknown initialization error';
        logger.error('Failed to initialize immersive environment:', errorMessage);
        remoteLog.error('Failed to initialize immersive environment', err);
        setError(errorMessage);
      }
    };

    initializeImmersiveEnvironment();

    return () => {
      logger.info('Cleaning up immersive environment...');
      setIsInitialized(false);
    };
  }, []);

  const handleRetry = () => {
    setError(null);
    setIsInitialized(false);
    setTimeout(() => setIsInitialized(true), 100);
  };

  const handleDragStateChange = (dragging: boolean) => {
    setIsDragging(dragging);
  };

  if (error) {
    return (
      <div className="immersive-error">
        <h2>Immersive Environment Error</h2>
        <p>{error}</p>
        <div className="error-actions">
          <button onClick={handleRetry}>Retry</button>
          {onExit && <button onClick={onExit}>Exit</button>}
        </div>
      </div>
    );
  }

  if (dataError) {
    return (
      <div className="immersive-error">
        <h2>Data Loading Error</h2>
        <p>{dataError}</p>
        <div className="error-actions">
          <button onClick={() => window.location.reload()}>Reload</button>
          {onExit && <button onClick={onExit}>Exit</button>}
        </div>
      </div>
    );
  }

  return (
    <div className="immersive-app" style={{
      width: '100vw',
      height: '100vh',
      position: 'fixed',
      top: 0,
      left: 0,
      zIndex: 1000
    }}>
      {!isInitialized || dataLoading ? (
        <div className="immersive-loading">
          <div className="loading-content">
            <h2>Initializing Immersive Environment</h2>
            <p>Setting up Three.js XR and graph visualization...</p>
            <div className="loading-spinner" />
          </div>
        </div>
      ) : graphData ? (
        <VRGraphCanvas
          onDragStateChange={handleDragStateChange}
        />
      ) : null}

      {isInitialized && (
        <div className="immersive-overlay" style={{
          position: 'absolute',
          top: '10px',
          right: '10px',
          zIndex: 1001
        }}>
          <div className="immersive-controls">
            {onExit && (
              <button
                className="exit-button"
                onClick={onExit}
                title="Exit Immersive Mode"
                style={{
                  padding: '10px 20px',
                  backgroundColor: '#f44336',
                  color: 'white',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: 'pointer'
                }}
              >
                Exit to Desktop
              </button>
            )}
          </div>

          {selectedNode && (
            <div className="immersive-info" style={{
              marginTop: '10px',
              padding: '10px',
              background: 'rgba(0,0,0,0.7)',
              color: 'white',
              borderRadius: '4px'
            }}>
              <h3>Selected: {selectedNode}</h3>
            </div>
          )}

          {isDragging && (
            <div style={{
              marginTop: '10px',
              padding: '5px 10px',
              background: 'rgba(33, 150, 243, 0.8)',
              color: 'white',
              borderRadius: '4px'
            }}>
              Dragging node...
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default ImmersiveApp;
