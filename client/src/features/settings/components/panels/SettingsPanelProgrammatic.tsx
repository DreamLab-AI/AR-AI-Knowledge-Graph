import React, { useState, useMemo, useCallback, useEffect } from 'react';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/features/design-system/components/Tabs';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/features/design-system/components/Card';
import { Button } from '@/features/design-system/components/Button';
import { SearchInput } from '@/features/design-system/components/SearchInput';
import {
  Eye,
  Settings,
  Smartphone,
  Info,
  ChevronDown,
  ChevronUp,
  Check,
  Search,
  Keyboard,
  User,
  Brain,
} from 'lucide-react';
import { useSettingsStore } from '@/store/settingsStore';
import { SettingControlComponent } from '../SettingControlComponent';
import { settingsUIDefinition, UICategoryDefinition, UISettingDefinition } from '../../config/settingsUIDefinition';
import { cn } from '@/utils/classNameUtils';
import { useKeyboardShortcuts } from '@/hooks/useKeyboardShortcuts';
import { KeyboardShortcutsModal } from '@/components/KeyboardShortcutsModal';
import { LoadingSpinner, LoadingOverlay } from '@/features/design-system/components/LoadingSpinner';
import { SkeletonSetting } from '@/features/design-system/components/LoadingSkeleton';
import { useErrorHandler } from '@/hooks/useErrorHandler';
import { useToast } from '@/features/design-system/components/Toast';
import { UndoRedoControls } from '../UndoRedoControls';
import NostrAuthSection from '../../../auth/components/NostrAuthSection';
import { useSelectiveSetting, useSettingSetter } from '@/hooks/useSelectiveSettingsStore';
import { VirtualizedSettingsGroup } from '../VirtualizedSettingsGroup';
import { performanceUtils } from '../../hooks/useSettingsPerformance';

interface SettingItem {
  key: string;
  path: string;
  definition: UISettingDefinition;
  isPowerUser?: boolean;
}

interface SettingGroup {
  title: string;
  description?: string;
  items: SettingItem[];
  isPowerUser?: boolean;
}

interface TabConfig {
  label: string;
  icon: React.ReactNode;
  isPowerUser?: boolean;
  groups: SettingGroup[];
}

interface SettingsPanelProgrammaticProps {
  toggleLowerRightPaneDock: () => void;
  isLowerRightPaneDocked: boolean;
}

// Icon mapping for categories
const iconMap: Record<string, React.ReactNode> = {
  Eye: <Eye className="h-4 w-4" />,
  Settings: <Settings className="h-4 w-4" />,
  Smartphone: <Smartphone className="h-4 w-4" />,
  User: <User className="h-4 w-4" />,
  Brain: <Brain className="h-4 w-4" />,
};

