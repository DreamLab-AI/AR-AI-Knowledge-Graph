import React, { useState, useEffect } from 'react';
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

export const IntegratedControlPanel: React.FC<IntegratedControlPanelProps> = ({ 
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

  // Check WebHID availability
  useEffect(() => {
    setWebHidAvailable('hid' in navigator);
  }, []);

  // Set up SpacePilot event listeners
  useEffect(() => {
    const handleConnect = () => {
      setSpacePilotConnected(true);
      // Disable mouse controls when SpacePilot connects
      if (onOrbitControlsToggle) {
        onOrbitControlsToggle(false);
      }
    };
    
    const handleDisconnect = () => {
      setSpacePilotConnected(false);
      setSpacePilotButtons([]);
      // Re-enable mouse controls when SpacePilot disconnects
      if (onOrbitControlsToggle) {
        onOrbitControlsToggle(true);
      }
    };
    
    const handleButtons = (event: any) => {
      setSpacePilotButtons(event.detail.buttons || []);
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
  }, [onOrbitControlsToggle]);

  const handleConnectClick = async () => {
    try {
      await SpaceDriver.scan();
    } catch (error) {
      console.error('Failed to connect to SpacePilot:', error);
    }
  };

  if (!isExpanded) {
    // Collapsed state - small square with red dot
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

  // Expanded state
  return (
    <div style={{
      position: 'absolute',
      top: 10,
      left: 10,
      color: 'white',
      fontFamily: 'monospace',
      fontSize: '12px',
      backgroundColor: 'rgba(0,0,0,0.8)',
      padding: '10px',
      borderRadius: '5px',
      border: '1px solid rgba(255,255,255,0.3)',
      minWidth: '280px'
    }}>
      {/* Header with fold button */}
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        marginBottom: '10px'
      }}>
        <div style={{ fontWeight: 'bold' }}>Simple Layout - Dual Graph View</div>
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
          
          {/* Connection status */}
          <div style={{ 
            display: 'flex', 
            alignItems: 'center', 
            gap: '8px',
            marginBottom: '8px'
          }}>
            <span style={{ 
              width: '8px', 
              height: '8px', 
              borderRadius: '50%',
              backgroundColor: swarmData.mcpConnected ? '#2ECC71' : '#E74C3C',
              display: 'inline-block'
            }}></span>
            <span style={{ fontSize: '11px', color: swarmData.mcpConnected ? '#2ECC71' : '#E74C3C' }}>
              {swarmData.mcpConnected ? 'MCP Connected' : 'MCP Disconnected'}
            </span>
          </div>
          
          {/* Swarm stats */}
          <div style={{ display: 'grid', gridTemplateColumns: 'auto auto', gap: '3px 15px', fontSize: '11px' }}>
            <span style={{ opacity: 0.7 }}>Agents:</span>
            <span style={{ color: '#F1C40F', fontWeight: 'bold' }}>{swarmData.nodeCount}</span>
            
            <span style={{ opacity: 0.7 }}>Active Links:</span>
            <span style={{ color: '#F1C40F', fontWeight: 'bold' }}>{swarmData.edgeCount}</span>
            
            <span style={{ opacity: 0.7 }}>Total Tokens:</span>
            <span style={{ color: '#F39C12', fontWeight: 'bold' }}>
              {swarmData.tokenCount.toLocaleString()}
            </span>
          </div>
          
          {/* Legend */}
          <div style={{ marginTop: '8px', fontSize: '10px', opacity: 0.8 }}>
            <div style={{ display: 'flex', gap: '12px' }}>
              <span><span style={{ color: '#F1C40F' }}>●</span> Coordinators</span>
              <span><span style={{ color: '#2ECC71' }}>●</span> Workers</span>
            </div>
          </div>
        </div>
      )}

      {/* SpacePilot Controls */}
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
              <span style={{ fontSize: '11px', color: '#2ECC71' }}>Connected (Mouse disabled)</span>
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

        {/* Button telltales */}
        {spacePilotConnected && (
          <>
            <div style={{ fontSize: '10px', marginBottom: '4px', opacity: 0.7 }}>Button States:</div>
            <div style={{ 
              display: 'grid', 
              gridTemplateColumns: 'repeat(8, 1fr)', 
              gap: '2px',
              marginBottom: '8px'
            }}>
              {Array.from({ length: 16 }, (_, i) => {
                const buttonNum = i + 1;
                const buttonHex = buttonNum.toString(16).toUpperCase();
                const isPressed = spacePilotButtons.includes(`[${buttonHex}]`);
                return (
                  <div
                    key={i}
                    style={{
                      width: '20px',
                      height: '20px',
                      borderRadius: '2px',
                      border: `1px solid ${isPressed ? '#2ECC71' : '#555'}`,
                      background: isPressed ? 'rgba(46, 204, 113, 0.3)' : 'rgba(255, 255, 255, 0.05)',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontSize: '9px',
                      color: isPressed ? '#2ECC71' : '#888',
                      fontWeight: isPressed ? 'bold' : 'normal',
                      transition: 'all 0.1s'
                    }}
                  >
                    {buttonHex}
                  </div>
                );
              })}
            </div>

            {/* Raw input values */}
            <div style={{ 
              paddingTop: '8px',
              borderTop: '1px solid rgba(255,255,255,0.2)',
              fontSize: '10px'
            }}>
              <div style={{ marginBottom: '4px', color: '#F39C12' }}>Translation (Raw):</div>
              <div style={{ 
                display: 'grid', 
                gridTemplateColumns: 'repeat(3, 1fr)', 
                gap: '5px',
                marginBottom: '6px'
              }}>
                <div style={{ color: '#E74C3C' }}>X: {spacePilotRawInput.translation.x}</div>
                <div style={{ color: '#2ECC71' }}>Y: {spacePilotRawInput.translation.y}</div>
                <div style={{ color: '#3498DB' }}>Z: {spacePilotRawInput.translation.z}</div>
              </div>
              
              <div style={{ marginBottom: '4px', color: '#F39C12' }}>Rotation (Raw):</div>
              <div style={{ 
                display: 'grid', 
                gridTemplateColumns: 'repeat(3, 1fr)', 
                gap: '5px'
              }}>
                <div style={{ color: '#E74C3C' }}>RX: {spacePilotRawInput.rotation.rx}</div>
                <div style={{ color: '#2ECC71' }}>RY: {spacePilotRawInput.rotation.ry}</div>
                <div style={{ color: '#3498DB' }}>RZ: {spacePilotRawInput.rotation.rz}</div>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
};