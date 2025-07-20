import React from 'react';
import { useSettingsStore } from '../../../store/settingsStore';
import { createLogger } from '../../../utils/logger';

const logger = createLogger('VisualEnhancementToggle');

export const VisualEnhancementToggle: React.FC = () => {
  const settings = useSettingsStore(state => state.settings);
  const updateSettings = useSettingsStore(state => state.updateSettings);
  
  const isEnhanced = settings?.visualisation?.nodes?.enableHologram || 
                     settings?.visualisation?.edges?.enableFlowEffect;
  
  const handleToggle = () => {
    logger.info('Toggling visual enhancements', { currentState: isEnhanced });
    
    updateSettings((draft) => {
      // Toggle hologram effect
      if (draft.visualisation?.nodes) {
        draft.visualisation.nodes.enableHologram = !isEnhanced;
      }
      
      // Toggle flow effect
      if (draft.visualisation?.edges) {
        draft.visualisation.edges.enableFlowEffect = !isEnhanced;
        draft.visualisation.edges.useGradient = !isEnhanced;
      }
      
      // Toggle bloom
      if (draft.visualisation?.bloom) {
        draft.visualisation.bloom.enabled = !isEnhanced;
      }
      
      // Toggle animations
      if (draft.visualisation?.animation) {
        draft.visualisation.animation.pulseEnabled = !isEnhanced;
      }
    });
  };
  
  return (
    <div style={{
      position: 'fixed',
      top: '20px',
      right: '20px',
      zIndex: 1000,
      background: 'rgba(0, 0, 0, 0.8)',
      border: '1px solid #00ffff',
      borderRadius: '8px',
      padding: '10px 20px',
      color: '#ffffff',
      fontFamily: 'monospace',
      fontSize: '14px',
      cursor: 'pointer',
      userSelect: 'none',
      boxShadow: isEnhanced ? '0 0 20px #00ffff' : 'none',
      transition: 'all 0.3s ease'
    }}
    onClick={handleToggle}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
        <span style={{ 
          display: 'inline-block',
          width: '12px',
          height: '12px',
          borderRadius: '50%',
          background: isEnhanced ? '#00ff00' : '#ff0000',
          boxShadow: isEnhanced ? '0 0 10px #00ff00' : 'none'
        }} />
        <span>Visual Effects: {isEnhanced ? 'ENHANCED' : 'STANDARD'}</span>
      </div>
      <div style={{ 
        fontSize: '12px', 
        opacity: 0.7,
        marginTop: '5px'
      }}>
        Click to toggle hologram & flow effects
      </div>
    </div>
  );
};