import { useEffect, useState } from 'react';
import { botsWebSocketIntegration } from '../services/BotsWebSocketIntegration';
import { createLogger } from '../../../utils/loggerConfig';
import { agentTelemetry } from '../../../telemetry/AgentTelemetry';
import { useTelemetry } from '../../../telemetry/useTelemetry';

const logger = createLogger('useBotsWebSocketIntegration');


export function useBotsWebSocketIntegration() {
  const telemetry = useTelemetry('useBotsWebSocketIntegration');
  const [connectionStatus, setConnectionStatus] = useState({
    logseq: false,
    overall: false
  });

  useEffect(() => {
    logger.info('Initializing bots WebSocket integration (binary position updates only)');

    const unsubLogseq = botsWebSocketIntegration.on('logseq-connected', ({ connected }) => {
      setConnectionStatus(prev => ({ ...prev, logseq: connected }));


      agentTelemetry.logAgentAction('websocket', 'logseq', connected ? 'connected' : 'disconnected');
    });


    const updateOverall = setInterval(() => {
      const status = botsWebSocketIntegration.getConnectionStatus();
      setConnectionStatus({
        logseq: status.logseq,
        overall: status.overall
      });
    }, 2000);

    
    
    logger.info('WebSocket connection ready for binary position updates. Agent metadata fetched via REST API.');
    agentTelemetry.logAgentAction('websocket', 'hook', 'initialized_position_updates');

    return () => {
      unsubLogseq();
      clearInterval(updateOverall);

      
      agentTelemetry.logAgentAction('websocket', 'hook', 'cleanup');

      
      
    };
  }, []);

  return connectionStatus;
}