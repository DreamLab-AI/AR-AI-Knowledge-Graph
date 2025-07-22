import React, { useState, useEffect, useRef } from 'react';
import { SpaceDriver } from '../../../services/SpaceDriverService';
import { useSettingsStore } from '../../../store/settingsStore';

interface IntegratedControlPanelProps {
  showStats: boolean;
  enableBloom: boolean;
  onOrbitControlsToggle?: (enabled: boolean) => void;
  swarmData?: {
    nodeCount: number;
    edgeCount: number;
    tokenCount: number;
    mcpConnected: boolean;
    dataSource: string;
  };
}

// Map SpacePilot buttons to menu sections
const BUTTON_MENU_MAP = {
  '1': { id: 'appearance', label: 'Appearance' },
  '2': { id: 'physics', label: 'Physics' },
  '3': { id: 'visual', label: 'Visual FX' },
  '4': { id: 'auth', label: 'Auth' },
  '5': { id: 'data', label: 'Data' },
  '6': { id: 'info', label: 'Info' }
};

// Navigation button mappings
const NAV_BUTTONS = {
  '7': 'up',     // Up button
  'A': 'down',   // Down button (hex A = 10)
  '9': 'right',  // Right button
  '8': 'left',   // Left button
  'F': 'commit'  // F button to commit (hex F = 15)
};

