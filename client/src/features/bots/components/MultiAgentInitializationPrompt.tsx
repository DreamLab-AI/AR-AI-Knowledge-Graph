

import React, { useState, useEffect, useMemo } from 'react';
import ReactDOM from 'react-dom';
import { Bot, X, AlertTriangle } from 'lucide-react';
import { createLogger } from '../../../utils/loggerConfig';
import { unifiedApiClient } from '../../../services/api/UnifiedApiClient';
import { Button } from '../../design-system/components/Button';
import { Textarea } from '../../design-system/components/Textarea';
import { skillDefinitions, type SkillDefinition } from '../../settings/components/panels/skillDefinitions';
import { AgentTypeGrid } from './AgentTypeGrid';
import { AgentSkillsSection } from './AgentSkillsSection';
import { AgentTopologyFields, type Topology } from './AgentTopologyFields';

const logger = createLogger('MultiAgentInitializationPrompt');

interface MultiAgentInitializationPromptProps {
  onClose: () => void;
  onInitialized: () => void;
}

export const MultiAgentInitializationPrompt: React.FC<MultiAgentInitializationPromptProps> = ({
  onClose,
  onInitialized
}) => {
  const [isLoading, setIsLoading] = useState(false);
  const [mcpConnected, setMcpConnected] = useState<boolean | null>(null);
  const [topology, setTopology] = useState<Topology>('mesh');
  const [maxAgents, setMaxAgents] = useState(8);
  const [enableNeural, setEnableNeural] = useState(true);
  const [agentTypes, setAgentTypes] = useState({
    queen: false,
    coordinator: true,
    researcher: true,
    coder: true,
    analyst: true,
    tester: true,
    architect: true,
    optimizer: true,
    reviewer: false,
    documenter: false,
    monitor: false,
    specialist: false,
  });
  const [customPrompt, setCustomPrompt] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [portalContainer, setPortalContainer] = useState<HTMLElement | null>(null);
  const [selectedSkills, setSelectedSkills] = useState<Set<string>>(new Set());
  const [skillSearchQuery, setSkillSearchQuery] = useState('');
  const [showSkills, setShowSkills] = useState(false);

  // Filter skills based on search
  const filteredSkills = useMemo(() => {
    if (!skillSearchQuery) return skillDefinitions;
    const query = skillSearchQuery.toLowerCase();
    return skillDefinitions.filter(
      (skill) =>
        skill.name.toLowerCase().includes(query) ||
        skill.description.toLowerCase().includes(query) ||
        skill.tags.some((tag) => tag.toLowerCase().includes(query))
    );
  }, [skillSearchQuery]);

  // Group skills by category
  const skillsByCategory = useMemo(() => {
    const categories: Record<string, SkillDefinition[]> = {};
    for (const skill of filteredSkills) {
      if (!categories[skill.category]) {
        categories[skill.category] = [];
      }
      categories[skill.category].push(skill);
    }
    return categories;
  }, [filteredSkills]);

  const toggleSkill = (skillId: string) => {
    setSelectedSkills((prev) => {
      const next = new Set(prev);
      if (next.has(skillId)) {
        next.delete(skillId);
      } else {
        next.add(skillId);
      }
      return next;
    });
  };

  const toggleAgentType = (type: string, enabled: boolean) => {
    setAgentTypes((prev) => ({ ...prev, [type]: enabled }));
  };


  useEffect(() => {
    const checkConnection = async () => {
      try {
        const response = await unifiedApiClient.getData('/bots/status');
        // API returns { success: true, data: { connected: true, ... } }
        setMcpConnected(response?.data?.connected ?? response?.connected ?? false);
      } catch (error) {
        setMcpConnected(false);
      }
    };


    checkConnection();


    const interval = setInterval(checkConnection, 3000);

    return () => clearInterval(interval);
  }, []);

  useEffect(() => {

    const container = document.createElement('div');
    container.id = 'multi-agent-modal-portal';
    container.style.position = 'fixed';
    container.style.top = '0';
    container.style.left = '0';
    container.style.width = '100%';
    container.style.height = '100%';
    container.style.zIndex = '9999999';
    container.style.pointerEvents = 'none';
    document.body.appendChild(container);
    setPortalContainer(container);

    return () => {
      document.body.removeChild(container);
    };
  }, []);

  const handleInitialize = async () => {
    setIsLoading(true);
    setError(null);

    try {

      const selectedAgentTypes = Object.entries(agentTypes)
        .filter(([_, enabled]) => enabled)
        .map(([type, _]) => type);

      if (selectedAgentTypes.length === 0) {
        setError('Please select at least one agent type');
        setIsLoading(false);
        return;
      }

      if (!customPrompt.trim()) {
        setError('Please provide a task for the hive mind');
        setIsLoading(false);
        return;
      }


      const config = {
        topology,
        maxAgents,
        strategy: 'adaptive',
        enableNeural,
        agentTypes: selectedAgentTypes,
        skills: Array.from(selectedSkills),
        customPrompt: customPrompt.trim(),
      };

      logger.info('Spawning hive mind with config:', config);


      logger.info('Calling API endpoint: /bots/initialize-swarm');
      const response = await unifiedApiClient.postData('/bots/initialize-swarm', config);

      if (response.success) {
        logger.info('Hive mind spawned successfully:', response);



        onInitialized();
      } else {
        throw new Error(response.error || 'Failed to spawn hive mind');
      }
    } catch (err) {
      logger.error('Failed to spawn hive mind:', err);
      setError(err instanceof Error ? err.message : 'Failed to spawn hive mind');
    } finally {
      setIsLoading(false);
    }
  };

  if (!portalContainer) return null;

  const mcpDotClass =
    mcpConnected === null ? 'bg-muted-foreground animate-pulse' : mcpConnected ? 'bg-green-500' : 'bg-destructive';
  const mcpTextClass =
    mcpConnected === null ? 'text-muted-foreground' : mcpConnected ? 'text-green-500' : 'text-destructive';

  return ReactDOM.createPortal(
    <div className="fixed inset-0 flex items-center justify-center bg-background/70 backdrop-blur-sm pointer-events-auto">
      <div
        data-testid="multi-agent-prompt"
        className="cc-glass cc-glass--accent w-[90%] max-w-lg max-h-[80vh] overflow-auto p-5"
      >
        <div className="mb-4 flex items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <span className="cc-title flex items-center gap-2 whitespace-nowrap">
              <Bot size={16} className="text-amber-400" aria-hidden="true" />
              Spawn Hive Mind
            </span>
            <span className={`flex items-center gap-1.5 text-xs whitespace-nowrap ${mcpTextClass}`}>
              <span className={`h-2 w-2 shrink-0 rounded-full ${mcpDotClass}`} aria-hidden="true" />
              {mcpConnected === null ? 'Checking...' : mcpConnected ? 'MCP Connected' : 'MCP Disconnected'}
            </span>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close"
            className="shrink-0 text-muted-foreground transition-colors hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-sm"
          >
            <X size={18} />
          </button>
        </div>

        {error && (
          <div className="mb-4 rounded-[calc(var(--radius)-2px)] border border-destructive/40 bg-destructive/10 p-2.5 text-xs text-destructive">
            {error}
          </div>
        )}

        {mcpConnected === false && (
          <div className="mb-4 flex items-center gap-2 rounded-[calc(var(--radius)-2px)] border border-amber-500/40 bg-amber-500/10 p-2.5 text-xs text-amber-500">
            <AlertTriangle size={14} className="shrink-0" aria-hidden="true" />
            MCP service is not connected. The multi-agent system may not initialize properly.
          </div>
        )}

        <AgentTopologyFields
          topology={topology}
          onTopologyChange={setTopology}
          maxAgents={maxAgents}
          onMaxAgentsChange={setMaxAgents}
          enableNeural={enableNeural}
          onEnableNeuralChange={setEnableNeural}
        />

        <AgentTypeGrid agentTypes={agentTypes} onToggle={toggleAgentType} topology={topology} />

        <AgentSkillsSection
          showSkills={showSkills}
          onToggleShow={() => setShowSkills((v) => !v)}
          selectedSkills={selectedSkills}
          onToggleSkill={toggleSkill}
          searchQuery={skillSearchQuery}
          onSearchChange={setSkillSearchQuery}
          skillsByCategory={skillsByCategory}
          filteredCount={filteredSkills.length}
        />

        <div className="mb-4">
          <span className="cc-field-label mb-1.5 block">
            Task for Hive Mind <span className="text-destructive">*</span>
          </span>
          <Textarea
            value={customPrompt}
            onChange={(e) => setCustomPrompt(e.target.value)}
            placeholder="Describe the task for the hive mind to accomplish..."
            required
            className="min-h-[100px] resize-y text-xs"
          />
          <p className="cc-helper-text mt-1">
            Example: "Build a REST API with user authentication and database integration"
          </p>
        </div>

        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose} disabled={isLoading}>
            Cancel
          </Button>
          <Button variant="default" onClick={handleInitialize} loading={isLoading} loadingText="Spawning...">
            Spawn Hive Mind
          </Button>
        </div>
      </div>
    </div>,
    portalContainer
  );
};
