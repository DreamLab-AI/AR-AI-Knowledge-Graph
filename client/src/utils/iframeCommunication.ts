import { IFRAME_COMMUNICATION_CONFIG, MessageAction } from '../config/iframeCommunication';

/**
 * Base interface for all iframe messages
 */
export interface IframeMessage {
  action: MessageAction;
  timestamp: number;
  [key: string]: any;
}

/**
 * Navigation message interface
 */
export interface NavigationMessage extends IframeMessage {
  action: typeof IFRAME_COMMUNICATION_CONFIG.messageActions.NAVIGATE;
  url: string;
  nodeId?: string;
  nodeLabel?: string;
}

/**
 * Send a message to an iframe
 */
export function sendMessageToIframe(
  iframe: HTMLIFrameElement,
  message: IframeMessage,
  targetOrigin?: string
): boolean {
  if (!iframe || !iframe.contentWindow) {
    console.error('Invalid iframe or iframe not loaded');
    return false;
  }

  try {
    iframe.contentWindow.postMessage(
      message,
      targetOrigin || IFRAME_COMMUNICATION_CONFIG.targetOrigin
    );
    
    if (IFRAME_COMMUNICATION_CONFIG.validation.logMessages) {
      console.log('Sent message to iframe:', message);
    }
    
    return true;
  } catch (error) {
    console.error('Failed to send message to iframe:', error);
    return false;
  }
}

/**
 * Send a navigation message to the Narrative Goldmine iframe
 */
export function navigateNarrativeGoldmine(
  nodeId: string,
  nodeLabel: string,
  slug: string
): boolean {
  const iframe = document.getElementById('narrative-goldmine-iframe') as HTMLIFrameElement | null;
  
  if (!iframe) {
    console.warn('Narrative Goldmine iframe not found');
    return false;
  }

  const url = `https://narrativegoldmine.com//#/page/${slug}`;
  const message: NavigationMessage = {
    action: IFRAME_COMMUNICATION_CONFIG.messageActions.NAVIGATE,
    url,
    nodeId,
    nodeLabel,
    timestamp: Date.now()
  };

  return sendMessageToIframe(iframe, message);
}

/**
 * Validate if a message is from an allowed origin
 */
export function isAllowedOrigin(origin: string): boolean {
  return IFRAME_COMMUNICATION_CONFIG.allowedOrigins.some(allowedOrigin =>
    origin === allowedOrigin || origin.startsWith(allowedOrigin)
  );
}

/**
 * Type guard for navigation messages
 */
export function isNavigationMessage(message: any): message is NavigationMessage {
  return (
    message &&
    typeof message === 'object' &&
    message.action === IFRAME_COMMUNICATION_CONFIG.messageActions.NAVIGATE &&
    typeof message.url === 'string'
  );
}