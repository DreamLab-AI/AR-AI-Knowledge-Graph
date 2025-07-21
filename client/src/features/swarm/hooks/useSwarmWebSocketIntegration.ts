import { useEffect, useState } from 'react';
import { swarmWebSocketIntegration } from '../services/SwarmWebSocketIntegration';
import { createLogger } from '../../../utils/logger';

const logger = createLogger('useSwarmWebSocketIntegration');

/**
 * Hook to manage SwarmWebSocket integration lifecycle
 * Ensures the integration service is properly initialized and cleaned up
 */
export function useSwarmWebSocketIntegration() {
  const [connectionStatus, setConnectionStatus] = useState({
    mcp: false,
    logseq: false,
    overall: false
  });
  
  useEffect(() => {
    logger.info('Initializing swarm WebSocket integration');
    
    // Listen for connection status changes
    const unsubMcp = swarmWebSocketIntegration.on('mcp-connected', ({ connected }) => {
      setConnectionStatus(prev => ({ ...prev, mcp: connected }));
    });
    
    const unsubLogseq = swarmWebSocketIntegration.on('logseq-connected', ({ connected }) => {
      setConnectionStatus(prev => ({ ...prev, logseq: connected }));
    });
    
    // Update overall status
    const updateOverall = setInterval(() => {
      const status = swarmWebSocketIntegration.getConnectionStatus();
      setConnectionStatus({
        mcp: status.mcp,
        logseq: status.logseq,
        overall: status.overall
      });
    }, 2000);
    
    // Request initial data once connected
    const checkAndRequestData = setInterval(() => {
      const status = swarmWebSocketIntegration.getConnectionStatus();
      if (status.overall) {
        swarmWebSocketIntegration.requestInitialData()
          .then(() => logger.info('Initial data requested'))
          .catch(err => logger.error('Failed to request initial data:', err));
        clearInterval(checkAndRequestData);
      }
    }, 1000);
    
    return () => {
      unsubMcp();
      unsubLogseq();
      clearInterval(updateOverall);
      clearInterval(checkAndRequestData);
      // Note: We don't disconnect here as the service is a singleton
      // and might be used by other components
    };
  }, []);
  
  return connectionStatus;
}