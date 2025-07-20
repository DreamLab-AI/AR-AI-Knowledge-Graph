import { Command } from './types';
import { commandRegistry } from './CommandRegistry';
import { 
  Settings, 
  HelpCircle, 
  RefreshCw, 
  Download, 
  Upload,
  Palette,
  Maximize2,
  Eye,
  EyeOff,
  RotateCcw,
  Save,
  Moon,
  Sun,
  Terminal,
  Search,
  Undo2,
  Redo2
} from 'lucide-react';
import { useSettingsStore } from '../../store/settingsStore';
import { helpRegistry } from '../help/HelpRegistry';

// Register default categories
export function registerDefaultCategories() {
  commandRegistry.registerCategory({
    id: 'navigation',
    name: 'Navigation',
    priority: 1
  });

  commandRegistry.registerCategory({
    id: 'settings',
    name: 'Settings',
    icon: Settings,
    priority: 2
  });

  commandRegistry.registerCategory({
    id: 'view',
    name: 'View',
    icon: Eye,
    priority: 3
  });

  commandRegistry.registerCategory({
    id: 'help',
    name: 'Help',
    icon: HelpCircle,
    priority: 4
  });

  commandRegistry.registerCategory({
    id: 'system',
    name: 'System',
    icon: Terminal,
    priority: 5
  });
}

// Register default commands
export function registerDefaultCommands() {
  const commands: Command[] = [
    // Navigation commands
    {
      id: 'nav.settings',
      title: 'Open Settings',
      description: 'Open the settings panel',
      category: 'navigation',
      icon: Settings,
      keywords: ['preferences', 'config', 'configuration'],
      handler: () => {
        // Emit event to open settings
        window.dispatchEvent(new CustomEvent('open-settings'));
      }
    },
    {
      id: 'nav.help',
      title: 'Show Help',
      description: 'Display help and documentation',
      category: 'navigation',
      icon: HelpCircle,
      keywords: ['docs', 'documentation', 'guide'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('show-help'));
      }
    },
    
    // Help commands
    {
      id: 'help.search',
      title: 'Search Help Topics',
      description: 'Search through all available help content',
      category: 'help',
      icon: Search,
      keywords: ['find', 'lookup', 'support'],
      handler: () => {
        // Open help search dialog
        window.dispatchEvent(new CustomEvent('search-help'));
      }
    },
    {
      id: 'help.keyboard',
      title: 'Show Keyboard Shortcuts',
      description: 'Display all available keyboard shortcuts',
      category: 'help',
      icon: Terminal,
      shortcut: { key: '?', shift: true },
      keywords: ['hotkeys', 'bindings', 'keys'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('show-keyboard-shortcuts'));
      }
    },
    {
      id: 'help.tour',
      title: 'Start Tutorial Tour',
      description: 'Begin an interactive tour of the application',
      category: 'help',
      icon: HelpCircle,
      keywords: ['guide', 'walkthrough', 'onboarding'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('start-tour'));
      }
    },

    // Settings commands
    {
      id: 'settings.undo',
      title: 'Undo Settings Change',
      description: 'Undo the last settings change',
      category: 'settings',
      icon: Undo2,
      shortcut: { key: 'z', ctrl: true },
      keywords: ['revert', 'back'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('settings-undo'));
      }
    },
    {
      id: 'settings.redo',
      title: 'Redo Settings Change',
      description: 'Redo the last undone settings change',
      category: 'settings',
      icon: Redo2,
      shortcut: { key: 'z', ctrl: true, shift: true },
      keywords: ['forward', 'restore'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('settings-redo'));
      }
    },
    {
      id: 'settings.reset',
      title: 'Reset All Settings',
      description: 'Reset all settings to default values',
      category: 'settings',
      icon: RotateCcw,
      keywords: ['default', 'restore'],
      handler: async () => {
        const confirmed = window.confirm('Are you sure you want to reset all settings to default?');
        if (confirmed) {
          await useSettingsStore.getState().resetSettings();
          window.location.reload();
        }
      }
    },
    {
      id: 'settings.export',
      title: 'Export Settings',
      description: 'Export current settings to a file',
      category: 'settings',
      icon: Download,
      keywords: ['save', 'backup'],
      handler: async () => {
        const settings = useSettingsStore.getState().settings;
        const blob = new Blob([JSON.stringify(settings, null, 2)], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `settings-${new Date().toISOString().split('T')[0]}.json`;
        a.click();
        URL.revokeObjectURL(url);
      }
    },
    {
      id: 'settings.import',
      title: 'Import Settings',
      description: 'Import settings from a file',
      category: 'settings',
      icon: Upload,
      keywords: ['load', 'restore'],
      handler: async () => {
        const input = document.createElement('input');
        input.type = 'file';
        input.accept = '.json';
        input.onchange = async (e) => {
          const file = (e.target as HTMLInputElement).files?.[0];
          if (file) {
            const text = await file.text();
            try {
              const settings = JSON.parse(text);
              await useSettingsStore.getState().updateSettings(settings);
              window.location.reload();
            } catch (error) {
              alert('Failed to import settings. Please check the file format.');
            }
          }
        };
        input.click();
      }
    },

    // View commands
    {
      id: 'view.fullscreen',
      title: 'Toggle Fullscreen',
      description: 'Enter or exit fullscreen mode',
      category: 'view',
      icon: Maximize2,
      shortcut: { key: 'f', ctrl: true, shift: true },
      keywords: ['maximize', 'full'],
      handler: () => {
        if (!document.fullscreenElement) {
          document.documentElement.requestFullscreen();
        } else {
          document.exitFullscreen();
        }
      }
    },
    {
      id: 'view.theme.toggle',
      title: 'Toggle Theme',
      description: 'Switch between light and dark theme',
      category: 'view',
      icon: Palette,
      keywords: ['dark', 'light', 'mode'],
      handler: () => {
        const currentTheme = document.documentElement.getAttribute('data-theme');
        const newTheme = currentTheme === 'dark' ? 'light' : 'dark';
        document.documentElement.setAttribute('data-theme', newTheme);
        localStorage.setItem('theme', newTheme);
      }
    },
    {
      id: 'view.refresh',
      title: 'Refresh View',
      description: 'Refresh the current view',
      category: 'view',
      icon: RefreshCw,
      shortcut: { key: 'r', ctrl: true },
      keywords: ['reload'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('refresh-view'));
      }
    },

    // System commands
    {
      id: 'system.reload',
      title: 'Reload Application',
      description: 'Reload the entire application',
      category: 'system',
      icon: RefreshCw,
      shortcut: { key: 'r', ctrl: true, shift: true },
      keywords: ['restart'],
      handler: () => {
        window.location.reload();
      }
    },
    {
      id: 'system.save',
      title: 'Save All Changes',
      description: 'Save all pending changes',
      category: 'system',
      icon: Save,
      shortcut: { key: 's', ctrl: true },
      handler: async () => {
        await useSettingsStore.getState().saveSettings();
        window.dispatchEvent(new CustomEvent('save-all'));
      }
    }
  ];

  commandRegistry.registerCommands(commands);
}

// Initialize default commands and categories
export function initializeCommandPalette() {
  registerDefaultCategories();
  registerDefaultCommands();
}