import { useState, useEffect, useCallback, useMemo } from 'react';
import { CommandPaletteState, Command } from '../types';
import { commandRegistry } from '../CommandRegistry';
import { useKeyboardShortcuts } from '../../../hooks/useKeyboardShortcuts';

export function useCommandPalette() {
  const [state, setState] = useState<CommandPaletteState>({
    isOpen: false,
    searchQuery: '',
    selectedIndex: 0,
    filteredCommands: [],
    recentCommands: []
  });

  // Update filtered commands when search query changes
  useEffect(() => {
    if (state.isOpen) {
      const filtered = state.searchQuery 
        ? commandRegistry.searchCommands(state.searchQuery)
        : commandRegistry.getRecentCommands().length > 0
          ? commandRegistry.getRecentCommands()
          : commandRegistry.getAllCommands().filter(cmd => cmd.enabled !== false);
      
      setState(prev => ({
        ...prev,
        filteredCommands: filtered,
        selectedIndex: 0
      }));
    }
  }, [state.searchQuery, state.isOpen]);

  // Load recent commands on mount
  useEffect(() => {
    setState(prev => ({
      ...prev,
      recentCommands: commandRegistry.getRecentCommands().map(cmd => cmd.id)
    }));
  }, []);

  // Open command palette
  const open = useCallback(() => {
    setState(prev => ({
      ...prev,
      isOpen: true,
      searchQuery: '',
      selectedIndex: 0
    }));
  }, []);

  // Close command palette
  const close = useCallback(() => {
    setState(prev => ({
      ...prev,
      isOpen: false,
      searchQuery: '',
      selectedIndex: 0
    }));
  }, []);

  // Toggle command palette
  const toggle = useCallback(() => {
    setState(prev => ({
      ...prev,
      isOpen: !prev.isOpen,
      searchQuery: '',
      selectedIndex: 0
    }));
  }, []);

  // Update search query
  const setSearchQuery = useCallback((query: string) => {
    setState(prev => ({
      ...prev,
      searchQuery: query,
      selectedIndex: 0
    }));
  }, []);

  // Navigate through commands
  const navigateUp = useCallback(() => {
    setState(prev => ({
      ...prev,
      selectedIndex: Math.max(0, prev.selectedIndex - 1)
    }));
  }, []);

  const navigateDown = useCallback(() => {
    setState(prev => ({
      ...prev,
      selectedIndex: Math.min(prev.filteredCommands.length - 1, prev.selectedIndex + 1)
    }));
  }, []);

  // Execute selected command
  const executeCommand = useCallback(async (command: Command) => {
    try {
      commandRegistry.addRecentCommand(command.id);
      close();
      await command.handler();
    } catch (error) {
      console.error('Failed to execute command:', error);
    }
  }, [close]);

  const executeSelectedCommand = useCallback(async () => {
    const selectedCommand = state.filteredCommands[state.selectedIndex];
    if (selectedCommand) {
      await executeCommand(selectedCommand);
    }
  }, [state.filteredCommands, state.selectedIndex, executeCommand]);

  // Register keyboard shortcut for opening command palette
  useKeyboardShortcuts({
    'command-palette-toggle': {
      key: 'k',
      ctrl: true,
      description: 'Toggle command palette',
      handler: toggle,
      category: 'General'
    }
  });

  return {
    ...state,
    open,
    close,
    toggle,
    setSearchQuery,
    navigateUp,
    navigateDown,
    executeCommand,
    executeSelectedCommand
  };
}