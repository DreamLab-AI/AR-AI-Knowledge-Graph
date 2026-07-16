import React, { useState, useEffect, useMemo, useRef } from 'react';
import { useBotsData } from '../contexts/BotsDataContext';
import { useAgentActionFeed, agentActionVerb } from '../hooks/useAgentActionFeed';
import { AgentActionType } from '@/services/BinaryWebSocketProtocol';
import type { BotsAgent } from '../types/BotsTypes';
import { Card } from '../../design-system/components/Card';
import { focusNodeById } from '@/features/visualisation/cameraFocus';

type LogLevel = 'info' | 'warning' | 'error' | 'success';

interface ActivityLogEntry {
  id: string;
  timestamp: Date;
  agentId: string;
  agentName: string;
  agentType: string;
  message: string;
  level: LogLevel;
}

interface ActivityLogPanelProps {
  className?: string;
  maxEntries?: number;
}

// Green/amber/red mapping keeps the action stream legible at a glance without
// leaving the panel's existing colour vocabulary.
const actionLevel = (actionType: AgentActionType): LogLevel => {
  switch (actionType) {
    case AgentActionType.Delete: return 'error';
    case AgentActionType.Create: return 'success';
    case AgentActionType.Update: return 'warning';
    default: return 'info';
  }
};

const shorten = (value: string, max = 18): string =>
  value.length > max ? `${value.slice(0, max - 1)}…` : value;

