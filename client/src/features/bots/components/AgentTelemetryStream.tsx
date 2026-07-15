

import React, { useState, useEffect, useMemo, useRef } from 'react';
import { unifiedApiClient } from '../../../services/api/UnifiedApiClient';
import { createLogger } from '../../../utils/loggerConfig';
import { useAgentActionFeed, agentActionVerb, type AgentActionFeedEntry } from '../hooks/useAgentActionFeed';
import { AgentActionType } from '@/services/BinaryWebSocketProtocol';

const logger = createLogger('AgentTelemetryStream');

type StreamLevel = 'info' | 'warning' | 'error' | 'success';

interface TelemetryMessage {
  key: string;
  timestamp: number;
  agentId: string;
  agentType?: string;
  message: string;
  level: StreamLevel;
}

// Cap the merged (telemetry + actions) view so the CRT stays light.
const MERGED_LIMIT = 80;

const actionLevel = (actionType: AgentActionType): StreamLevel => {
  switch (actionType) {
    case AgentActionType.Delete: return 'error';
    case AgentActionType.Create: return 'success';
    case AgentActionType.Update: return 'warning';
    default: return 'info';
  }
};

const formatActionMessage = (entry: AgentActionFeedEntry): string => {
  const parts = [`ACT:${agentActionVerb(entry.actionType).toUpperCase()}`, `TGT:${entry.targetNodeId}`];
  if (entry.intent) parts.push(entry.intent.substring(0, 24));
  return parts.join(' | ');
};

