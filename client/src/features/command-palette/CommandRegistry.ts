import { Command, CommandCategory, CommandRegistryOptions } from './types';

export class CommandRegistry {
  private commands: Map<string, Command> = new Map();
  private categories: Map<string, CommandCategory> = new Map();
  private listeners: Set<(commands: Command[]) => void> = new Set();
  private recentCommands: string[] = [];
  private maxRecentCommands: number;
  private fuzzySearchThreshold: number;

  constructor(options: CommandRegistryOptions = {}) {
    this.maxRecentCommands = options.maxRecentCommands || 5;
    this.fuzzySearchThreshold = options.fuzzySearchThreshold || 0.3;
    this.loadRecentCommands();
  }

  // Command registration
  registerCommand(command: Command): void {
    this.commands.set(command.id, command);
    this.notifyListeners();
  }

  registerCommands(commands: Command[]): void {
    commands.forEach(cmd => this.commands.set(cmd.id, cmd));
    this.notifyListeners();
  }

  unregisterCommand(id: string): void {
    this.commands.delete(id);
    this.notifyListeners();
  }

  // Category management
  registerCategory(category: CommandCategory): void {
    this.categories.set(category.id, category);
  }

  getCategories(): CommandCategory[] {
    return Array.from(this.categories.values()).sort((a, b) => 
      (a.priority || 0) - (b.priority || 0)
    );
  }

  // Command retrieval
  getCommand(id: string): Command | undefined {
    return this.commands.get(id);
  }

  getAllCommands(): Command[] {
    return Array.from(this.commands.values());
  }

  getCommandsByCategory(categoryId: string): Command[] {
    return this.getAllCommands().filter(cmd => cmd.category === categoryId);
  }

  // Search functionality
  searchCommands(query: string): Command[] {
    if (!query.trim()) {
      return this.getAllCommands().filter(cmd => cmd.enabled !== false);
    }

    const lowerQuery = query.toLowerCase();
    const results: Array<{ command: Command; score: number }> = [];

    this.commands.forEach(command => {
      if (command.enabled === false) return;

      let score = 0;

      // Exact match in title
      if (command.title.toLowerCase() === lowerQuery) {
        score = 1;
      }
      // Starts with query
      else if (command.title.toLowerCase().startsWith(lowerQuery)) {
        score = 0.8;
      }
      // Contains query
      else if (command.title.toLowerCase().includes(lowerQuery)) {
        score = 0.6;
      }
      // Check description
      else if (command.description?.toLowerCase().includes(lowerQuery)) {
        score = 0.4;
      }
      // Check keywords
      else if (command.keywords?.some(k => k.toLowerCase().includes(lowerQuery))) {
        score = 0.3;
      }

      // Fuzzy match on title
      if (score === 0) {
        score = this.fuzzyMatch(lowerQuery, command.title.toLowerCase());
      }

      if (score > this.fuzzySearchThreshold) {
        results.push({ command, score });
      }
    });

    // Sort by score and return commands
    return results
      .sort((a, b) => b.score - a.score)
      .map(r => r.command);
  }

  // Recent commands
  addRecentCommand(commandId: string): void {
    const index = this.recentCommands.indexOf(commandId);
    if (index > -1) {
      this.recentCommands.splice(index, 1);
    }
    this.recentCommands.unshift(commandId);
    if (this.recentCommands.length > this.maxRecentCommands) {
      this.recentCommands.pop();
    }
    this.saveRecentCommands();
  }

  getRecentCommands(): Command[] {
    return this.recentCommands
      .map(id => this.commands.get(id))
      .filter((cmd): cmd is Command => cmd !== undefined && cmd.enabled !== false);
  }

  // Listeners
  addListener(listener: (commands: Command[]) => void): void {
    this.listeners.add(listener);
  }

  removeListener(listener: (commands: Command[]) => void): void {
    this.listeners.delete(listener);
  }

  private notifyListeners(): void {
    const commands = this.getAllCommands();
    this.listeners.forEach(listener => listener(commands));
  }

  // Persistence
  private loadRecentCommands(): void {
    try {
      const stored = localStorage.getItem('commandPalette.recentCommands');
      if (stored) {
        this.recentCommands = JSON.parse(stored);
      }
    } catch (error) {
      console.error('Failed to load recent commands:', error);
    }
  }

  private saveRecentCommands(): void {
    try {
      localStorage.setItem('commandPalette.recentCommands', JSON.stringify(this.recentCommands));
    } catch (error) {
      console.error('Failed to save recent commands:', error);
    }
  }

  // Fuzzy matching algorithm
  private fuzzyMatch(pattern: string, str: string): number {
    let patternIdx = 0;
    let strIdx = 0;
    let score = 0;
    let consecutive = 0;

    while (patternIdx < pattern.length && strIdx < str.length) {
      if (pattern[patternIdx] === str[strIdx]) {
        score += 1 + consecutive * 0.1;
        consecutive++;
        patternIdx++;
      } else {
        consecutive = 0;
      }
      strIdx++;
    }

    if (patternIdx === pattern.length) {
      return score / pattern.length;
    }
    return 0;
  }
}

// Global instance
export const commandRegistry = new CommandRegistry();