// Programmatically generate settings structure from settingsUIDefinition
function generateSettingsStructure(definition: Record<string, UICategoryDefinition>): Record<string, TabConfig> {
  const structure: Record<string, TabConfig> = {};

  // Define category to tab mapping with enhanced organization
  const categoryToTab: Record<string, { tab: string; group?: string }> = {
    visualisation: { tab: 'appearance' },
    system: { tab: 'performance', group: 'System Settings' },
    xr: { tab: 'xr' },
    ai: { tab: 'advanced', group: 'AI Services' },
  };

  // Define subsection groupings
  const subsectionGroups: Record<string, Record<string, string>> = {
    visualisation: {
      nodes: 'Node Appearance',
      edges: 'Edge Appearance',
      labels: 'Labels',
      bloom: 'Visual Effects',
      rendering: 'Lighting & Rendering',
      hologram: 'Hologram Effect',
      animations: 'Animations',
      physics: 'Physics Engine',
    },
    system: {
      general: 'General Settings',
      websocket: 'Network Settings',
      debug: 'Debug Options',
    },
    xr: {
      general: 'XR Mode',
      handFeatures: 'Interaction',
      environmentUnderstanding: 'Environment Understanding',
      passthrough: 'Passthrough',
    },
    ai: {
      ragflow: 'AI Services',
      perplexity: 'AI Services',
      openai: 'AI Services',
    },
  };

  // Initialize tabs
  structure.appearance = {
    label: 'Appearance',
    icon: iconMap.Eye,
    groups: [],
  };
  structure.performance = {
    label: 'Performance',
    icon: iconMap.Settings,
    groups: [],
  };
  structure.xr = {
    label: 'XR/VR',
    icon: iconMap.Smartphone,
    groups: [],
  };
  structure.auth = {
    label: 'Authentication',
    icon: iconMap.User,
    groups: [
      {
        title: 'Nostr Authentication',
        description: 'Authenticate to access advanced features.',
        items: [],
      },
    ],
  };
  structure.advanced = {
    label: 'Advanced',
    icon: iconMap.Settings,
    isPowerUser: true,
    groups: [],
  };

  // Process each category
  Object.entries(definition).forEach(([categoryKey, category]) => {
    const tabMapping = categoryToTab[categoryKey];
    if (!tabMapping) return;

    const { tab, group: defaultGroupName } = tabMapping;
    const groupings = subsectionGroups[categoryKey] || {};

    // Process each subsection
    Object.entries(category.subsections).forEach(([subsectionKey, subsection]) => {
      const groupName = groupings[subsectionKey] || defaultGroupName || subsection.label;
      
      // Find or create group
      let group = structure[tab].groups.find(g => g.title === groupName);
      if (!group) {
        group = {
          title: groupName,
          description: getGroupDescription(categoryKey, subsectionKey, groupName),
          items: [],
          isPowerUser: isGroupPowerUser(categoryKey, subsectionKey),
        };
        structure[tab].groups.push(group);
      }

      // Add settings to group
      Object.entries(subsection.settings).forEach(([settingKey, settingDef]) => {
        group!.items.push({
          key: settingKey,
          path: settingDef.path,
          definition: settingDef,
          isPowerUser: settingDef.isPowerUserOnly,
        });
      });
    });
  });

  // Sort groups by importance
  Object.values(structure).forEach(tab => {
    tab.groups.sort((a, b) => {
      const order = [
        'Node Appearance',
        'Edge Appearance',
        'Labels',
        'Visual Effects',
        'Lighting & Rendering',
        'Rendering Quality',
        'Physics Engine',
        'Force Settings',
        'XR Mode',
        'Interaction',
        'General Settings',
        'Network Settings',
        'Debug Options',
        'AI Services',
      ];
      return order.indexOf(a.title) - order.indexOf(b.title);
    });
  });

  return structure;
}

// Helper function to get group descriptions
function getGroupDescription(category: string, subsection: string, groupName: string): string {
  const descriptions: Record<string, string> = {
    'Node Appearance': 'Customize how nodes look',
    'Edge Appearance': 'Customize connection lines',
    'Labels': 'Text display settings',
    'Visual Effects': 'Bloom and glow effects',
    'Lighting & Rendering': 'Control lighting and background',
    'Hologram Effect': 'Control hologram visualization',
    'Animations': 'Animation controls',
    'Rendering Quality': 'Balance quality and performance',
    'Physics Engine': 'Node movement behavior',
    'Force Settings': 'Control attraction, repulsion, and spring forces',
    'XR Mode': 'Virtual reality settings',
    'Interaction': 'Hand tracking and controls',
    'Environment Understanding': 'Settings for AR environment features',
    'Passthrough': 'Control passthrough portal settings',
    'Network Settings': 'Connection optimization',
    'Debug Options': 'Developer tools',
    'AI Services': 'API configuration',
  };
  return descriptions[groupName] || '';
}

// Helper function to determine if a group is power user only
function isGroupPowerUser(category: string, subsection: string): boolean {
  const powerUserGroups = [
    'system.websocket',
    'system.debug',
    'xr.handFeatures',
    'xr.environmentUnderstanding',
    'xr.passthrough',
  ];
  return powerUserGroups.includes(`${category}.${subsection}`);
}

