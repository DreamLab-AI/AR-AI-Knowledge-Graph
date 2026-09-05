/**
 * AgentStatusBadges.tsx
 * HTML overlay (via drei <Html>) rendered beside each BotsNode on hover / active.
 * Shows health bar, status pills, token usage, current task, capabilities, swarm info.
 */
import React, { useState, useEffect } from 'react';
import { BotsAgent } from '../types/BotsTypes';

export interface AgentStatusBadgesProps {
  agent: BotsAgent;
  logs?: string[];
}

export const AgentStatusBadges: React.FC<AgentStatusBadgesProps> = ({
  agent,
  logs = [],
}) => {
  const [logKey, setLogKey] = useState(0);
  const [displayLogs, setDisplayLogs] = useState<{ text: string; key: number }[]>([]);

  useEffect(() => {
    const newLogs = logs.slice(-3).map((log, index) => ({
      text: log,
      key: logKey + index,
    }));
    setDisplayLogs(newLogs);
    setLogKey(prev => prev + logs.length);
  // Deps intentionally narrowed; react-hooks/exhaustive-deps is not enforced in this config.
  }, [logs]);

  return (
    <div style={{
      display: 'flex',
      flexDirection: 'column',
      gap: '4px',
      minWidth: '250px',
      maxWidth: '350px',
    }}>
      {/* Header: name + type */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '4px' }}>
        <span style={{ fontWeight: 'bold', fontSize: '14px', color: '#1A1A1A' }}>
          {agent.name || agent.id}
        </span>
        <span style={{
          fontSize: '11px',
          padding: '2px 6px',
          borderRadius: '3px',
          backgroundColor: 'rgba(0, 0, 0, 0.1)',
          color: '#333',
        }}>
          {agent.type}
        </span>
      </div>

      {/* Bioluminescent health bar */}
      <div style={{
        width: '100%',
        height: '2px',
        backgroundColor: 'rgba(0, 0, 0, 0.15)',
        borderRadius: '1px',
        overflow: 'hidden',
        marginBottom: '2px',
      }}>
        <div style={{
          width: `${agent.health}%`,
          height: '100%',
          background: agent.health >= 80
            ? 'linear-gradient(to right, #2ECC71, #00FF00)'
            : agent.health >= 50
            ? 'linear-gradient(to right, #F39C12, #F1C40F)'
            : 'linear-gradient(to right, #E74C3C, #E67E22)',
          borderRadius: '1px',
          transition: 'width 0.5s ease',
          boxShadow: agent.health >= 80
            ? '0 0 4px rgba(46, 204, 113, 0.6)'
            : agent.health >= 50
            ? '0 0 4px rgba(243, 156, 18, 0.6)'
            : '0 0 4px rgba(231, 76, 60, 0.6)',
        }} />
      </div>

      {/* Status/health/cpu/memory/success pills */}
      <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
        <div style={{
          padding: '3px 8px',
          borderRadius: '12px',
          fontSize: '11px',
          backgroundColor: agent.status === 'active' ? '#2ECC71' :
                          agent.status === 'busy'   ? '#F39C12' :
                          agent.status === 'idle'   ? '#95A5A6' : '#E74C3C',
          color: 'white',
          fontWeight: '500',
        }}>
          {agent.status}
        </div>

        <div style={{
          padding: '3px 8px',
          borderRadius: '12px',
          fontSize: '11px',
          backgroundColor: agent.health > 80 ? '#27AE60' : agent.health > 50 ? '#F39C12' : '#E74C3C',
          color: 'white',
        }}>
          Health: {agent.health.toFixed(0)}%
        </div>

        {agent.cpuUsage > 0 && (
          <div style={{
            padding: '3px 8px',
            borderRadius: '12px',
            fontSize: '11px',
            backgroundColor: 'rgba(52, 152, 219, 0.8)',
            color: 'white',
          }}>
            CPU: {agent.cpuUsage.toFixed(0)}%
          </div>
        )}

        {agent.memoryUsage && agent.memoryUsage > 0 && (
          <div style={{
            padding: '3px 8px',
            borderRadius: '12px',
            fontSize: '11px',
            backgroundColor: 'rgba(155, 89, 182, 0.8)',
            color: 'white',
          }}>
            MEM: {agent.memoryUsage.toFixed(0)}%
          </div>
        )}

        {agent.successRate !== undefined && (
          <div style={{
            padding: '3px 8px',
            borderRadius: '12px',
            fontSize: '11px',
            backgroundColor: agent.successRate > 0.8 ? '#27AE60' :
                            agent.successRate > 0.6 ? '#F39C12' : '#E74C3C',
            color: 'white',
          }}>
            Success: {(agent.successRate * 100).toFixed(0)}%
          </div>
        )}
      </div>

      {/* Token usage */}
      {(agent.tokens || agent.tokenRate) && (
        <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap', marginTop: '2px' }}>
          {agent.tokens && (
            <div style={{
              padding: '2px 6px',
              borderRadius: '10px',
              fontSize: '10px',
              backgroundColor: 'rgba(230, 126, 34, 0.8)',
              color: 'white',
            }}>
              Tokens: {agent.tokens.toLocaleString()}
            </div>
          )}
          {agent.tokenRate && (
            <div style={{
              padding: '2px 6px',
              borderRadius: '10px',
              fontSize: '10px',
              backgroundColor: agent.tokenRate > 10 ? 'rgba(243, 156, 18, 0.9)' : 'rgba(231, 76, 60, 0.8)',
              color: 'white',
              animation: agent.tokenRate > 10 ? 'sparkle 1s ease-in-out infinite' : 'none',
              display: 'flex',
              alignItems: 'center',
              gap: '2px',
            }}>
              {agent.tokenRate > 10 && <span style={{ fontSize: '9px' }}>{'~'}</span>}
              {Math.round(agent.tokenRate)}/min
            </div>
          )}
        </div>
      )}

      {/* Task counts */}
      {((agent.tasksActive ?? 0) > 0 || (agent.tasksCompleted ?? 0) > 0) && (
        <div style={{ fontSize: '10px', color: '#666', marginTop: '2px' }}>
          Tasks: {agent.tasksActive} active, {agent.tasksCompleted} completed
        </div>
      )}

      {/* Current task / logs */}
      {(agent.currentTask || displayLogs.length > 0) && (
        <div style={{
          marginTop: '4px',
          fontSize: '10px',
          color: '#444',
          lineHeight: '1.3',
          maxHeight: '60px',
          overflow: 'hidden',
        }}>
          {agent.currentTask ? (
            <div style={{ fontStyle: 'italic' }}>{agent.currentTask}</div>
          ) : (
            displayLogs.map((log, index) => (
              <div
                key={log.key}
                style={{
                  opacity: 1 - (index * 0.3),
                  animation: 'fadeIn 0.5s ease-in',
                  marginBottom: '2px',
                }}
              >
                • {log.text}
              </div>
            ))
          )}
        </div>
      )}

      {/* Capabilities */}
      {agent.capabilities && agent.capabilities.length > 0 && (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '3px', marginTop: '4px' }}>
          {agent.capabilities.slice(0, 4).map(cap => (
            <span
              key={cap}
              style={{
                fontSize: '9px',
                padding: '1px 4px',
                borderRadius: '3px',
                backgroundColor: 'rgba(0, 123, 255, 0.1)',
                color: '#0056b3',
                border: '1px solid rgba(0, 123, 255, 0.2)',
              }}
            >
              {cap.replace(/_/g, ' ')}
            </span>
          ))}
          {agent.capabilities.length > 4 && (
            <span style={{ fontSize: '9px', color: '#999' }}>
              +{agent.capabilities.length - 4} more
            </span>
          )}
        </div>
      )}

      {/* Mode / age */}
      {(agent.agentMode || agent.age) && (
        <div style={{
          fontSize: '9px',
          color: '#666',
          marginTop: '2px',
          display: 'flex',
          gap: '8px',
        }}>
          {agent.agentMode && <span>Mode: {agent.agentMode}</span>}
          {agent.age && <span>Age: {Math.floor(agent.age / 1000 / 60)}m</span>}
        </div>
      )}

      {/* Swarm info */}
      {agent.swarmId && (
        <div style={{ fontSize: '9px', color: '#888', marginTop: '2px' }}>
          swarm: {agent.swarmId}
          {agent.parentQueenId && ` • Queen: ${agent.parentQueenId.slice(0, 8)}...`}
        </div>
      )}
    </div>
  );
};
