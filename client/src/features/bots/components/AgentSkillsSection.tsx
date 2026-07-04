import React from 'react';
import { Zap, ChevronDown, ChevronRight, Search } from 'lucide-react';
import { cn } from '../../../utils/classNameUtils';
import { Button } from '../../design-system/components/Button';
import { Input } from '../../design-system/components/Input';
import { Badge } from '../../design-system/components/Badge';
import {
  categoryLabels,
  categoryIcons,
  type SkillDefinition,
} from '../../settings/components/panels/skillDefinitions';

export interface AgentSkillsSectionProps {
  showSkills: boolean;
  onToggleShow: () => void;
  selectedSkills: Set<string>;
  onToggleSkill: (skillId: string) => void;
  searchQuery: string;
  onSearchChange: (value: string) => void;
  skillsByCategory: Record<string, SkillDefinition[]>;
  filteredCount: number;
}

/** Collapsible skill picker (search + category groups) for the hive-mind spawn config. */
export const AgentSkillsSection: React.FC<AgentSkillsSectionProps> = ({
  showSkills,
  onToggleShow,
  selectedSkills,
  onToggleSkill,
  searchQuery,
  onSearchChange,
  skillsByCategory,
  filteredCount,
}) => {
  return (
    <div className="mb-4">
      <Button
        type="button"
        variant="outline"
        onClick={onToggleShow}
        className={cn(
          'flex h-auto w-full items-center justify-between px-3 py-2 text-xs',
          selectedSkills.size > 0 && 'border-amber-400/50 bg-amber-400/10 text-amber-400',
        )}
      >
        <span className="flex items-center gap-2">
          <Zap size={14} aria-hidden="true" />
          Skills
          {selectedSkills.size > 0 && (
            <Badge className="bg-amber-400 text-black hover:bg-amber-400">
              {selectedSkills.size} selected
            </Badge>
          )}
        </span>
        {showSkills ? <ChevronDown size={14} aria-hidden="true" /> : <ChevronRight size={14} aria-hidden="true" />}
      </Button>

      {showSkills && (
        <div className="mt-2 max-h-[300px] overflow-auto rounded-[calc(var(--radius)-2px)] bg-foreground/5 p-2">
          <div className="relative mb-2">
            <Search size={12} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
              placeholder="Search skills..."
              size="sm"
              className="pl-7 text-xs"
            />
          </div>

          {Object.entries(skillsByCategory).map(([category, skills]) => (
            <div key={category} className="mb-2">
              <div className="cc-subgroup-label mb-1">
                {categoryIcons[category]} {categoryLabels[category]}
              </div>
              <div className="grid grid-cols-2 gap-1">
                {skills.map((skill) => (
                  <label
                    key={skill.id}
                    title={skill.description}
                    className={cn(
                      'flex cursor-pointer items-center gap-1.5 truncate rounded px-1.5 py-1 text-xs',
                      selectedSkills.has(skill.id)
                        ? 'bg-amber-400/10 text-amber-400'
                        : 'text-muted-foreground hover:bg-foreground/5',
                    )}
                  >
                    <input
                      type="checkbox"
                      checked={selectedSkills.has(skill.id)}
                      onChange={() => onToggleSkill(skill.id)}
                      className="accent-amber-400"
                    />
                    <span aria-hidden="true">{skill.icon}</span>
                    <span className="truncate">{skill.name}</span>
                    {skill.mcpServer && (
                      <Badge variant="success" className="ml-auto shrink-0 px-1 py-0 text-[9px]">
                        MCP
                      </Badge>
                    )}
                  </label>
                ))}
              </div>
            </div>
          ))}

          {filteredCount === 0 && (
            <div className="cc-helper-text py-4 text-center">No skills match your search</div>
          )}
        </div>
      )}
    </div>
  );
};

export default AgentSkillsSection;
