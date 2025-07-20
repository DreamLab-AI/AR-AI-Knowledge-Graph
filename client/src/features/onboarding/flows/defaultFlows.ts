import { OnboardingFlow } from '../types';
import { commandRegistry } from '../../command-palette/CommandRegistry';

export const welcomeFlow: OnboardingFlow = {
  id: 'welcome',
  name: 'Welcome Tour',
  description: 'Get started with the application',
  steps: [
    {
      id: 'welcome',
      title: 'Welcome to LogSeq Spring Thing!',
      description: 'This interactive tour will help you get familiar with the main features of the application. You can skip this tour at any time or restart it later from the help menu.',
      position: 'center'
    },
    {
      id: 'graph-view',
      title: 'Graph Visualization',
      description: 'This is your main workspace where you can visualize and interact with your knowledge graph. Use your mouse to pan, zoom, and select nodes.',
      target: 'canvas',
      position: 'right'
    },
    {
      id: 'settings-panel',
      title: 'Settings Panel',
      description: 'Customize your visualization with various settings. You can adjust colors, node sizes, and many other visual properties.',
      target: '.setting-control',
      position: 'left'
    },
    {
      id: 'command-palette',
      title: 'Command Palette',
      description: 'Press Ctrl+K (or Cmd+K on Mac) to open the command palette. It provides quick access to all available commands and features.',
      position: 'center',
      action: () => {
        // Show command palette briefly
        window.dispatchEvent(new KeyboardEvent('keydown', { 
          key: 'k', 
          ctrlKey: true,
          bubbles: true 
        }));
        setTimeout(() => {
          window.dispatchEvent(new KeyboardEvent('keydown', { 
            key: 'Escape',
            bubbles: true 
          }));
        }, 2000);
      }
    },
    {
      id: 'help-system',
      title: 'Getting Help',
      description: 'Look for the info icons next to settings for detailed help. You can also press Shift+? to view all keyboard shortcuts.',
      position: 'center'
    },
    {
      id: 'complete',
      title: 'You\'re all set!',
      description: 'You\'ve completed the welcome tour. Feel free to explore the application and don\'t hesitate to check the help menu if you need assistance.',
      position: 'center',
      nextButtonText: 'Get Started'
    }
  ]
};

export const settingsFlow: OnboardingFlow = {
  id: 'settings-tour',
  name: 'Settings Tour',
  description: 'Learn about the settings system',
  steps: [
    {
      id: 'settings-intro',
      title: 'Settings Overview',
      description: 'The settings panel allows you to customize every aspect of your visualization. Let\'s explore the key features.',
      position: 'center'
    },
    {
      id: 'settings-search',
      title: 'Search Settings',
      description: 'Use the search bar to quickly find specific settings. Try typing "color" or "size" to filter settings.',
      target: 'input[placeholder*="Search settings"]',
      position: 'bottom'
    },
    {
      id: 'settings-tabs',
      title: 'Settings Categories',
      description: 'Settings are organized into categories. Click on different tabs to explore visualization, system, AI, and XR settings.',
      target: '[role="tablist"]',
      position: 'bottom'
    },
    {
      id: 'settings-undo',
      title: 'Undo/Redo Changes',
      description: 'Made a mistake? Use the undo and redo buttons to revert changes. You can also use Ctrl+Z and Ctrl+Shift+Z.',
      target: '.flex.items-center.gap-2',
      position: 'bottom'
    },
    {
      id: 'settings-help',
      title: 'Contextual Help',
      description: 'Hover over any setting label to see detailed help information. This helps you understand what each setting does.',
      target: '.setting-control',
      position: 'left'
    }
  ]
};

export const advancedFlow: OnboardingFlow = {
  id: 'advanced-features',
  name: 'Advanced Features',
  description: 'Discover power user features',
  steps: [
    {
      id: 'keyboard-shortcuts',
      title: 'Master Keyboard Shortcuts',
      description: 'Press Shift+? to see all available keyboard shortcuts. Power users can navigate the entire application without touching the mouse.',
      position: 'center',
      action: () => {
        window.dispatchEvent(new CustomEvent('show-keyboard-shortcuts'));
      }
    },
    {
      id: 'export-import',
      title: 'Export and Import Settings',
      description: 'You can export your settings to a file and import them later. This is useful for backing up your configuration or sharing it with others.',
      position: 'center'
    },
    {
      id: 'xr-mode',
      title: 'XR Mode',
      description: 'If you have compatible hardware, you can enable XR mode to view your graph in virtual or augmented reality.',
      position: 'center'
    }
  ]
};

// Register onboarding commands
export function registerOnboardingCommands() {
  commandRegistry.registerCommands([
    {
      id: 'onboarding.welcome',
      title: 'Start Welcome Tour',
      description: 'Begin the interactive welcome tour',
      category: 'help',
      keywords: ['tutorial', 'getting started', 'intro'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('start-onboarding', { 
          detail: { flowId: 'welcome' } 
        }));
      }
    },
    {
      id: 'onboarding.settings',
      title: 'Start Settings Tour',
      description: 'Learn about the settings system',
      category: 'help',
      keywords: ['tutorial', 'configuration', 'preferences'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('start-onboarding', { 
          detail: { flowId: 'settings-tour' } 
        }));
      }
    },
    {
      id: 'onboarding.advanced',
      title: 'Advanced Features Tour',
      description: 'Discover power user features',
      category: 'help',
      keywords: ['tutorial', 'power user', 'advanced'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('start-onboarding', { 
          detail: { flowId: 'advanced-features' } 
        }));
      }
    },
    {
      id: 'onboarding.reset',
      title: 'Reset All Tours',
      description: 'Mark all tours as not completed',
      category: 'help',
      keywords: ['restart', 'clear'],
      handler: () => {
        window.dispatchEvent(new CustomEvent('reset-onboarding'));
      }
    }
  ]);
}