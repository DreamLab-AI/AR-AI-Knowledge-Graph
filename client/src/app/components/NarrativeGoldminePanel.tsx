import React, { CSSProperties, useEffect, useRef } from 'react';
import { IFRAME_COMMUNICATION_CONFIG } from '../../config/iframeCommunication';
import { isAllowedOrigin, isNavigationMessage, NavigationMessage } from '../../utils/iframeCommunication';

const NarrativeGoldminePanel: React.FC = () => {
  const iframeRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      // Security: Validate origin using utility function
      if (!isAllowedOrigin(event.origin) && IFRAME_COMMUNICATION_CONFIG.validation.requireKnownDomain) {
        console.warn('Message from unauthorized origin blocked:', event.origin);
        return;
      }
      
      if (IFRAME_COMMUNICATION_CONFIG.validation.logMessages) {
        console.log('Received message from origin:', event.origin);
      }

      // Type guard for navigation messages using utility function
      if (isNavigationMessage(event.data)) {
        const message = event.data as NavigationMessage;

        // Security: Validate URL format and domain
        try {
          const url = new URL(message.url);
          
          // Check if it's a narrativegoldmine.com URL
          if (!url.hostname.includes('narrativegoldmine.com')) {
            console.warn('Navigation blocked: URL not from narrativegoldmine.com', url.hostname);
            return;
          }

          // If validation passes, navigate the iframe
          if (iframeRef.current) {
            iframeRef.current.src = message.url;
            
            if (IFRAME_COMMUNICATION_CONFIG.validation.logMessages) {
              console.log('Navigated iframe to:', message.url, {
                nodeId: message.nodeId,
                nodeLabel: message.nodeLabel,
                timestamp: message.timestamp
              });
            }
          }
        } catch (error) {
          console.error('Invalid URL in navigation message:', message.url, error);
        }
      }
    };

    // Add event listener
    window.addEventListener('message', handleMessage);

    // Cleanup on unmount
    return () => {
      window.removeEventListener('message', handleMessage);
    };
  }, []);

  const panelStyle: CSSProperties = {
    width: '100%',
    height: '100%',
    overflow: 'hidden', // Iframe will handle its own scrolling
    backgroundColor: '#000', // Optional: background while iframe loads
  };

  const iframeStyle: CSSProperties = {
    width: '100%',
    height: '100%',
    border: 'none', // Remove default iframe border
  };

  return (
    <div style={panelStyle}>
      <iframe
        ref={iframeRef}
        id="narrative-goldmine-iframe" // Added ID for backward compatibility
        src="https://narrativegoldmine.com//#/graph"
        style={iframeStyle}
        title="Narrative Goldmine"
        sandbox="allow-scripts allow-same-origin allow-popups allow-forms" // Standard sandbox attributes
        loading="lazy"
        referrerPolicy="no-referrer"
      ></iframe>
    </div>
  );
};

export default NarrativeGoldminePanel;