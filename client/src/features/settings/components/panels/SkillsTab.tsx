/**
 * SkillsTab Component
 *
 * Displays available skills from multi-agent-docker/skills with checkbox-style
 * selection for explicit invocation. Integrates with the Agent Control Panel.
 */

import React, { useState, useMemo, useCallback } from 'react';
import { Zap, Search, CheckCircle, XCircle, Play, Settings2, Filter, ChevronDown, ChevronRight } from 'lucide-react';
import { Button } from '../../../design-system/components/Button';
import {
  skillDefinitions,
  getSkillsByCategory,
  categoryLabels,
  categoryIcons,
  SkillDefinition,
} from './skillDefinitions';
import { unifiedApiClient } from '../../../../services/api/UnifiedApiClient';
import { createLogger } from '../../../../utils/loggerConfig';

const logger = createLogger('SkillsTab');

interface SkillsTabProps {
  className?: string;
  /** Callback when skills are invoked */
  onSkillInvoke?: (skillIds: string[]) => void;
}

export const SkillsTab: React.FC<SkillsTabProps> = ({ className, onSkillInvoke }) => {
  // Selected skills for invocation
  const [selectedSkills, setSelectedSkills] = useState<Set<string>>(new Set());
  // Search filter
  const [searchQuery, setSearchQuery] = useState('');
  // Category filter
  const [categoryFilter, setCategoryFilter] = useState<string | null>(null);
  // MCP-only filter
  const [mcpOnly, setMcpOnly] = useState(false);
  // Collapsed categories
  const [collapsedCategories, setCollapsedCategories] = useState<Set<string>>(new Set());
  // Invocation state
  const [invoking, setInvoking] = useState(false);
  // Show settings panel
  const [showSettings, setShowSettings] = useState(false);

  // Filter skills based on search and filters
  const filteredSkills = useMemo(() => {
    return skillDefinitions.filter((skill) => {
      // Search filter
      if (searchQuery) {
        const query = searchQuery.toLowerCase();
        const matchesSearch =
          skill.name.toLowerCase().includes(query) ||
          skill.description.toLowerCase().includes(query) ||
          skill.tags.some((tag) => tag.toLowerCase().includes(query));
        if (!matchesSearch) return false;
      }

      // Category filter
      if (categoryFilter && skill.category !== categoryFilter) {
        return false;
      }

      // MCP-only filter
      if (mcpOnly && !skill.mcpServer) {
        return false;
      }

      return true;
    });
  }, [searchQuery, categoryFilter, mcpOnly]);

  // Group filtered skills by category
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

  // Toggle skill selection
  const toggleSkill = useCallback((skillId: string) => {
    setSelectedSkills((prev) => {
      const next = new Set(prev);
      if (next.has(skillId)) {
        next.delete(skillId);
      } else {
        next.add(skillId);
      }
      return next;
    });
  }, []);

  // Toggle category collapse
  const toggleCategory = useCallback((category: string) => {
    setCollapsedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(category)) {
        next.delete(category);
      } else {
        next.add(category);
      }
      return next;
    });
  }, []);

  // Select all skills in a category
  const selectCategory = useCallback((category: string, skills: SkillDefinition[]) => {
    setSelectedSkills((prev) => {
      const next = new Set(prev);
      const allSelected = skills.every((s) => prev.has(s.id));

      if (allSelected) {
        // Deselect all
        skills.forEach((s) => next.delete(s.id));
      } else {
        // Select all
        skills.forEach((s) => next.add(s.id));
      }
      return next;
    });
  }, []);

  // Invoke selected skills
  const invokeSkills = useCallback(async () => {
    if (selectedSkills.size === 0) return;

    setInvoking(true);
    const skillIds = Array.from(selectedSkills);

    try {
      logger.info('Invoking skills:', skillIds);

      // Call backend to invoke skills
      await unifiedApiClient.post('/bots/invoke-skills', {
        skills: skillIds,
        config: {
          parallel: true,
          timeout: 30000,
        },
      });

      // Notify parent
      onSkillInvoke?.(skillIds);

      // Clear selection after successful invocation
      setSelectedSkills(new Set());
    } catch (error) {
      logger.error('Failed to invoke skills:', error);
    } finally {
      setInvoking(false);
    }
  }, [selectedSkills, onSkillInvoke]);

  // Clear all selections
  const clearSelection = useCallback(() => {
    setSelectedSkills(new Set());
  }, []);

  return (
    <div className={`space-y-4 ${className}`}>
      {/* Header */}
      <div className="border rounded-lg p-4 bg-card">
        <div className="flex items-center justify-between mb-3">
          <h3 className="cc-title flex items-center gap-2">
            <Zap className="w-4 h-4" />
            Skills Invocation
          </h3>
          <div className="flex items-center gap-2">
            <span className="cc-value-readout">
              {selectedSkills.size} selected
            </span>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowSettings(!showSettings)}
            >
              <Settings2 className="w-4 h-4" />
            </Button>
          </div>
        </div>

        {/* Search and Filters */}
        <div className="space-y-2">
          <div className="relative">
            <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <input
              type="text"
              placeholder="Search skills..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-8 pr-3 py-2 text-xs border rounded bg-background"
            />
          </div>

          {/* Quick filters */}
          <div className="flex items-center gap-2 flex-wrap">
            <Button
              variant={categoryFilter === null ? 'default' : 'ghost'}
              size="sm"
              onClick={() => setCategoryFilter(null)}
              className="text-xs"
            >
              All
            </Button>
            {Object.keys(categoryLabels).map((cat) => (
              <Button
                key={cat}
                variant={categoryFilter === cat ? 'default' : 'ghost'}
                size="sm"
                onClick={() => setCategoryFilter(categoryFilter === cat ? null : cat)}
                className="text-xs"
              >
                {categoryIcons[cat]} {categoryLabels[cat]}
              </Button>
            ))}
          </div>

          {/* Advanced filters */}
          {showSettings && (
            <div className="flex items-center gap-4 pt-2 border-t">
              <label className="flex items-center gap-2 cc-field-label">
                <input
                  type="checkbox"
                  checked={mcpOnly}
                  onChange={(e) => setMcpOnly(e.target.checked)}
                  className="rounded"
                />
                MCP Servers Only
              </label>
            </div>
          )}
        </div>

        {/* Action buttons */}
        <div className="flex items-center justify-between mt-3 pt-3 border-t">
          <div className="cc-value-readout">
            {filteredSkills.length} skills available
          </div>
          <div className="flex items-center gap-2">
            {selectedSkills.size > 0 && (
              <Button variant="ghost" size="sm" onClick={clearSelection}>
                Clear
              </Button>
            )}
            <Button
              onClick={invokeSkills}
              disabled={selectedSkills.size === 0 || invoking}
              size="sm"
              className="flex items-center gap-2"
            >
              <Play className="w-4 h-4" />
              {invoking ? 'Invoking...' : 'Invoke Selected'}
            </Button>
          </div>
        </div>
      </div>

      {/* Skills by Category */}
      {Object.entries(skillsByCategory).map(([category, skills]) => (
        <div key={category} className="border rounded-lg bg-card">
          {/* Category Header */}
          <button
            onClick={() => toggleCategory(category)}
            className="w-full flex items-center justify-between p-3 hover:bg-accent/50 transition-colors"
          >
            <div className="flex items-center gap-2">
              {collapsedCategories.has(category) ? (
                <ChevronRight className="w-4 h-4" />
              ) : (
                <ChevronDown className="w-4 h-4" />
              )}
              <span className="cc-subgroup-label">
                {categoryIcons[category]} {categoryLabels[category]}
              </span>
              <span className="cc-value-readout">
                ({skills.length})
              </span>
            </div>
            <button
              onClick={(e) => {
                e.stopPropagation();
                selectCategory(category, skills);
              }}
              className="text-xs text-primary hover:underline"
            >
              {skills.every((s) => selectedSkills.has(s.id)) ? 'Deselect All' : 'Select All'}
            </button>
          </button>

          {/* Skills List */}
          {!collapsedCategories.has(category) && (
            <div className="px-3 pb-3 space-y-1">
              {skills.map((skill) => (
                <SkillItem
                  key={skill.id}
                  skill={skill}
                  selected={selectedSkills.has(skill.id)}
                  onToggle={() => toggleSkill(skill.id)}
                />
              ))}
            </div>
          )}
        </div>
      ))}

      {/* Empty state */}
      {filteredSkills.length === 0 && (
        <div className="text-center py-8 text-muted-foreground">
          <p className="text-sm">No skills match your search criteria</p>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setSearchQuery('');
              setCategoryFilter(null);
              setMcpOnly(false);
            }}
            className="mt-2"
          >
            Clear Filters
          </Button>
        </div>
      )}
    </div>
  );
};

