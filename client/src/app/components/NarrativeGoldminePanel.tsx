import React, { CSSProperties } from 'react';

const NarrativeGoldminePanel: React.FC = () => {
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
        id="narrative-goldmine-iframe" // Added ID
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