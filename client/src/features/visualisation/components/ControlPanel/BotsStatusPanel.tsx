

import React, { useState } from 'react';
import { Zap } from 'lucide-react';
import { Button } from '../../../design-system/components/Button';
import { MultiAgentInitializationPrompt } from '../../../bots/components';
import { AgentTelemetryStream } from '../../../bots/components/AgentTelemetryStream';
import { unifiedApiClient } from '../../../../services/api/UnifiedApiClient';
import { botsWebSocketIntegration } from '../../../bots/services/BotsWebSocketIntegration';
import { useBotsData } from '../../../bots/contexts/BotsDataContext';
import type { BotsData } from './types';

interface BotsStatusPanelProps {
  botsData?: BotsData;
}

const statTileClass =
  'flex flex-col items-center gap-0.5 rounded-[calc(var(--radius)-2px)] bg-foreground/5 py-1';

export const BotsStatusPanel: React.FC<BotsStatusPanelProps> = ({ botsData }) => {
  const [showMultiAgentPrompt, setShowMultiAgentPrompt] = useState(false);
  const { updateBotsData } = useBotsData();

  if (!botsData) return null;

  const handleDisconnect = async () => {
    try {
      const response = await unifiedApiClient.post('/bots/disconnect-multi-agent');
      if (response.status >= 200 && response.status < 300) {
        botsWebSocketIntegration.clearAgents();
        updateBotsData({
          nodeCount: 0,
          edgeCount: 0,
          tokenCount: 0,
          mcpConnected: false,
          dataSource: 'disconnected',
          agents: [],
          edges: []
        });
      }
    } catch (error) {

    }
  };

  return (
    <>
      <div data-testid="agents-panel" className="mb-1.5 border-b border-border/40 pb-1.5">
        <div className="mb-1.5 flex items-center gap-1.5 text-amber-400">
          <Zap size={12} aria-hidden="true" />
          <span className="cc-rail-label font-semibold">
            VisionClaw ({botsData.dataSource.toUpperCase()})
          </span>
        </div>

        {botsData.nodeCount === 0 ? (
          <div className="py-1.5 text-center">
            <div className="cc-helper-text mb-1.5">No active multi-agent</div>
            <Button
              data-testid="agents-initialize"
              variant="default"
              size="sm"
              onClick={() => setShowMultiAgentPrompt(true)}
              className="h-auto px-2.5 py-1 text-[10px] font-semibold"
            >
              Initialize multi-agent
            </Button>
          </div>
        ) : (
          <>
            <div className="mb-1.5 grid grid-cols-3 gap-1">
              <div data-testid="agents-stat-agents" className={statTileClass}>
                <span className="cc-helper-text">Agents</span>
                <span className="cc-value-readout font-semibold text-amber-400">
                  {botsData.nodeCount}
                </span>
              </div>
              <div data-testid="agents-stat-links" className={statTileClass}>
                <span className="cc-helper-text">Links</span>
                <span className="cc-value-readout font-semibold text-amber-400">
                  {botsData.edgeCount}
                </span>
              </div>
              <div data-testid="agents-stat-tokens" className={statTileClass}>
                <span className="cc-helper-text">Tokens</span>
                <span className="cc-value-readout font-semibold text-amber-500">
                  {botsData.tokenCount.toLocaleString()}
                </span>
              </div>
            </div>

            <div className="flex gap-1">
              <Button
                data-testid="agents-new-task"
                variant="default"
                size="sm"
                onClick={() => setShowMultiAgentPrompt(true)}
                className="h-auto flex-1 px-2 py-1 text-[10px] font-semibold"
              >
                New Task
              </Button>
              <Button
                data-testid="agents-disconnect"
                variant="destructive"
                size="sm"
                onClick={handleDisconnect}
                className="h-auto flex-1 px-2 py-1 text-[10px] font-semibold"
              >
                Disconnect
              </Button>
            </div>
          </>
        )}
      </div>

      {}
      {botsData.nodeCount > 0 && (
        <AgentTelemetryStream />
      )}

      {showMultiAgentPrompt && (
        <MultiAgentInitializationPrompt
          onClose={() => setShowMultiAgentPrompt(false)}
          onInitialized={() => setShowMultiAgentPrompt(false)}
        />
      )}
    </>
  );
};
