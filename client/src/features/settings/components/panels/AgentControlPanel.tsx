import React, { useState, useEffect } from 'react';
import { Activity, Play, Pause, Square, Settings as SettingsIcon, RefreshCw, AlertCircle, CheckCircle, XCircle, Users, Cpu, Zap, Layers } from 'lucide-react';
import { Button } from '../../../design-system/components/Button';
import { createLogger } from '../../../../utils/loggerConfig';
import { AgentTelemetryStream } from '../../../bots/components/AgentTelemetryStream';
import { useSettingsStore } from '../../../../store/settingsStore';
import { unifiedApiClient } from '../../../../services/api/UnifiedApiClient';
import { SkillsTab } from './SkillsTab';

const logger = createLogger('AgentControlPanel');

// Simple toast helper that works with the existing toast system
const showToast = (message: string, type: 'success' | 'error' | 'info' = 'info') => {
  logger.info(`[Toast ${type}]: ${message}`);
  // The toast system may not expose success/error directly, so just log for now
};

interface AgentStatus {
  id: string;
  type: string;
  status: 'idle' | 'active' | 'busy' | 'error';
  health: number;
  uptime: number;
  tasksCompleted: number;
}

interface AgentControlPanelProps {
  className?: string;
}

type PanelTab = 'agents' | 'skills';