export const ActivityLogPanel: React.FC<ActivityLogPanelProps> = ({
  className,
  maxEntries = 100,
}) => {
  const { botsData } = useBotsData();
  const feed = useAgentActionFeed({ limit: maxEntries });
  const [logEntries, setLogEntries] = useState<ActivityLogEntry[]>([]);
  const [autoScroll, setAutoScroll] = useState(true);
  const logContainerRef = useRef<HTMLDivElement>(null);
  const lastUpdateRef = useRef<string | undefined>(undefined);

  // Resolve numeric wire ids against the polled agent roster for readable labels.
  const agentsById = useMemo(() => {
    const map = new Map<string, BotsAgent>();
    botsData?.agents?.forEach(agent => map.set(String(agent.id), agent));
    return map;
  }, [botsData?.agents]);

  const resolveLabel = (numericId: number): string => {
    const agent = agentsById.get(String(numericId));
    if (agent) return agent.name || agent.id;
    return `#${numericId}`;
  };

  // Secondary fallback: synthesise coarse lines from status snapshots, shown
  // only while the live action feed is empty.
  useEffect(() => {
    if (!botsData || !botsData.agents) return;
    if (botsData.lastUpdate === lastUpdateRef.current) return;
    lastUpdateRef.current = botsData.lastUpdate;

    const newEntries: ActivityLogEntry[] = [];

    botsData.agents.forEach(agent => {
      if (agent.status === 'active' || agent.status === 'busy') {
        newEntries.push({
          id: `${agent.id}-status-${Date.now()}`,
          timestamp: new Date(),
          agentId: agent.id,
          agentName: agent.name || agent.id,
          agentType: agent.type,
          message: `Agent is ${agent.status}${agent.currentTask ? `: ${agent.currentTask}` : ''}`,
          level: 'info',
        });
      } else if (agent.status === 'error') {
        newEntries.push({
          id: `${agent.id}-error-${Date.now()}`,
          timestamp: new Date(),
          agentId: agent.id,
          agentName: agent.name || agent.id,
          agentType: agent.type,
          message: 'Agent encountered an error',
          level: 'error',
        });
      }

      if (agent.processingLogs && agent.processingLogs.length > 0) {
        agent.processingLogs.slice(-3).forEach((log, index) => {
          newEntries.push({
            id: `${agent.id}-log-${Date.now()}-${index}`,
            timestamp: new Date(),
            agentId: agent.id,
            agentName: agent.name || agent.id,
            agentType: agent.type,
            message: log,
            level: 'info',
          });
        });
      }

      if (agent.health < 50) {
        newEntries.push({
          id: `${agent.id}-health-${Date.now()}`,
          timestamp: new Date(),
          agentId: agent.id,
          agentName: agent.name || agent.id,
          agentType: agent.type,
          message: `Low health: ${agent.health.toFixed(1)}%`,
          level: 'warning',
        });
      }

      if (agent.cpuUsage > 80) {
        newEntries.push({
          id: `${agent.id}-cpu-${Date.now()}`,
          timestamp: new Date(),
          agentId: agent.id,
          agentName: agent.name || agent.id,
          agentType: agent.type,
          message: `High CPU usage: ${agent.cpuUsage.toFixed(1)}%`,
          level: 'warning',
        });
      }
    });

    setLogEntries(prev => [...prev, ...newEntries].slice(-maxEntries));
  }, [botsData, maxEntries]);

  const showFeed = feed.entries.length > 0;

  // Newest is at the top of the feed, at the bottom of the status fallback —
  // scroll accordingly so the freshest line stays visible.
  useEffect(() => {
    if (!autoScroll || !logContainerRef.current) return;
    const el = logContainerRef.current;
    el.scrollTop = showFeed ? 0 : el.scrollHeight;
  }, [feed.entries, logEntries, autoScroll, showFeed]);

  const getLevelColor = (level: LogLevel) => {
    switch (level) {
      case 'error': return 'text-red-600 bg-red-50';
      case 'warning': return 'text-yellow-600 bg-yellow-50';
      case 'success': return 'text-green-600 bg-green-50';
      default: return 'text-blue-600 bg-blue-50';
    }
  };

  const getLevelIcon = (level: LogLevel) => {
    switch (level) {
      case 'error': return '❌';
      case 'warning': return '⚠️';
      case 'success': return '✅';
      default: return 'ℹ️';
    }
  };

  const clearLog = () => {
    setLogEntries([]);
    feed.clear();
  };

  // Click / Enter / Space on a live action row flies the camera to its target
  // node and pulse-highlights it (reuses the graph's established focus utility).
  const focusTarget = (targetNodeId: number) => {
    focusNodeById(targetNodeId);
  };
  const onRowKeyDown = (e: React.KeyboardEvent, targetNodeId: number) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      focusTarget(targetNodeId);
    }
  };

  const displayCount = showFeed ? feed.entries.length : logEntries.length;

  return (
    <Card className={className}>
      <div className="p-4 h-full flex flex-col">
        <div className="flex justify-between items-center mb-3">
          <h3 className="text-lg font-semibold">Activity Log</h3>
          <div className="flex items-center gap-2">
            <label className="flex items-center text-sm">
              <input
                type="checkbox"
                checked={autoScroll}
                onChange={(e) => setAutoScroll(e.target.checked)}
                className="mr-1"
              />
              Auto-scroll
            </label>
            <button
              onClick={clearLog}
              className="text-xs px-2 py-1 bg-gray-200 hover:bg-gray-300 rounded transition-colors"
            >
              Clear
            </button>
          </div>
        </div>

        <div
          ref={logContainerRef}
          className="flex-1 overflow-y-auto space-y-1 text-xs font-mono"
          style={{ maxHeight: '400px' }}
        >
          {showFeed ? (
            feed.entries.map(entry => {
              const level = actionLevel(entry.actionType);
              const targetLabel = resolveLabel(entry.targetNodeId);
              return (
                <div
                  key={entry.id}
                  role="button"
                  tabIndex={0}
                  onClick={() => focusTarget(entry.targetNodeId)}
                  onKeyDown={(e) => onRowKeyDown(e, entry.targetNodeId)}
                  title={`Fly to ${targetLabel}`}
                  aria-label={`${agentActionVerb(entry.actionType)} ${targetLabel} — fly camera to node`}
                  className={`p-2 rounded flex items-start gap-2 cursor-pointer transition-all hover:brightness-95 hover:ring-1 hover:ring-current/30 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 ${getLevelColor(level)}`}
                >
                  <span className="flex-shrink-0">{getLevelIcon(level)}</span>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-baseline gap-2">
                      <span className="text-gray-600">
                        {new Date(entry.ts).toLocaleTimeString()}
                      </span>
                      <span className="font-semibold truncate">
                        {shorten(resolveLabel(entry.sourceAgentId))}
                      </span>
                    </div>
                    <div className="mt-1 break-words">
                      {agentActionVerb(entry.actionType)}{' '}
                      <span className="font-semibold underline decoration-dotted underline-offset-2">{shorten(targetLabel)}</span>
                      {entry.intent ? <span className="text-gray-600"> — {entry.intent}</span> : null}
                      {entry.verification ? (
                        <span className="ml-1 rounded bg-green-100 px-1 text-green-700">
                          ✓ {entry.verification}
                        </span>
                      ) : null}
                    </div>
                  </div>
                </div>
              );
            })
          ) : logEntries.length === 0 ? (
            <div className="text-gray-500 text-center py-4">No activity yet</div>
          ) : (
            logEntries.map(entry => (
              <div
                key={entry.id}
                className={`p-2 rounded flex items-start gap-2 ${getLevelColor(entry.level)}`}
              >
                <span className="flex-shrink-0">{getLevelIcon(entry.level)}</span>
                <div className="flex-1 min-w-0">
                  <div className="flex items-baseline gap-2">
                    <span className="text-gray-600">
                      {entry.timestamp.toLocaleTimeString()}
                    </span>
                    <span className="font-semibold truncate">
                      [{entry.agentType}] {entry.agentName}
                    </span>
                  </div>
                  <div className="mt-1 break-words">{entry.message}</div>
                </div>
              </div>
            ))
          )}
        </div>

        <div className="mt-3 pt-3 border-t border-gray-200 text-xs text-gray-500">
          Showing {displayCount} {showFeed ? 'actions' : 'entries'}
        </div>
      </div>
    </Card>
  );
};
