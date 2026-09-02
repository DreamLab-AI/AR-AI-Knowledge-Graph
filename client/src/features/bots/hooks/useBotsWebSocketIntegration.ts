import { useEffect, useState } from 'react';
import { botsWebSocketIntegration } from '../services/BotsWebSocketIntegration';
import { createLogger } from '../../../utils/loggerConfig';
import { agentTelemetry } from '../../../telemetry/AgentTelemetry';
import { useTelemetry } from '../../../telemetry/useTelemetry';

const logger = createLogger('useBotsWebSocketIntegration');


export function useBotsWebSocketIntegration() {
  const telemetry = useTelemetry('useBotsWebSocketIntegration');
  const [connectionStatus, setConnectionStatus] = useState({
    knowledge: false,
    overall: false
  });

  useEffect(() => {
    logger.info('Initializing bots WebSocket integration (binary position updates only)');

    const unsubKnowledge = botsWebSocketIntegration.on('knowledge-connected', ({ connected }) => {
      setConnectionStatus(prev => ({ ...prev, knowledge: connected }));


      agentTelemetry.logAgentAction('websocket', 'knowledge', connected ? 'connected' : 'disconnected');
    });


    const updateOverall = setInterval(() => {
      const status = botsWebSocketIntegration.getConnectionStatus();
      setConnectionStatus({
        knowledge: status.knowledge,
        overall: status.overall
      });
    }, 2000);

    
    
    logger.info('WebSocket connection ready for binary position updates. Agent metadata fetched via REST API.');
    agentTelemetry.logAgentAction('websocket', 'hook', 'initialized_position_updates');

    return () => {
      unsubKnowledge();
      clearInterval(updateOverall);

      
      agentTelemetry.logAgentAction('websocket', 'hook', 'cleanup');

      
      
    };
  }, []);

  return connectionStatus;
}