export function SettingsPanelProgrammatic({ 
  toggleLowerRightPaneDock, 
  isLowerRightPaneDocked 
}: SettingsPanelProgrammaticProps) {
  const { isPowerUser } = useSettingsStore();
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set(['Node Appearance']));
  const [savedNotification, setSavedNotification] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState('');
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [loadingSettings, setLoadingSettings] = useState<Set<string>>(new Set());
  const [isInitializing, setIsInitializing] = useState(true);
  const [isSearching, setIsSearching] = useState(false);
  const { handleError } = useErrorHandler();
  const { toast } = useToast();
  const { set: setSetting } = useSettingSetter();

  // Generate settings structure programmatically with memoization
  const settingsStructure = useMemo(
    () => generateSettingsStructure(settingsUIDefinition),
    []
  );

  // Optimized filter function with memoization
  const filterSettings = useMemo(
    () => performanceUtils.memoizeComputation((groups: SettingGroup[], query: string): SettingGroup[] => {
      if (!query.trim()) return groups;

      const lowerQuery = query.toLowerCase();
      return groups
        .map(group => {
          const filteredItems = group.items.filter(item => {
            const matchesKey = item.key.toLowerCase().includes(lowerQuery);
            const matchesLabel = item.definition?.label?.toLowerCase().includes(lowerQuery);
            const matchesDescription = item.definition?.description?.toLowerCase().includes(lowerQuery);
            const matchesGroup = group.title.toLowerCase().includes(lowerQuery);
            const matchesGroupDesc = group.description?.toLowerCase().includes(lowerQuery);

            return matchesKey || matchesLabel || matchesDescription || matchesGroup || matchesGroupDesc;
          });

          if (filteredItems.length > 0) {
            return { ...group, items: filteredItems };
          }
          return null;
        })
        .filter(group => group !== null) as SettingGroup[];
    }, 20), // Cache last 20 search results
    []
  );

  // Debounce search query
  useEffect(() => {
    setIsSearching(true);
    const timer = setTimeout(() => {
      setDebouncedSearchQuery(searchQuery);
      setIsSearching(false);
    }, 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  // Auto-expand groups when searching
  useEffect(() => {
    if (debouncedSearchQuery.trim()) {
      const groupsToExpand = new Set<string>();
      Object.values(settingsStructure).forEach(section => {
        const filtered = filterSettings(section.groups, debouncedSearchQuery);
        filtered.forEach(group => {
          groupsToExpand.add(group.title);
        });
      });
      setExpandedGroups(groupsToExpand);
    }
  }, [debouncedSearchQuery, settingsStructure, filterSettings]);

  // Register keyboard shortcuts
  useKeyboardShortcuts({
    'settings-search': {
      key: '/',
      ctrl: true,
      description: 'Focus search in settings',
      category: 'Settings',
      handler: () => {
        const searchInput = document.querySelector('input[placeholder="Search settings..."]') as HTMLInputElement;
        searchInput?.focus();
      }
    },
    'settings-shortcuts': {
      key: '?',
      shift: true,
      description: 'Show keyboard shortcuts',
      category: 'General',
      handler: () => setShowShortcuts(true)
    },
    'settings-clear-search': {
      key: 'Escape',
      description: 'Clear search',
      category: 'Settings',
      handler: () => setSearchQuery(''),
      enabled: !!searchQuery
    },
  });

  // Simulate initial loading
  useEffect(() => {
    const timer = setTimeout(() => {
      setIsInitializing(false);
    }, 300);
    return () => clearTimeout(timer);
  }, []);

  const toggleGroup = useCallback((groupTitle: string) => {
    setExpandedGroups(prev => {
      const next = new Set(prev);
      if (next.has(groupTitle)) {
        next.delete(groupTitle);
      } else {
        next.add(groupTitle);
      }
      return next;
    });
  }, []);

  const handleSettingChange = useCallback(async (path: string, value: any) => {
    setLoadingSettings(prev => new Set(prev).add(path));

    try {
      setSetting(path, value);

      toast({
        title: 'Setting saved',
        description: `Successfully updated ${path.split('.').pop()}`,
      });

      setSavedNotification(path);
      setTimeout(() => setSavedNotification(null), 2000);
    } catch (error) {
      handleError(error, {
        title: 'Failed to save setting',
        actionLabel: 'Retry',
        onAction: () => handleSettingChange(path, value)
      });
    } finally {
      setLoadingSettings(prev => {
        const next = new Set(prev);
        next.delete(path);
        return next;
      });
    }
  }, [setSetting, toast, handleError]);

  const renderSettingGroup = useCallback((group: SettingGroup, groupIndex: number) => {
    if (group.isPowerUser && !isPowerUser) return null;

    return (
      <VirtualizedSettingsGroup
        key={group.title}
        title={group.title}
        description={group.description}
        items={group.items}
        isExpanded={expandedGroups.has(group.title)}
        onToggle={() => toggleGroup(group.title)}
        isPowerUser={isPowerUser}
        loadingSettings={loadingSettings}
        savedNotification={savedNotification}
        onSettingChange={handleSettingChange}
        groupIndex={groupIndex}
      />
    );
  }, [isPowerUser, expandedGroups, toggleGroup, loadingSettings, savedNotification, handleSettingChange]);

  const renderTabContent = useCallback((tabKey: string) => {
    const tab = settingsStructure[tabKey];
    if (!tab) return null;

    if (tab.isPowerUser && !isPowerUser) {
      return (
        <div className="flex flex-col items-center justify-center h-64 text-center p-6">
          <Settings className="h-12 w-12 text-muted-foreground mb-4" />
          <h3 className="text-lg font-medium mb-2">Power User Features</h3>
          <p className="text-sm text-muted-foreground max-w-sm">
            Authenticate with Nostr to unlock advanced settings and features.
          </p>
        </div>
      );
    }

    if (tabKey === 'auth') {
      return <NostrAuthSection />;
    }

    const filteredGroups = filterSettings(tab.groups, debouncedSearchQuery);

    if (debouncedSearchQuery && filteredGroups.length === 0) {
      return (
        <div className="flex flex-col items-center justify-center h-64 text-center p-6">
          <Search className="h-12 w-12 text-muted-foreground mb-4" />
          <h3 className="text-lg font-medium mb-2">No results found</h3>
          <p className="text-sm text-muted-foreground max-w-sm">
            Try searching with different keywords or browse categories.
          </p>
        </div>
      );
    }

    if (isInitializing) {
      return (
        <div className="space-y-3">
          {[1, 2, 3].map(i => (
            <Card key={i} className="overflow-hidden">
              <CardContent className="p-0">
                {[1, 2, 3].map(j => (
                  <SkeletonSetting key={j} />
                ))}
              </CardContent>
            </Card>
          ))}
        </div>
      );
    }

    return (
      <div className="flex-1 min-h-0 space-y-3">
        {filteredGroups.map((group, index) => renderSettingGroup(group, index))}
      </div>
    );
  }, [settingsStructure, isPowerUser, searchQuery, filterSettings, isInitializing, renderSettingGroup]);

  return (
    <div className="w-full h-full flex flex-col min-h-0 bg-background text-foreground">
      <div className="border-b border-border">
        <div className="px-4 py-3 flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold">Settings & Controls</h2>
            <p className="text-sm text-muted-foreground">
              Customize your visualization and experience
            </p>
          </div>
          <div className="flex items-center gap-2">
            <UndoRedoControls showHistory />
            <div className="w-px h-6 bg-border" />
            <Button
              variant="ghost"
              size="icon"
              onClick={toggleLowerRightPaneDock}
              title={isLowerRightPaneDocked ? "Expand lower panels" : "Collapse lower panels"}
            >
              {isLowerRightPaneDocked ? <ChevronDown className="h-4 w-4" /> : <ChevronUp className="h-4 w-4" />}
            </Button>
          </div>
        </div>
        <div className="px-4 pb-3">
          <div className="relative">
            <SearchInput
              value={searchQuery}
              onChange={setSearchQuery}
              placeholder="Search settings..."
              className="w-full"
              onKeyDown={(e) => {
                if (e.key === 'Escape') {
                  setSearchQuery('');
                }
              }}
            />
            {isSearching && (
              <div className="absolute right-3 top-1/2 -translate-y-1/2">
                <LoadingSpinner size="sm" />
              </div>
            )}
          </div>
          <div className="mt-2 flex items-center justify-between text-xs text-muted-foreground">
            <span>Press <kbd className="px-1 py-0.5 bg-muted rounded text-xs">Ctrl+/</kbd> to search</span>
            <button
              onClick={() => setShowShortcuts(true)}
              className="flex items-center gap-1 hover:text-foreground transition-colors"
            >
              <Keyboard className="h-3 w-3" />
              <span>View shortcuts</span>
            </button>
          </div>
        </div>
      </div>

      <div className="flex-1 overflow-auto">
        <Tabs defaultValue="appearance" className="h-full">
          <TabsList className="px-4 bg-muted/30 w-full justify-start">
            {Object.entries(settingsStructure).map(([key, section]) => (
              <TabsTrigger key={key} value={key} className="flex items-center gap-2">
                {section.icon}
                {section.label}
              </TabsTrigger>
            ))}
          </TabsList>
          {Object.entries(settingsStructure).map(([key]) => (
            <TabsContent key={key} value={key} className="px-4 py-3">
              {renderTabContent(key)}
            </TabsContent>
          ))}
        </Tabs>
      </div>

      <div className="px-4 py-2 border-t border-border bg-muted/30 flex items-center justify-between text-xs">
        <div className="flex items-center gap-2">
          <Info className="h-3 w-3" />
          <span>Changes save automatically</span>
        </div>
        {isPowerUser && (
          <div className="flex items-center gap-1 text-primary">
            <Settings className="h-3 w-3" />
            <span>Power User</span>
          </div>
        )}
      </div>

      <KeyboardShortcutsModal
        isOpen={showShortcuts}
        onClose={() => setShowShortcuts(false)}
      />
    </div>
  );
}