export const IntegratedControlPanelEnhanced: React.FC<IntegratedControlPanelProps> = ({ 
  showStats, 
  enableBloom,
  onOrbitControlsToggle,
  swarmData
}) => {
  const [isExpanded, setIsExpanded] = useState(true);
  const [spacePilotConnected, setSpacePilotConnected] = useState(false);
  const [spacePilotButtons, setSpacePilotButtons] = useState<string[]>([]);
  const [webHidAvailable, setWebHidAvailable] = useState(false);
  const [spacePilotRawInput, setSpacePilotRawInput] = useState({
    translation: { x: 0, y: 0, z: 0 },
    rotation: { rx: 0, ry: 0, rz: 0 }
  });
  
  // Menu state
  const [activeSection, setActiveSection] = useState<string | null>(null);
  const [selectedFieldIndex, setSelectedFieldIndex] = useState(0);
  const [selectedValue, setSelectedValue] = useState<number>(0);
  
  // Nostr auth state
  const [nostrConnected, setNostrConnected] = useState(false);
  const [nostrPublicKey, setNostrPublicKey] = useState<string>('');
  
  // Settings store access
  const settings = useSettingsStore(state => state.settings);
  const updateSettings = useSettingsStore(state => state.updateSettings);

  // Check WebHID availability
  useEffect(() => {
    setWebHidAvailable('hid' in navigator);
  }, []);

  // Set up SpacePilot event listeners
  useEffect(() => {
    const handleConnect = () => {
      setSpacePilotConnected(true);
      if (onOrbitControlsToggle) {
        onOrbitControlsToggle(false);
      }
    };
    
    const handleDisconnect = () => {
      setSpacePilotConnected(false);
      setSpacePilotButtons([]);
      if (onOrbitControlsToggle) {
        onOrbitControlsToggle(true);
      }
    };
    
    const handleButtons = (event: any) => {
      const buttons = event.detail.buttons || [];
      setSpacePilotButtons(buttons);
      
      // Handle menu section buttons (1-6)
      buttons.forEach((btn: string) => {
        const btnNum = btn.replace('[', '').replace(']', '');
        if (BUTTON_MENU_MAP[btnNum]) {
          setActiveSection(BUTTON_MENU_MAP[btnNum].id);
        }
        
        // Handle navigation buttons
        if (NAV_BUTTONS[btnNum]) {
          handleNavigation(NAV_BUTTONS[btnNum]);
        }
      });
    };
    
    const handleTranslate = (event: any) => {
      setSpacePilotRawInput(prev => ({
        ...prev,
        translation: {
          x: event.detail.x || 0,
          y: event.detail.y || 0,
          z: event.detail.z || 0
        }
      }));
    };
    
    const handleRotate = (event: any) => {
      setSpacePilotRawInput(prev => ({
        ...prev,
        rotation: {
          rx: event.detail.rx || 0,
          ry: event.detail.ry || 0,
          rz: event.detail.rz || 0
        }
      }));
    };
    
    SpaceDriver.addEventListener('connect', handleConnect);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);
    SpaceDriver.addEventListener('buttons', handleButtons);
    SpaceDriver.addEventListener('translate', handleTranslate);
    SpaceDriver.addEventListener('rotate', handleRotate);
    
    return () => {
      SpaceDriver.removeEventListener('connect', handleConnect);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
      SpaceDriver.removeEventListener('buttons', handleButtons);
      SpaceDriver.removeEventListener('translate', handleTranslate);
      SpaceDriver.removeEventListener('rotate', handleRotate);
    };
  }, [onOrbitControlsToggle, activeSection, selectedFieldIndex]);

  const handleConnectClick = async () => {
    try {
      await SpaceDriver.scan();
    } catch (error) {
      console.error('Failed to connect to SpacePilot:', error);
    }
  };

  const handleNavigation = (action: string) => {
    if (!activeSection) return;
    
    const sectionSettings = getSectionSettings();
    if (!sectionSettings) return;
    
    const fieldCount = sectionSettings.fields.length;
    
    switch (action) {
      case 'up':
        setSelectedFieldIndex(prev => Math.max(0, prev - 1));
        break;
      case 'down':
        setSelectedFieldIndex(prev => Math.min(fieldCount - 1, prev + 1));
        break;
      case 'left':
      case 'right':
        handleValueAdjustment(action);
        break;
      case 'commit':
        handleCommitSettings();
        break;
    }
  };
  
  const handleValueAdjustment = (direction: string) => {
    const sectionSettings = getSectionSettings();
    if (!sectionSettings) return;
    
    const field = sectionSettings.fields[selectedFieldIndex];
    if (!field) return;
    
    const currentValue = getValueFromPath(field.path);
    
    switch (field.type) {
      case 'slider':
        const step = (field.max - field.min) / 20; // 5% steps
        const delta = direction === 'right' ? step : -step;
        const newValue = Math.max(field.min, Math.min(field.max, currentValue + delta));
        updateSettingByPath(field.path, newValue);
        break;
      case 'toggle':
        updateSettingByPath(field.path, !currentValue);
        break;
      case 'color':
        // For color, cycle through preset colors
        const colors = ['#FF0000', '#00FF00', '#0000FF', '#FFFF00', '#FF00FF', '#00FFFF', '#FFFFFF'];
        const currentIndex = colors.indexOf(currentValue) || 0;
        const nextIndex = direction === 'right' 
          ? (currentIndex + 1) % colors.length 
          : (currentIndex - 1 + colors.length) % colors.length;
        updateSettingByPath(field.path, colors[nextIndex]);
        break;
    }
  };
  
  const getValueFromPath = (path: string): any => {
    const keys = path.split('.');
    let value = settings;
    for (const key of keys) {
      value = value?.[key];
    }
    return value;
  };
  
  const updateSettingByPath = (path: string, value: any) => {
    updateSettings((draft) => {
      const keys = path.split('.');
      let obj: any = draft;
      for (let i = 0; i < keys.length - 1; i++) {
        if (!obj[keys[i]]) obj[keys[i]] = {};
        obj = obj[keys[i]];
      }
      obj[keys[keys.length - 1]] = value;
    });
  };
  
  const handleCommitSettings = () => {
    // Settings are already committed via updateSettings
    // This could trigger additional actions like saving to localStorage
    // or sending telemetry
    console.log('Settings committed for section:', activeSection);
  };

  // Get section settings based on active section
  const getSectionSettings = () => {
    switch (activeSection) {
      case 'appearance':
        return {
          title: 'Appearance Settings',
          fields: [
            { key: 'nodeColor', label: 'Node Color', type: 'color', path: 'visualisation.graphs.logseq.nodes.baseColor' },
            { key: 'nodeSize', label: 'Node Size', type: 'slider', min: 0.1, max: 2, path: 'visualisation.graphs.logseq.nodes.nodeSize' },
            { key: 'edgeColor', label: 'Edge Color', type: 'color', path: 'visualisation.graphs.logseq.edges.color' },
            { key: 'labelSize', label: 'Label Size', type: 'slider', min: 8, max: 24, path: 'visualisation.graphs.logseq.labels.desktopFontSize' }
          ]
        };
      case 'physics':
        return {
          title: 'Physics Settings',
          fields: [
            { key: 'enabled', label: 'Physics Enabled', type: 'toggle', path: 'visualisation.graphs.logseq.physics.enabled' },
            { key: 'damping', label: 'Damping', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.physics.damping' },
            { key: 'repulsion', label: 'Repulsion', type: 'slider', min: 0, max: 100, path: 'visualisation.graphs.logseq.physics.repulsionStrength' },
            { key: 'attraction', label: 'Attraction', type: 'slider', min: 0, max: 1, path: 'visualisation.graphs.logseq.physics.attractionStrength' }
          ]
        };
      case 'visual':
        return {
          title: 'Visual Effects',
          fields: [
            { key: 'bloom', label: 'Bloom', type: 'toggle', path: 'visualisation.bloom.enabled' },
            { key: 'bloomStrength', label: 'Bloom Strength', type: 'slider', min: 0, max: 5, path: 'visualisation.bloom.strength' },
            { key: 'hologram', label: 'Hologram', type: 'toggle', path: 'visualisation.graphs.logseq.nodes.enableHologram' },
            { key: 'flowEffect', label: 'Edge Flow', type: 'toggle', path: 'visualisation.graphs.logseq.edges.enableFlowEffect' }
          ]
        };
      case 'auth':
        return {
          title: 'Authentication Settings',
          fields: [
            { key: 'enabled', label: 'Auth Required', type: 'toggle', path: 'system.auth.enabled' },
            { key: 'timeout', label: 'Session Timeout', type: 'slider', min: 5, max: 60, path: 'system.auth.sessionTimeout' },
            { key: 'autoLogin', label: 'Auto Login', type: 'toggle', path: 'system.auth.autoLogin' }
          ]
        };
      case 'data':
        return {
          title: 'Data Settings',
          fields: [
            { key: 'autoSave', label: 'Auto Save', type: 'toggle', path: 'data.autoSave' },
            { key: 'saveInterval', label: 'Save Interval (min)', type: 'slider', min: 1, max: 30, path: 'data.saveInterval' },
            { key: 'cacheSize', label: 'Cache Size (MB)', type: 'slider', min: 10, max: 500, path: 'data.cacheSize' },
            { key: 'compression', label: 'Data Compression', type: 'toggle', path: 'data.compression' }
          ]
        };
      case 'info':
        return {
          title: 'System Information',
          fields: [
            { key: 'showFPS', label: 'Show FPS', type: 'toggle', path: 'system.debug.showFPS' },
            { key: 'showMemory', label: 'Show Memory', type: 'toggle', path: 'system.debug.showMemory' },
            { key: 'logLevel', label: 'Log Level', type: 'slider', min: 0, max: 3, path: 'system.debug.logLevel' },
            { key: 'telemetry', label: 'Telemetry', type: 'toggle', path: 'system.debug.enableTelemetry' }
          ]
        };
      default:
        return null;
    }
  };

  if (!isExpanded) {
    // Collapsed state
    return (
      <div style={{
        position: 'absolute',
        top: 10,
        left: 10,
        width: '40px',
        height: '40px',
        backgroundColor: 'rgba(0,0,0,0.8)',
        border: '1px solid rgba(255,255,255,0.3)',
        borderRadius: '5px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        cursor: 'pointer'
      }}
      onClick={() => setIsExpanded(true)}
      >
        <div style={{
          width: '12px',
          height: '12px',
          backgroundColor: '#E74C3C',
          borderRadius: '50%',
          boxShadow: '0 0 5px rgba(231, 76, 60, 0.5)'
        }} />
      </div>
    );
  }

  const sectionSettings = getSectionSettings();

  // Expanded state
  return (
    <div style={{
      position: 'absolute',
      top: 10,
      left: 10,
      color: 'white',
      fontFamily: 'monospace',
      fontSize: '12px',
      backgroundColor: 'rgba(0,0,0,0.85)',
      padding: '10px',
      borderRadius: '5px',
      border: '1px solid rgba(255,255,255,0.3)',
      minWidth: '320px',
      maxHeight: '90vh',
      overflowY: 'auto'
    }}>
      {/* Header with fold button */}
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        marginBottom: '10px'
      }}>
        <div style={{ fontWeight: 'bold' }}>Control Center</div>
        <div 
          style={{
            width: '16px',
            height: '16px',
            backgroundColor: '#2ECC71',
            borderRadius: '50%',
            cursor: 'pointer',
            boxShadow: '0 0 5px rgba(46, 204, 113, 0.5)'
          }}
          onClick={() => setIsExpanded(false)}
          title="Fold panel"
        />
      </div>

      {/* System info */}
      <div style={{ marginBottom: '10px', paddingBottom: '10px', borderBottom: '1px solid rgba(255,255,255,0.2)' }}>
        <div>Stats: {showStats ? 'ON' : 'OFF'}</div>
        <div>Bloom: {enableBloom ? 'ON' : 'OFF'}</div>
      </div>

      {/* VisionFlow Status */}
      {swarmData && (
        <div style={{ marginBottom: '10px', paddingBottom: '10px', borderBottom: '1px solid rgba(255,255,255,0.2)' }}>
          <div style={{ fontWeight: 'bold', marginBottom: '8px', color: '#F1C40F' }}>
            ⚡ VisionFlow ({swarmData.dataSource.toUpperCase()})
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: 'auto auto', gap: '3px 15px', fontSize: '11px' }}>
            <span style={{ opacity: 0.7 }}>Agents:</span>
            <span style={{ color: '#F1C40F' }}>{swarmData.nodeCount}</span>
            <span style={{ opacity: 0.7 }}>Links:</span>
            <span style={{ color: '#F1C40F' }}>{swarmData.edgeCount}</span>
            <span style={{ opacity: 0.7 }}>Tokens:</span>
            <span style={{ color: '#F39C12' }}>{swarmData.tokenCount.toLocaleString()}</span>
          </div>
        </div>
      )}

      {/* SpacePilot Menu Controls */}
      <div>
        <div style={{ fontWeight: 'bold', marginBottom: '8px' }}>SpacePilot Controls</div>
        
        {/* Connection status */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
          <span style={{ fontSize: '11px' }}>Status:</span>
          {!webHidAvailable ? (
            <span style={{ fontSize: '11px', color: '#E74C3C' }}>WebHID not available</span>
          ) : spacePilotConnected ? (
            <>
              <span style={{ 
                width: '8px', 
                height: '8px', 
                borderRadius: '50%',
                backgroundColor: '#2ECC71',
                display: 'inline-block'
              }}></span>
              <span style={{ fontSize: '11px', color: '#2ECC71' }}>Connected</span>
            </>
          ) : (
            <>
              <span style={{ 
                width: '8px', 
                height: '8px', 
                borderRadius: '50%',
                backgroundColor: '#E74C3C',
                display: 'inline-block'
              }}></span>
              <button
                onClick={handleConnectClick}
                style={{
                  background: '#3498DB',
                  color: 'white',
                  border: 'none',
                  borderRadius: '3px',
                  padding: '2px 8px',
                  fontSize: '11px',
                  cursor: 'pointer'
                }}
              >
                Connect
              </button>
            </>
          )}
        </div>

        {/* Menu Buttons (1-6) */}
        {spacePilotConnected && (
          <>
            <div style={{ fontSize: '10px', marginBottom: '4px', opacity: 0.7 }}>Menu Sections:</div>
            <div style={{ 
              display: 'grid', 
              gridTemplateColumns: 'repeat(3, 1fr)', 
              gap: '4px',
              marginBottom: '12px'
            }}>
              {Object.entries(BUTTON_MENU_MAP).map(([btnNum, menu]) => {
                const isPressed = spacePilotButtons.includes(`[${btnNum}]`);
                const isActive = activeSection === menu.id;
                return (
                  <div
                    key={btnNum}
                    style={{
                      padding: '6px 8px',
                      borderRadius: '3px',
                      border: `1px solid ${isActive ? '#F1C40F' : (isPressed ? '#2ECC71' : '#555')}`,
                      background: isActive ? 'rgba(241, 196, 15, 0.2)' : (isPressed ? 'rgba(46, 204, 113, 0.1)' : 'rgba(255, 255, 255, 0.05)'),
                      fontSize: '11px',
                      color: isActive ? '#F1C40F' : (isPressed ? '#2ECC71' : '#888'),
                      transition: 'all 0.1s',
                      cursor: 'pointer',
                      textAlign: 'center'
                    }}
                    onClick={() => setActiveSection(menu.id)}
                  >
                    {btnNum}. {menu.label}
                  </div>
                );
              })}
            </div>

            {/* Active Section Settings */}
            {activeSection && sectionSettings && (
              <div style={{
                marginTop: '12px',
                padding: '8px',
                border: '1px solid rgba(241, 196, 15, 0.3)',
                borderRadius: '4px',
                background: 'rgba(241, 196, 15, 0.05)'
              }}>
                <div style={{ fontWeight: 'bold', marginBottom: '8px', color: '#F1C40F' }}>
                  {sectionSettings.title}
                </div>
                <div style={{ fontSize: '11px' }}>
                  {sectionSettings.fields.map((field, index) => {
                    const currentValue = getValueFromPath(field.path);
                    const isSelected = selectedFieldIndex === index;
                    
                    return (
                      <div 
                        key={field.key}
                        style={{
                          padding: '6px',
                          marginBottom: '4px',
                          background: isSelected ? 'rgba(241, 196, 15, 0.1)' : 'transparent',
                          border: isSelected ? '1px solid rgba(241, 196, 15, 0.3)' : '1px solid transparent',
                          borderRadius: '3px',
                          transition: 'all 0.1s'
                        }}
                      >
                        <div style={{ 
                          display: 'flex', 
                          justifyContent: 'space-between',
                          alignItems: 'center',
                          marginBottom: '2px' 
                        }}>
                          <span>{field.label}</span>
                          {isSelected && (
                            <span style={{ fontSize: '9px', color: '#F1C40F' }}>◀ ▶</span>
                          )}
                        </div>
                        
                        {/* Value display based on type */}
                        <div style={{ 
                          opacity: 0.8, 
                          fontSize: '10px',
                          display: 'flex',
                          alignItems: 'center',
                          gap: '8px'
                        }}>
                          {field.type === 'slider' && (
                            <>
                              <div style={{
                                flex: 1,
                                height: '4px',
                                background: 'rgba(255,255,255,0.1)',
                                borderRadius: '2px',
                                position: 'relative'
                              }}>
                                <div style={{
                                  position: 'absolute',
                                  left: 0,
                                  top: 0,
                                  height: '100%',
                                  width: `${((currentValue - field.min) / (field.max - field.min)) * 100}%`,
                                  background: isSelected ? '#F1C40F' : '#888',
                                  borderRadius: '2px',
                                  transition: 'width 0.1s'
                                }} />
                              </div>
                              <span style={{ minWidth: '40px', textAlign: 'right' }}>
                                {typeof currentValue === 'number' ? currentValue.toFixed(1) : '0.0'}
                              </span>
                            </>
                          )}
                          
                          {field.type === 'toggle' && (
                            <div style={{
                              display: 'flex',
                              alignItems: 'center',
                              gap: '4px'
                            }}>
                              <div style={{
                                width: '30px',
                                height: '16px',
                                borderRadius: '8px',
                                background: currentValue ? '#2ECC71' : '#555',
                                position: 'relative',
                                transition: 'background 0.2s'
                              }}>
                                <div style={{
                                  position: 'absolute',
                                  top: '2px',
                                  left: currentValue ? '16px' : '2px',
                                  width: '12px',
                                  height: '12px',
                                  borderRadius: '50%',
                                  background: 'white',
                                  transition: 'left 0.2s'
                                }} />
                              </div>
                              <span>{currentValue ? 'ON' : 'OFF'}</span>
                            </div>
                          )}
                          
                          {field.type === 'color' && (
                            <div style={{
                              display: 'flex',
                              alignItems: 'center',
                              gap: '4px'
                            }}>
                              <div style={{
                                width: '20px',
                                height: '20px',
                                borderRadius: '3px',
                                background: currentValue || '#888',
                                border: '1px solid rgba(255,255,255,0.3)'
                              }} />
                              <span>{currentValue || '#888888'}</span>
                            </div>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
                
                {/* Navigation hints */}
                <div style={{
                  marginTop: '8px',
                  paddingTop: '8px',
                  borderTop: '1px solid rgba(255,255,255,0.2)',
                  fontSize: '9px',
                  opacity: 0.6
                }}>
                  7/A: Navigate ↑↓ | 8/9: Adjust ←→ | F: Commit
                </div>
              </div>
            )}

            {/* Raw input values (debug) */}
            <div style={{ 
              marginTop: '8px',
              paddingTop: '8px',
              borderTop: '1px solid rgba(255,255,255,0.2)',
              fontSize: '10px',
              fontFamily: 'monospace',
              opacity: 0.5
            }}>
              <div style={{ marginBottom: '4px' }}>Raw Input:</div>
              <div style={{ 
                display: 'grid', 
                gridTemplateColumns: 'repeat(3, 1fr)', 
                gap: '5px',
                fontSize: '9px'
              }}>
                <div>X: {spacePilotRawInput.translation.x}</div>
                <div>Y: {spacePilotRawInput.translation.y}</div>
                <div>Z: {spacePilotRawInput.translation.z}</div>
                <div>RX: {spacePilotRawInput.rotation.rx}</div>
                <div>RY: {spacePilotRawInput.rotation.ry}</div>
                <div>RZ: {spacePilotRawInput.rotation.rz}</div>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
};