/**
 * Individual skill item with checkbox
 */
const SkillItem: React.FC<{
  skill: SkillDefinition;
  selected: boolean;
  onToggle: () => void;
}> = ({ skill, selected, onToggle }) => {
  return (
    <div
      onClick={onToggle}
      className={`
        flex items-start gap-3 p-2 rounded cursor-pointer transition-colors
        ${selected ? 'bg-primary/10 border border-primary/30' : 'hover:bg-accent/50'}
      `}
    >
      {/* Checkbox */}
      <div className="pt-0.5">
        {selected ? (
          <CheckCircle className="w-4 h-4 text-primary" />
        ) : (
          <div className="w-4 h-4 border rounded-sm" />
        )}
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-base">{skill.icon}</span>
          <span className="text-sm font-medium">{skill.name}</span>
          <span className="cc-value-readout">v{skill.version}</span>
          {skill.mcpServer && (
            <span className="text-xs px-1.5 py-0.5 bg-green-500/10 text-green-500 rounded">
              MCP
            </span>
          )}
        </div>
        <p className="cc-helper-text mt-0.5 line-clamp-1">
          {skill.description}
        </p>
        <div className="flex flex-wrap gap-1 mt-1">
          {skill.tags.slice(0, 4).map((tag) => (
            <span
              key={tag}
              className="text-xs px-1.5 py-0.5 bg-accent rounded text-muted-foreground"
            >
              {tag}
            </span>
          ))}
          {skill.tags.length > 4 && (
            <span className="cc-helper-text">
              +{skill.tags.length - 4}
            </span>
          )}
        </div>
      </div>
    </div>
  );
};

export default SkillsTab;