export const AgentTelemetryStream: React.FC = () => {
  const [messages, setMessages] = useState<TelemetryMessage[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const [activeTab, setActiveTab] = useState<'telemetry' | 'goap'>('telemetry');
  const streamRef = useRef<HTMLDivElement>(null);
  const pollIntervalRef = useRef<NodeJS.Timeout | null>(null);

  // Real 0x23 action stream, interleaved with the status poll below.
  const feed = useAgentActionFeed({ limit: MERGED_LIMIT });

  
  // Merge telemetry snapshots with live action lines, oldest→newest so the
  // freshest line sits at the bottom (matching the auto-scroll behaviour).
  const combined = useMemo<TelemetryMessage[]>(() => {
    const actionLines: TelemetryMessage[] = feed.entries.map(entry => ({
      key: `act-${entry.id}`,
      timestamp: entry.ts,
      agentId: String(entry.sourceAgentId),
      message: formatActionMessage(entry),
      level: actionLevel(entry.actionType),
    }));
    return [...messages, ...actionLines]
      .sort((a, b) => a.timestamp - b.timestamp)
      .slice(-MERGED_LIMIT);
  }, [messages, feed.entries]);

  useEffect(() => {
    if (streamRef.current) {
      streamRef.current.scrollTop = streamRef.current.scrollHeight;
    }
  }, [combined]);

  useEffect(() => {
    const pollTelemetry = async () => {
      try {
        const response = await unifiedApiClient.get('/bots/agents');

        if (response.data && response.data.agents) {
          const pollTs = Date.now();
          const newMessages: TelemetryMessage[] = response.data.agents.map((agent: Record<string, unknown>, idx: number) => ({
            key: `tel-${pollTs}-${idx}`,
            timestamp: pollTs,
            agentId: (agent.id as string) || 'unknown',
            agentType: (agent.type as string) || (agent.agent_type as string) || 'agent',
            message: formatAgentStatus(agent),
            level: getMessageLevel(agent)
          }));

          setMessages(prev => {
            const combined = [...prev, ...newMessages];
            
            return combined.slice(-50);
          });

          setIsConnected(true);
        }
      } catch (error) {
        logger.error('Failed to poll telemetry:', error);
        setIsConnected(false);
      }
    };

    
    pollTelemetry();
    pollIntervalRef.current = setInterval(pollTelemetry, 5000);

    return () => {
      if (pollIntervalRef.current) {
        clearInterval(pollIntervalRef.current);
      }
    };
  }, []);

  const formatAgentStatus = (agent: Record<string, unknown>): string => {
    const parts: string[] = [];

    if (agent.status) parts.push(`STS:${String(agent.status).toUpperCase()}`);
    if (agent.health !== undefined) parts.push(`HP:${Math.round(Number(agent.health))}%`);

    const cpuUsage = agent.cpuUsage ?? agent.cpu_usage;
    if (cpuUsage !== undefined) parts.push(`CPU:${Math.round(Number(cpuUsage))}%`);
    const memoryUsage = agent.memoryUsage ?? agent.memory_usage;
    if (memoryUsage !== undefined) parts.push(`MEM:${Math.round(Number(memoryUsage))}MB`);
    if (agent.workload !== undefined) parts.push(`WL:${Math.round(Number(agent.workload))}`);
    const currentTask = (agent.current_task ?? agent.currentTask) as string | undefined;
    if (currentTask) parts.push(`TSK:${currentTask.substring(0, 20)}`);

    return parts.join(' | ') || 'IDLE';
  };

  const getMessageLevel = (agent: Record<string, unknown>): 'info' | 'warning' | 'error' | 'success' => {
    const health = Number(agent.health ?? 100);
    if (agent.status === 'error' || health < 30) return 'error';
    if (agent.status === 'warning' || health < 60) return 'warning';
    if (agent.status === 'active' || agent.status === 'working') return 'success';
    return 'info';
  };

  const getLevelColor = (level: string): string => {
    switch (level) {
      case 'error': return '#dc2626';
      case 'warning': return '#b45309';
      case 'success': return '#15803d';
      default: return '#000000';
    }
  };

  const formatTime = (timestamp: number): string => {
    const date = new Date(timestamp);
    return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}:${String(date.getSeconds()).padStart(2, '0')}`;
  };

  return (
    <div className="mt-2 border-t border-border/40 pt-2">
      <div className="mb-1.5 flex items-center justify-between text-[9px] font-semibold text-amber-400">
        <div className="flex gap-1.5">
          <button
            onClick={() => setActiveTab('telemetry')}
            className={`rounded-[3px] border border-amber-500 px-1.5 py-0.5 text-[9px] font-semibold transition-colors ${
              activeTab === 'telemetry' ? 'bg-amber-500 text-black' : 'bg-transparent text-amber-400'
            }`}
          >
            TELEMETRY
          </button>
          <button
            onClick={() => setActiveTab('goap')}
            onDoubleClick={() => window.open('https://goal.ruv.io/', '_blank')}
            className={`rounded-[3px] border border-amber-500 px-1.5 py-0.5 text-[9px] font-semibold transition-colors ${
              activeTab === 'goap' ? 'bg-amber-500 text-black' : 'bg-transparent text-amber-400'
            }`}
          >
            GOAP
          </button>
        </div>
        <span
          aria-hidden="true"
          className={`h-2 w-2 rounded-full ${
            isConnected
              ? 'bg-green-500 shadow-[0_0_4px_rgba(34,197,94,0.8)]'
              : 'bg-destructive shadow-[0_0_4px_rgba(239,68,68,0.8)]'
          }`}
        />
      </div>

      {activeTab === 'telemetry' ? (
        <div
          ref={streamRef}
          className="h-[200px] overflow-y-auto rounded border-2 border-amber-700 bg-amber-500 p-2 text-[10px] leading-snug text-black shadow-[inset_0_2px_4px_rgba(0,0,0,0.3)]"
          style={{ fontFamily: "'DSEG7Classic', monospace" }}
        >
          {combined.length === 0 ? (
            <div className="py-5 text-center font-mono text-[9px] text-black/50">
              WAITING FOR TELEMETRY...
            </div>
          ) : (
            combined.map((msg) => (
              <div key={msg.key} className="mb-1 flex gap-2 rounded-sm bg-black/10 px-1 py-0.5 text-[9px]">
                <span className="min-w-[60px] font-bold" style={{ color: getLevelColor(msg.level) }}>
                  {formatTime(msg.timestamp)}
                </span>
                <span className="min-w-[80px] overflow-hidden text-ellipsis whitespace-nowrap font-bold text-black">
                  {msg.agentId.substring(0, 12)}
                </span>
                <span className="flex-1 text-black" style={{ fontFamily: "'DSEG7Classic', monospace" }}>
                  {msg.message}
                </span>
              </div>
            ))
          )}
        </div>
      ) : (
        <div className="h-[200px] overflow-y-auto rounded border-2 border-amber-500 bg-background p-1 font-mono text-[9px] text-amber-400 shadow-[inset_0_2px_4px_rgba(0,0,0,0.3)]">
          <div id="goap-widget-container"></div>
          <style>{`
            #goap-widget-container * {
              font-size: 9px !important;
            }
            #goap-widget-container {
              max-width: 100%;
              margin: 0;
            }
          `}</style>
        </div>
      )}

      <style>{`
        @font-face {
          font-family: 'DSEG7Classic';
          src: url('/fonts/DSEG7Classic-Bold.woff2') format('woff2');
          font-weight: bold;
          font-style: normal;
        }

        @font-face {
          font-family: 'DSEG7Classic';
          src: url('/fonts/DSEG7Classic-Regular.woff2') format('woff2');
          font-weight: normal;
          font-style: normal;
        }
      `}</style>
    </div>
  );
};
