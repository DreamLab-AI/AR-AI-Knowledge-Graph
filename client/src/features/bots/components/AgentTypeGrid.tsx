import React from 'react';
import { cn } from '../../../utils/classNameUtils';

const AGENT_TYPE_INFO: Record<string, { icon: string; description: string }> = {
  queen: { icon: '👑', description: 'Hive mind leader' },
  coordinator: { icon: '🎯', description: 'Task orchestration' },
  researcher: { icon: '🔍', description: 'Information gathering' },
  coder: { icon: '💻', description: 'Code implementation' },
  analyst: { icon: '📊', description: 'Data analysis' },
  tester: { icon: '🧪', description: 'Quality assurance' },
  architect: { icon: '🏗️', description: 'System design' },
  optimizer: { icon: '⚡', description: 'Performance tuning' },
  reviewer: { icon: '👁️', description: 'Code review' },
  documenter: { icon: '📝', description: 'Documentation' },
  monitor: { icon: '📡', description: 'System monitoring' },
  specialist: { icon: '🔧', description: 'Specialized tasks' },
};

export interface AgentTypeGridProps {
  agentTypes: Record<string, boolean>;
  onToggle: (type: string, enabled: boolean) => void;
  topology: string;
}

/** Checkbox grid for the hive-mind agent-type roster, plus the hierarchical/queen hint. */
export const AgentTypeGrid: React.FC<AgentTypeGridProps> = ({ agentTypes, onToggle, topology }) => {
  return (
    <div className="mb-4">
      <span className="cc-field-label mb-1.5 block">Agent Types</span>
      <div className="grid grid-cols-2 gap-1.5">
        {Object.entries(agentTypes).map(([type, enabled]) => {
          const info = AGENT_TYPE_INFO[type] || { icon: '🤖', description: type };

          return (
            <label
              key={type}
              title={info.description}
              className={cn(
                'flex items-center gap-1.5 rounded-[calc(var(--radius)-2px)] px-2 py-1.5 text-xs cursor-pointer transition-colors',
                enabled ? 'bg-amber-400/10 text-amber-400' : 'text-muted-foreground hover:bg-foreground/5',
              )}
            >
              <input
                type="checkbox"
                checked={enabled}
                onChange={(e) => onToggle(type, e.target.checked)}
                className="accent-amber-400"
              />
              <span aria-hidden="true">{info.icon}</span>
              <span>{type.charAt(0).toUpperCase() + type.slice(1)}</span>
            </label>
          );
        })}
      </div>

      {topology === 'hierarchical' && !agentTypes.queen && (
        <p className="cc-helper-text mt-2 text-amber-500">
          Tip: enable the Queen agent for hierarchical topology
        </p>
      )}
    </div>
  );
};

export default AgentTypeGrid;
