export interface Command {
  id: string;
  title: string;
  description?: string;
  category: string;
  keywords?: string[];
  icon?: React.ComponentType<{ className?: string }>;
  shortcut?: {
    key: string;
    ctrl?: boolean;
    alt?: boolean;
    shift?: boolean;
    meta?: boolean;
  };
  handler: () => void | Promise<void>;
  enabled?: boolean;
}

export interface CommandCategory {
  id: string;
  name: string;
  icon?: React.ComponentType<{ className?: string }>;
  priority?: number;
}

export interface CommandPaletteState {
  isOpen: boolean;
  searchQuery: string;
  selectedIndex: number;
  filteredCommands: Command[];
  recentCommands: string[];
}

export interface CommandRegistryOptions {
  maxRecentCommands?: number;
  fuzzySearchThreshold?: number;
}