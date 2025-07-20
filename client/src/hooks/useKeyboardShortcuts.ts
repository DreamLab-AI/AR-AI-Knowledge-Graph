import { useEffect, useRef, useCallback } from 'react';

export interface KeyboardShortcut {
  key: string;
  ctrl?: boolean;
  alt?: boolean;
  shift?: boolean;
  meta?: boolean;
  description: string;
  handler: () => void;
  enabled?: boolean;
  category?: string;
}

export interface KeyboardShortcutOptions {
  preventDefault?: boolean;
  stopPropagation?: boolean;
  allowInInput?: boolean;
}

class KeyboardShortcutRegistry {
  private shortcuts: Map<string, KeyboardShortcut> = new Map();
  private listeners: Set<() => void> = new Set();

  register(id: string, shortcut: KeyboardShortcut) {
    this.shortcuts.set(id, { ...shortcut, enabled: shortcut.enabled !== false });
    this.notifyListeners();
  }

  unregister(id: string) {
    this.shortcuts.delete(id);
    this.notifyListeners();
  }

  getShortcuts(): Map<string, KeyboardShortcut> {
    return new Map(this.shortcuts);
  }

  getShortcutsByCategory(): Map<string, KeyboardShortcut[]> {
    const byCategory = new Map<string, KeyboardShortcut[]>();
    
    this.shortcuts.forEach((shortcut, id) => {
      const category = shortcut.category || 'General';
      if (!byCategory.has(category)) {
        byCategory.set(category, []);
      }
      byCategory.get(category)!.push({ ...shortcut, key: id });
    });
    
    return byCategory;
  }

  enable(id: string) {
    const shortcut = this.shortcuts.get(id);
    if (shortcut) {
      shortcut.enabled = true;
      this.notifyListeners();
    }
  }

  disable(id: string) {
    const shortcut = this.shortcuts.get(id);
    if (shortcut) {
      shortcut.enabled = false;
      this.notifyListeners();
    }
  }

  addListener(listener: () => void) {
    this.listeners.add(listener);
  }

  removeListener(listener: () => void) {
    this.listeners.delete(listener);
  }

  private notifyListeners() {
    this.listeners.forEach(listener => listener());
  }

  getShortcutString(shortcut: KeyboardShortcut): string {
    const parts: string[] = [];
    if (shortcut.ctrl) parts.push('Ctrl');
    if (shortcut.alt) parts.push('Alt');
    if (shortcut.shift) parts.push('Shift');
    if (shortcut.meta) parts.push('⌘');
    parts.push(shortcut.key.toUpperCase());
    return parts.join('+');
  }
}

// Global registry instance
export const keyboardShortcutRegistry = new KeyboardShortcutRegistry();

// Hook for using keyboard shortcuts
export function useKeyboardShortcuts(
  shortcuts: Record<string, Omit<KeyboardShortcut, 'handler'> & { handler: () => void }>,
  options: KeyboardShortcutOptions = {}
) {
  const { preventDefault = true, stopPropagation = true, allowInInput = false } = options;
  const registeredIds = useRef<Set<string>>(new Set());

  useEffect(() => {
    // Register all shortcuts
    Object.entries(shortcuts).forEach(([id, shortcut]) => {
      keyboardShortcutRegistry.register(id, shortcut);
      registeredIds.current.add(id);
    });

    // Cleanup function
    return () => {
      registeredIds.current.forEach(id => {
        keyboardShortcutRegistry.unregister(id);
      });
      registeredIds.current.clear();
    };
  }, [shortcuts]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Skip if in input element and not allowed
      if (!allowInInput) {
        const target = event.target as HTMLElement;
        if (
          target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.isContentEditable
        ) {
          return;
        }
      }

      // Check each registered shortcut
      keyboardShortcutRegistry.getShortcuts().forEach((shortcut, id) => {
        if (!shortcut.enabled) return;

        const matchesKey = event.key.toLowerCase() === shortcut.key.toLowerCase();
        const matchesCtrl = shortcut.ctrl ? (event.ctrlKey || event.metaKey) : !event.ctrlKey;
        const matchesAlt = shortcut.alt ? event.altKey : !event.altKey;
        const matchesShift = shortcut.shift ? event.shiftKey : !event.shiftKey;
        const matchesMeta = shortcut.meta ? event.metaKey : !event.metaKey;

        if (matchesKey && matchesCtrl && matchesAlt && matchesShift && matchesMeta) {
          if (preventDefault) event.preventDefault();
          if (stopPropagation) event.stopPropagation();
          shortcut.handler();
        }
      });
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [allowInInput, preventDefault, stopPropagation]);
}

// Hook for displaying keyboard shortcuts
export function useKeyboardShortcutsList() {
  const [shortcuts, setShortcuts] = useState<Map<string, KeyboardShortcut[]>>(new Map());

  useEffect(() => {
    const updateShortcuts = () => {
      setShortcuts(keyboardShortcutRegistry.getShortcutsByCategory());
    };

    updateShortcuts();
    keyboardShortcutRegistry.addListener(updateShortcuts);

    return () => {
      keyboardShortcutRegistry.removeListener(updateShortcuts);
    };
  }, []);

  return shortcuts;
}

// Utility function to format shortcut for display
export function formatShortcut(shortcut: KeyboardShortcut): string {
  return keyboardShortcutRegistry.getShortcutString(shortcut);
}

import { useState } from 'react';