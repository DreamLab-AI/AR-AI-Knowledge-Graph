import { useState, useCallback, useRef, useEffect } from 'react';
import { useSettingsStore } from '../../../store/settingsStore';
import { SettingsState } from '../types/settingsTypes';

interface HistoryEntry {
  settings: SettingsState;
  timestamp: number;
  description?: string;
}

interface SettingsHistoryState {
  history: HistoryEntry[];
  currentIndex: number;
  canUndo: boolean;
  canRedo: boolean;
  maxHistorySize: number;
}

const MAX_HISTORY_SIZE = 50;

export function useSettingsHistory() {
  const [historyState, setHistoryState] = useState<SettingsHistoryState>({
    history: [],
    currentIndex: -1,
    canUndo: false,
    canRedo: false,
    maxHistorySize: MAX_HISTORY_SIZE
  });

  const isUndoingRef = useRef(false);
  const lastSettingsRef = useRef<string>('');

  // Subscribe to settings changes
  useEffect(() => {
    const unsubscribe = useSettingsStore.subscribe((state) => {
      // Skip if we're in the middle of an undo/redo operation
      if (isUndoingRef.current) return;

      const currentSettings = JSON.stringify(state.settings);
      
      // Skip if settings haven't actually changed
      if (currentSettings === lastSettingsRef.current) return;
      
      lastSettingsRef.current = currentSettings;

      // Add new entry to history
      setHistoryState(prev => {
        const newEntry: HistoryEntry = {
          settings: JSON.parse(currentSettings),
          timestamp: Date.now()
        };

        // If we're not at the end of history, remove future entries
        const newHistory = prev.currentIndex < prev.history.length - 1
          ? [...prev.history.slice(0, prev.currentIndex + 1), newEntry]
          : [...prev.history, newEntry];

        // Limit history size
        if (newHistory.length > prev.maxHistorySize) {
          newHistory.shift();
        }

        const newIndex = newHistory.length - 1;

        return {
          ...prev,
          history: newHistory,
          currentIndex: newIndex,
          canUndo: newIndex > 0,
          canRedo: false
        };
      });
    });

    return unsubscribe;
  }, []);

  // Undo operation
  const undo = useCallback(async () => {
    if (!historyState.canUndo) return;

    isUndoingRef.current = true;
    
    try {
      const targetIndex = historyState.currentIndex - 1;
      const targetEntry = historyState.history[targetIndex];
      
      if (targetEntry) {
        await useSettingsStore.getState().updateSettings(targetEntry.settings);
        
        setHistoryState(prev => ({
          ...prev,
          currentIndex: targetIndex,
          canUndo: targetIndex > 0,
          canRedo: true
        }));
      }
    } finally {
      // Reset flag after a brief delay to ensure the update has propagated
      setTimeout(() => {
        isUndoingRef.current = false;
      }, 100);
    }
  }, [historyState.canUndo, historyState.currentIndex, historyState.history]);

  // Redo operation
  const redo = useCallback(async () => {
    if (!historyState.canRedo) return;

    isUndoingRef.current = true;
    
    try {
      const targetIndex = historyState.currentIndex + 1;
      const targetEntry = historyState.history[targetIndex];
      
      if (targetEntry) {
        await useSettingsStore.getState().updateSettings(targetEntry.settings);
        
        setHistoryState(prev => ({
          ...prev,
          currentIndex: targetIndex,
          canUndo: true,
          canRedo: targetIndex < prev.history.length - 1
        }));
      }
    } finally {
      // Reset flag after a brief delay
      setTimeout(() => {
        isUndoingRef.current = false;
      }, 100);
    }
  }, [historyState.canRedo, historyState.currentIndex, historyState.history]);

  // Clear history
  const clearHistory = useCallback(() => {
    setHistoryState({
      history: [],
      currentIndex: -1,
      canUndo: false,
      canRedo: false,
      maxHistorySize: MAX_HISTORY_SIZE
    });
    lastSettingsRef.current = '';
  }, []);

  // Get history info
  const getHistoryInfo = useCallback(() => {
    return {
      totalEntries: historyState.history.length,
      currentPosition: historyState.currentIndex + 1,
      oldestEntry: historyState.history[0]?.timestamp,
      newestEntry: historyState.history[historyState.history.length - 1]?.timestamp
    };
  }, [historyState]);

  return {
    undo,
    redo,
    canUndo: historyState.canUndo,
    canRedo: historyState.canRedo,
    clearHistory,
    getHistoryInfo,
    historyLength: historyState.history.length,
    currentIndex: historyState.currentIndex
  };
}