import { HelpContent } from './types';
import { helpRegistry } from './HelpRegistry';

export const settingsHelpContent: Record<string, HelpContent> = {
  // System Settings
  'settings.system.debug': {
    id: 'settings.system.debug',
    title: 'Debug Mode',
    description: 'Enable debug mode to see detailed logging and performance metrics',
    detailedHelp: 'Debug mode provides additional information for troubleshooting issues. This includes console logs, performance metrics, and network request details.',
    examples: [
      'View console logs in browser DevTools',
      'Monitor WebSocket connection status',
      'Track component render performance'
    ]
  },
  'settings.system.performance': {
    id: 'settings.system.performance',
    title: 'Performance Mode',
    description: 'Optimize rendering performance for smoother visualization',
    detailedHelp: 'Performance mode adjusts various rendering settings to improve frame rates. This may reduce visual quality slightly but provides smoother interaction.',
    examples: [
      'Reduces particle effects',
      'Simplifies lighting calculations',
      'Lowers texture resolution'
    ]
  },
  'settings.system.autoSave': {
    id: 'settings.system.autoSave',
    title: 'Auto-save Settings',
    description: 'Automatically save your settings changes',
    detailedHelp: 'When enabled, settings are saved automatically after each change. Disable this to manually control when settings are persisted.',
  },

  // Visualization Settings
  'settings.visualization.theme': {
    id: 'settings.visualization.theme',
    title: 'Visualization Theme',
    description: 'Choose the color theme for the graph visualization',
    detailedHelp: 'Different themes optimize visibility for various lighting conditions and personal preferences.',
    examples: [
      'Dark theme for reduced eye strain',
      'Light theme for bright environments',
      'High contrast for better visibility'
    ]
  },
  'settings.visualization.nodeSize': {
    id: 'settings.visualization.nodeSize',
    title: 'Node Size',
    description: 'Adjust the size of nodes in the graph',
    detailedHelp: 'Larger nodes are easier to click but may overlap in dense graphs. Smaller nodes allow viewing more connections at once.',
  },
  'settings.visualization.linkOpacity': {
    id: 'settings.visualization.linkOpacity',
    title: 'Link Opacity',
    description: 'Control the transparency of connections between nodes',
    detailedHelp: 'Lower opacity helps see through dense connection networks, while higher opacity makes individual connections clearer.',
  },

  // AI Settings
  'settings.ai.provider': {
    id: 'settings.ai.provider',
    title: 'AI Provider',
    description: 'Select which AI service to use for analysis',
    detailedHelp: 'Different providers offer various capabilities and pricing models. Some may require API keys.',
    relatedTopics: ['API Keys', 'AI Models']
  },
  'settings.ai.temperature': {
    id: 'settings.ai.temperature',
    title: 'AI Temperature',
    description: 'Control the creativity vs consistency of AI responses',
    detailedHelp: 'Lower values (0.1-0.3) produce more focused, deterministic responses. Higher values (0.7-1.0) increase creativity and variation.',
    examples: [
      '0.1 - Very consistent, factual responses',
      '0.5 - Balanced creativity and consistency',
      '0.9 - Highly creative, varied responses'
    ]
  },

  // XR Settings
  'settings.xr.enabled': {
    id: 'settings.xr.enabled',
    title: 'Enable XR',
    description: 'Enable extended reality (VR/AR) features',
    detailedHelp: 'XR features require compatible hardware and browser support. Enable this to access immersive visualization modes.',
    relatedTopics: ['WebXR', 'VR Headsets']
  },
  'settings.xr.handTracking': {
    id: 'settings.xr.handTracking',
    title: 'Hand Tracking',
    description: 'Use hand tracking instead of controllers in XR',
    detailedHelp: 'Hand tracking provides controller-free interaction but may be less precise. Requires supported XR hardware.',
  }
};

// Register all settings help
export function registerSettingsHelp() {
  Object.values(settingsHelpContent).forEach(help => {
    helpRegistry.registerHelp(help);
  });

  // Register categories
  helpRegistry.registerCategory({
    id: 'settings',
    name: 'Settings',
    description: 'Help for application settings and configuration',
    items: Object.values(settingsHelpContent)
  });
}