export const AgentControlPanel: React.FC<AgentControlPanelProps> = ({ className }) => {
  const [activeTab, setActiveTab] = useState<PanelTab>('agents');
  const [spawning, setSpawning] = useState(false);
  const [agents, setAgents] = useState<AgentStatus[]>([]);
  const [refreshing, setRefreshing] = useState(false);
  const settings = useSettingsStore(state => state.settings);
  const updateSettings = useSettingsStore(state => state.updateSettings);

  
  const agentTypes = [
    { id: 'researcher', label: 'Researcher', icon: '🔍' },
    { id: 'coder', label: 'Coder', icon: '💻' },
    { id: 'analyzer', label: 'Analyzer', icon: '📊' },
    { id: 'tester', label: 'Tester', icon: '🧪' },
    { id: 'optimizer', label: 'Optimizer', icon: '⚡' },
    { id: 'coordinator', label: 'Coordinator', icon: '🎯' }
  ];

  // Extended agent settings type
  interface AgentSettingsShape {
    spawn?: { auto_scale?: boolean; max_concurrent?: number; default_priority?: string; default_strategy?: string; default_provider?: string };
    lifecycle?: { idle_timeout?: number; auto_restart?: boolean; health_check_interval?: number };
    monitoring?: { telemetry_enabled?: boolean; telemetry_poll_interval?: number; log_level?: string };
    visualization?: { show_in_graph?: boolean; node_size?: number; node_color?: string };
  }
  const agentSettings: AgentSettingsShape = ((settings as unknown as Record<string, unknown>)?.agents as AgentSettingsShape | undefined) || {
    spawn: {
      auto_scale: true,
      max_concurrent: 10,
      default_priority: 'medium',
      default_strategy: 'adaptive',
      default_provider: 'gemini'
    },
    lifecycle: {
      idle_timeout: 300,
      auto_restart: true,
      health_check_interval: 30
    },
    monitoring: {
      telemetry_enabled: true,
      telemetry_poll_interval: 5,
      log_level: 'info'
    },
    visualization: {
      show_in_graph: true,
      node_size: 1.0,
      node_color: '#ff8800'
    }
  };

  
  const fetchAgents = async () => {
    setRefreshing(true);
    try {
      const response = await unifiedApiClient.get('/bots/agents');
      if (response.data && response.data.agents) {
        setAgents(response.data.agents);
      }
    } catch (error) {
      logger.error('Failed to fetch agents:', error);
      showToast('Failed to fetch agent status', 'error');
    } finally {
      setRefreshing(false);
    }
  };

  
  useEffect(() => {
    fetchAgents();
    const interval = setInterval(fetchAgents, (agentSettings.monitoring?.telemetry_poll_interval || 5) * 1000);
    return () => clearInterval(interval);
  }, [agentSettings.monitoring?.telemetry_poll_interval]);

  
  const spawnAgent = async (agentType: string) => {
    setSpawning(true);
    try {
      await unifiedApiClient.post('/bots/spawn-agent-hybrid', {
        agent_type: agentType,
        swarm_id: 'main-swarm',
        method: 'mcp-fallback',
        priority: agentSettings.spawn?.default_priority || 'medium',
        strategy: agentSettings.spawn?.default_strategy || 'adaptive',
        config: {
          auto_scale: agentSettings.spawn?.auto_scale ?? true,
          monitor: agentSettings.monitoring?.telemetry_enabled ?? true,
          max_workers: agentSettings.spawn?.max_concurrent || 10,
          provider: agentSettings.spawn?.default_provider || 'gemini'
        }
      });
      showToast(`${agentType} agent spawned successfully`, 'success');
      fetchAgents();
    } catch (error) {
      logger.error('Failed to spawn agent:', error);
      showToast(`Failed to spawn ${agentType} agent`, 'error');
    } finally {
      setSpawning(false);
    }
  };

  
  const updateSetting = (path: string, value: string | number | boolean) => {
    updateSettings((draft) => {
      const parts = path.split('.');
      let current = draft as unknown as Record<string, unknown>;
      for (let i = 0; i < parts.length - 1; i++) {
        if (!current[parts[i]]) current[parts[i]] = {};
        current = current[parts[i]] as Record<string, unknown>;
      }
      current[parts[parts.length - 1]] = value;
    });
  };

  
  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'active': return <CheckCircle className="w-4 h-4 text-green-500" />;
      case 'busy': return <Activity className="w-4 h-4 text-orange-500 animate-pulse" />;
      case 'error': return <XCircle className="w-4 h-4 text-red-500" />;
      default: return <AlertCircle className="w-4 h-4 text-gray-500" />;
    }
  };

  // Handle skill invocation
  const handleSkillInvoke = (skillIds: string[]) => {
    logger.info('Skills invoked:', skillIds);
    showToast(`Invoked ${skillIds.length} skill(s)`, 'success');
  };

  return (
    <div className={`space-y-4 ${className}`}>
      {/* Tab Switcher */}
      <div className="flex items-center gap-1 p-1 bg-muted rounded-lg">
        <button
          onClick={() => setActiveTab('agents')}
          className={`flex-1 flex items-center justify-center gap-2 py-2 px-3 rounded-md text-sm font-medium transition-colors ${
            activeTab === 'agents'
              ? 'bg-background shadow-sm text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          }`}
        >
          <Users className="w-4 h-4" />
          Agents
        </button>
        <button
          onClick={() => setActiveTab('skills')}
          className={`flex-1 flex items-center justify-center gap-2 py-2 px-3 rounded-md text-sm font-medium transition-colors ${
            activeTab === 'skills'
              ? 'bg-background shadow-sm text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          }`}
        >
          <Layers className="w-4 h-4" />
          Skills
        </button>
      </div>

      {/* Skills Tab Content */}
      {activeTab === 'skills' && (
        <SkillsTab onSkillInvoke={handleSkillInvoke} />
      )}

      {/* Agents Tab Content */}
      {activeTab === 'agents' && (
        <>
      {}
      <div className="border rounded-lg p-4 bg-card">
        <div className="flex items-center justify-between mb-3">
          <h3 className="cc-title flex items-center gap-2">
            <Play className="w-4 h-4" />
            Agent Spawner
          </h3>
          <Button
            variant="ghost"
            size="sm"
            onClick={fetchAgents}
            disabled={refreshing}
          >
            <RefreshCw className={`w-4 h-4 ${refreshing ? 'animate-spin' : ''}`} />
          </Button>
        </div>

        <div className="grid grid-cols-3 gap-2">
          {agentTypes.map(agent => (
            <Button
              key={agent.id}
              onClick={() => spawnAgent(agent.id)}
              disabled={spawning || agents.length >= (agentSettings.spawn?.max_concurrent || 10)}
              size="sm"
              className="flex items-center gap-2"
            >
              <span>{agent.icon}</span>
              <span>{agent.label}</span>
            </Button>
          ))}
        </div>

        <div className="cc-value-readout mt-3 flex items-center justify-between">
          <span>Active Agents: {agents.length} / {agentSettings.spawn?.max_concurrent || 10}</span>
          <span>Provider: {agentSettings.spawn?.default_provider || 'gemini'}</span>
        </div>
      </div>

      {}
      <div className="border rounded-lg p-4 bg-card">
        <h3 className="cc-title mb-3 flex items-center gap-2">
          <Users className="w-4 h-4" />
          Active Agents
        </h3>

        {agents.length === 0 ? (
          <div className="cc-helper-text text-center py-4">
            No active agents. Spawn an agent to get started.
          </div>
        ) : (
          <div className="space-y-2">
            {agents.map(agent => (
              <div key={agent.id} className="flex items-center justify-between p-2 bg-background rounded border">
                <div className="flex items-center gap-2">
                  {getStatusIcon(agent.status)}
                  <div>
                    <div className="cc-field-label font-medium">{agent.type}</div>
                    <div className="cc-helper-text">{agent.id.substring(0, 12)}...</div>
                  </div>
                </div>
                <div className="flex items-center gap-3">
                  <span className="cc-value-readout">Health: {Math.round(agent.health)}%</span>
                  <span className="cc-value-readout">Tasks: {agent.tasksCompleted}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {}
      <div className="border rounded-lg p-4 bg-card">
        <h3 className="cc-title mb-3 flex items-center gap-2">
          <SettingsIcon className="w-4 h-4" />
          Agent Settings
        </h3>

        <div className="space-y-3">
          {}
          <div className="space-y-2">
            <h4 className="cc-subgroup-label border-b border-border/40 pb-1">Spawning</h4>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Auto-Scale</label>
              <input
                type="checkbox"
                checked={agentSettings.spawn?.auto_scale ?? true}
                onChange={(e) => updateSetting('agents.spawn.auto_scale', e.target.checked)}
                className="rounded"
              />
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Max Concurrent Agents</label>
              <input
                type="number"
                value={agentSettings.spawn?.max_concurrent || 10}
                onChange={(e) => updateSetting('agents.spawn.max_concurrent', parseInt(e.target.value))}
                min={1}
                max={50}
                className="w-20 px-2 py-1 text-xs border rounded"
              />
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">AI Provider</label>
              <select
                value={agentSettings.spawn?.default_provider || 'gemini'}
                onChange={(e) => updateSetting('agents.spawn.default_provider', e.target.value)}
                className="px-2 py-1 text-xs border rounded"
              >
                <option value="gemini">Gemini</option>
                <option value="openai">OpenAI</option>
                <option value="claude">Claude</option>
              </select>
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Default Priority</label>
              <select
                value={agentSettings.spawn?.default_priority || 'medium'}
                onChange={(e) => updateSetting('agents.spawn.default_priority', e.target.value)}
                className="px-2 py-1 text-xs border rounded"
              >
                <option value="low">Low</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
                <option value="critical">Critical</option>
              </select>
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Default Strategy</label>
              <select
                value={agentSettings.spawn?.default_strategy || 'adaptive'}
                onChange={(e) => updateSetting('agents.spawn.default_strategy', e.target.value)}
                className="px-2 py-1 text-xs border rounded"
              >
                <option value="parallel">Parallel</option>
                <option value="sequential">Sequential</option>
                <option value="adaptive">Adaptive</option>
              </select>
            </div>
          </div>

          {}
          <div className="space-y-2">
            <h4 className="cc-subgroup-label border-b border-border/40 pb-1">Lifecycle</h4>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Idle Timeout (seconds)</label>
              <input
                type="number"
                value={agentSettings.lifecycle?.idle_timeout || 300}
                onChange={(e) => updateSetting('agents.lifecycle.idle_timeout', parseInt(e.target.value))}
                min={60}
                max={600}
                className="w-20 px-2 py-1 text-xs border rounded"
              />
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Auto-Restart Failed Agents</label>
              <input
                type="checkbox"
                checked={agentSettings.lifecycle?.auto_restart ?? true}
                onChange={(e) => updateSetting('agents.lifecycle.auto_restart', e.target.checked)}
                className="rounded"
              />
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Health Check Interval (seconds)</label>
              <input
                type="number"
                value={agentSettings.lifecycle?.health_check_interval || 30}
                onChange={(e) => updateSetting('agents.lifecycle.health_check_interval', parseInt(e.target.value))}
                min={10}
                max={120}
                className="w-20 px-2 py-1 text-xs border rounded"
              />
            </div>
          </div>

          {}
          <div className="space-y-2">
            <h4 className="cc-subgroup-label border-b border-border/40 pb-1">Monitoring</h4>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Enable Telemetry</label>
              <input
                type="checkbox"
                checked={agentSettings.monitoring?.telemetry_enabled ?? true}
                onChange={(e) => updateSetting('agents.monitoring.telemetry_enabled', e.target.checked)}
                className="rounded"
              />
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Poll Interval (seconds)</label>
              <input
                type="number"
                value={agentSettings.monitoring?.telemetry_poll_interval || 5}
                onChange={(e) => updateSetting('agents.monitoring.telemetry_poll_interval', parseInt(e.target.value))}
                min={1}
                max={30}
                className="w-20 px-2 py-1 text-xs border rounded"
              />
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Log Level</label>
              <select
                value={agentSettings.monitoring?.log_level || 'info'}
                onChange={(e) => updateSetting('agents.monitoring.log_level', e.target.value)}
                className="px-2 py-1 text-xs border rounded"
              >
                <option value="debug">Debug</option>
                <option value="info">Info</option>
                <option value="warn">Warning</option>
                <option value="error">Error</option>
              </select>
            </div>
          </div>

          {}
          <div className="space-y-2">
            <h4 className="cc-subgroup-label border-b border-border/40 pb-1">Visualization</h4>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Show in Main Graph</label>
              <input
                type="checkbox"
                checked={agentSettings.visualization?.show_in_graph ?? true}
                onChange={(e) => updateSetting('agents.visualization.show_in_graph', e.target.checked)}
                className="rounded"
              />
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Agent Node Size</label>
              <input
                type="range"
                value={agentSettings.visualization?.node_size || 1.0}
                onChange={(e) => updateSetting('agents.visualization.node_size', parseFloat(e.target.value))}
                min={0.5}
                max={3.0}
                step={0.1}
                className="flex-1 ml-2"
              />
              <span className="cc-value-readout ml-2 w-12">
                {(agentSettings.visualization?.node_size || 1.0).toFixed(1)}
              </span>
            </div>

            <div className="flex items-center justify-between">
              <label className="cc-field-label">Agent Node Color</label>
              <input
                type="color"
                value={agentSettings.visualization?.node_color || '#ff8800'}
                onChange={(e) => updateSetting('agents.visualization.node_color', e.target.value)}
                className="w-20 h-8 rounded cursor-pointer"
              />
            </div>
          </div>
        </div>
      </div>

      {}
      {agentSettings.monitoring?.telemetry_enabled && (
        <div className="border rounded-lg p-4 bg-card">
          <h3 className="cc-title mb-3 flex items-center gap-2">
            <Activity className="w-4 h-4" />
            Agent Telemetry
          </h3>
          <AgentTelemetryStream />
        </div>
      )}
        </>
      )}
    </div>
  );
};
