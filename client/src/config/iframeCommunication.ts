/**
 * Configuration for secure iframe communication
 */

export const IFRAME_COMMUNICATION_CONFIG = {
  // Allowed origins for postMessage communication
  // In production, this should be restricted to specific domains
  allowedOrigins: [
    'https://narrativegoldmine.com',
    'https://www.narrativegoldmine.com',
    // Add other trusted origins here
  ],
  
  // Target origin for sending messages (use specific origin in production)
  // Currently using '*' for development, but should be updated
  targetOrigin: '*', // TODO: Change to 'https://narrativegoldmine.com' in production
  
  // Message action types
  messageActions: {
    NAVIGATE: 'navigate',
    UPDATE_NODE: 'updateNode',
    SYNC_STATE: 'syncState',
  } as const,
  
  // Validation settings
  validation: {
    requireValidUrl: true,
    requireKnownDomain: true,
    logMessages: true, // Set to false in production
  }
};

export type MessageAction = typeof IFRAME_COMMUNICATION_CONFIG.messageActions[keyof typeof IFRAME_COMMUNICATION_CONFIG.messageActions];