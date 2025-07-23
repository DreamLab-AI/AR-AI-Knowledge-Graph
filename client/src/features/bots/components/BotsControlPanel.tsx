import React, { useState, useEffect } from 'react';
import { Html } from '@react-three/drei';
import { 
  configurationMapper, 
  VisualizationConfig 
} from '../services/ConfigurationMapper';
import { mockDataGenerator } from '../services/MockDataGenerator';
import { BotsAgent } from '../types/BotsTypes';

interface BotsControlPanelProps {
  position?: [number, number, number];
  onConfigChange?: (config: VisualizationConfig) => void;
}

export const BotsControlPanel: React.FC<BotsControlPanelProps> = ({ 
  position = [30, 10, 0],
  onConfigChange 
}) => {
  const [config, setConfig] = useState<VisualizationConfig>(
    configurationMapper.getConfig()
  );
  const [activeTab, setActiveTab] = useState<'colors' | 'animation' | 'physics' | 'rendering'>('colors');
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [agentCount, setAgentCount] = useState(12);

  useEffect(() => {
    // Subscribe to configuration changes
    const id = 'control-panel';
    configurationMapper.subscribe(id, (newConfig) => {
      setConfig(newConfig);
      onConfigChange?.(newConfig);
    });

    return () => {
      configurationMapper.unsubscribe(id);
    };
  }, [onConfigChange]);

  const handleColorChange = (path: string, value: string) => {
    configurationMapper.updatePath(path, value);
  };

  const handleNumberChange = (path: string, value: number) => {
    configurationMapper.updatePath(path, value);
  };

  const handlePresetChange = (preset: string) => {
    configurationMapper.applyPreset(preset);
  };

  const handleAddAgent = (type: BotsAgent['type']) => {
    mockDataGenerator.addAgent(type);
    setAgentCount(prev => prev + 1);
  };

  const handleExportConfig = () => {
    const json = configurationMapper.exportConfig();
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'bots-visualization-config.json';
    a.click();
    URL.revokeObjectURL(url);
  };

  const panelStyle: React.CSSProperties = {
    background: 'rgba(0, 0, 0, 0.95)',
    border: '2px solid #F1C40F',
    borderRadius: '10px',
    padding: isCollapsed ? '10px' : '20px',
    color: '#FFFFFF',
    fontFamily: 'monospace',
    fontSize: '12px',
    width: isCollapsed ? 'auto' : '350px',
    maxHeight: '600px',
    overflowY: 'auto',
    backdropFilter: 'blur(10px)',
    boxShadow: '0 0 20px rgba(241, 196, 15, 0.3)',
  };

  const tabStyle = (isActive: boolean): React.CSSProperties => ({
    padding: '5px 10px',
    margin: '0 5px',
    background: isActive ? '#F1C40F' : 'transparent',
    color: isActive ? '#000' : '#F1C40F',
    border: '1px solid #F1C40F',
    borderRadius: '5px',
    cursor: 'pointer',
    transition: 'all 0.3s ease',
  });

  const inputStyle: React.CSSProperties = {
    background: 'rgba(241, 196, 15, 0.1)',
    border: '1px solid #F1C40F',
    borderRadius: '3px',
    color: '#F1C40F',
    padding: '3px 5px',
    width: '100%',
    marginTop: '3px',
  };

  const colorInputStyle: React.CSSProperties = {
    ...inputStyle,
    cursor: 'pointer',
    height: '25px',
  };

  return (
    <Html position={position} style={{ pointerEvents: 'auto' }}>
      <div style={panelStyle}>
        <div style={{ 
          display: 'flex', 
          justifyContent: 'space-between', 
          alignItems: 'center',
          marginBottom: isCollapsed ? '0' : '15px'
        }}>
          <h3 style={{ margin: 0, color: '#F1C40F' }}>
            {isCollapsed ? '⚙️' : '⚙️ Bots Control Panel'}
          </h3>
          <button
            onClick={() => setIsCollapsed(!isCollapsed)}
            style={{
              background: 'transparent',
              border: '1px solid #F1C40F',
              color: '#F1C40F',
              borderRadius: '3px',
              cursor: 'pointer',
              padding: '2px 8px',
            }}
          >
            {isCollapsed ? '➕' : '➖'}
          </button>
        </div>

        {!isCollapsed && (
          <>
            {/* Tabs */}
            <div style={{ display: 'flex', marginBottom: '15px', flexWrap: 'wrap' }}>
              <button
                style={tabStyle(activeTab === 'colors')}
                onClick={() => setActiveTab('colors')}
              >
                Colors
              </button>
              <button
                style={tabStyle(activeTab === 'animation')}
                onClick={() => setActiveTab('animation')}
              >
                Animation
              </button>
              <button
                style={tabStyle(activeTab === 'physics')}
                onClick={() => setActiveTab('physics')}
              >
                Physics
              </button>
              <button
                style={tabStyle(activeTab === 'rendering')}
                onClick={() => setActiveTab('rendering')}
              >
                Rendering
              </button>
            </div>

            {/* Colors Tab */}
            {activeTab === 'colors' && (
              <div>
                <h4 style={{ color: '#F1C40F', marginBottom: '10px' }}>Agent Colors</h4>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px' }}>
                  {Object.entries(config.colors.agents).map(([type, color]) => (
                    <div key={type}>
                      <label style={{ fontSize: '11px' }}>
                        {type.charAt(0).toUpperCase() + type.slice(1)}
                      </label>
                      <input
                        type="color"
                        value={color}
                        onChange={(e) => handleColorChange(`colors.agents.${type}`, e.target.value)}
                        style={colorInputStyle}
                      />
                    </div>
                  ))}
                </div>

                <h4 style={{ color: '#F1C40F', marginTop: '15px', marginBottom: '10px' }}>
                  Health Colors
                </h4>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px' }}>
                  {Object.entries(config.colors.health).map(([level, color]) => (
                    <div key={level}>
                      <label style={{ fontSize: '11px' }}>
                        {level.charAt(0).toUpperCase() + level.slice(1)}
                      </label>
                      <input
                        type="color"
                        value={color}
                        onChange={(e) => handleColorChange(`colors.health.${level}`, e.target.value)}
                        style={colorInputStyle}
                      />
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Animation Tab */}
            {activeTab === 'animation' && (
              <div>
                <h4 style={{ color: '#F1C40F', marginBottom: '10px' }}>Animation Settings</h4>
                <div style={{ marginBottom: '10px' }}>
                  <label>Pulse Speed</label>
                  <input
                    type="range"
                    min="0.1"
                    max="5"
                    step="0.1"
                    value={config.animation.pulseSpeed}
                    onChange={(e) => handleNumberChange('animation.pulseSpeed', parseFloat(e.target.value))}
                    style={inputStyle}
                  />
                  <span style={{ fontSize: '10px', color: '#999' }}>
                    {config.animation.pulseSpeed.toFixed(1)}
                  </span>
                </div>

                <div style={{ marginBottom: '10px' }}>
                  <label>Pulse Amplitude</label>
                  <input
                    type="range"
                    min="0.01"
                    max="0.5"
                    step="0.01"
                    value={config.animation.pulseAmplitude}
                    onChange={(e) => handleNumberChange('animation.pulseAmplitude', parseFloat(e.target.value))}
                    style={inputStyle}
                  />
                  <span style={{ fontSize: '10px', color: '#999' }}>
                    {config.animation.pulseAmplitude.toFixed(2)}
                  </span>
                </div>

                <div style={{ marginBottom: '10px' }}>
                  <label>Particle Count</label>
                  <input
                    type="range"
                    min="2"
                    max="16"
                    step="1"
                    value={config.animation.particleCount}
                    onChange={(e) => handleNumberChange('animation.particleCount', parseInt(e.target.value))}
                    style={inputStyle}
                  />
                  <span style={{ fontSize: '10px', color: '#999' }}>
                    {config.animation.particleCount}
                  </span>
                </div>

                <div style={{ marginBottom: '10px' }}>
                  <label>Edge Activity Threshold (ms)</label>
                  <input
                    type="range"
                    min="1000"
                    max="10000"
                    step="500"
                    value={config.animation.edgeActivityThreshold}
                    onChange={(e) => handleNumberChange('animation.edgeActivityThreshold', parseInt(e.target.value))}
                    style={inputStyle}
                  />
                  <span style={{ fontSize: '10px', color: '#999' }}>
                    {config.animation.edgeActivityThreshold}ms
                  </span>
                </div>
              </div>
            )}

            {/* Physics Tab */}
            {activeTab === 'physics' && (
              <div>
                <h4 style={{ color: '#F1C40F', marginBottom: '10px' }}>Physics Settings</h4>
                {Object.entries(config.physics).map(([key, value]) => (
                  <div key={key} style={{ marginBottom: '10px' }}>
                    <label>
                      {key.replace(/([A-Z])/g, ' $1').trim()}
                    </label>
                    <input
                      type="range"
                      min={key === 'damping' ? '0.5' : '0.1'}
                      max={key === 'damping' ? '1' : key === 'linkDistance' ? '50' : '1'}
                      step="0.01"
                      value={value}
                      onChange={(e) => handleNumberChange(`physics.${key}`, parseFloat(e.target.value))}
                      style={inputStyle}
                    />
                    <span style={{ fontSize: '10px', color: '#999' }}>
                      {(value as number).toFixed(2)}
                    </span>
                  </div>
                ))}
              </div>
            )}

            {/* Rendering Tab */}
            {activeTab === 'rendering' && (
              <div>
                <h4 style={{ color: '#F1C40F', marginBottom: '10px' }}>Rendering Settings</h4>
                <div style={{ marginBottom: '10px' }}>
                  <label>Metalness</label>
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.05"
                    value={config.rendering.metalness}
                    onChange={(e) => handleNumberChange('rendering.metalness', parseFloat(e.target.value))}
                    style={inputStyle}
                  />
                  <span style={{ fontSize: '10px', color: '#999' }}>
                    {config.rendering.metalness.toFixed(2)}
                  </span>
                </div>

                <div style={{ marginBottom: '10px' }}>
                  <label>Roughness</label>
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.05"
                    value={config.rendering.roughness}
                    onChange={(e) => handleNumberChange('rendering.roughness', parseFloat(e.target.value))}
                    style={inputStyle}
                  />
                  <span style={{ fontSize: '10px', color: '#999' }}>
                    {config.rendering.roughness.toFixed(2)}
                  </span>
                </div>

                <div style={{ marginBottom: '10px' }}>
                  <label>Node Opacity</label>
                  <input
                    type="range"
                    min="0.1"
                    max="1"
                    step="0.05"
                    value={config.rendering.nodeOpacity}
                    onChange={(e) => handleNumberChange('rendering.nodeOpacity', parseFloat(e.target.value))}
                    style={inputStyle}
                  />
                  <span style={{ fontSize: '10px', color: '#999' }}>
                    {config.rendering.nodeOpacity.toFixed(2)}
                  </span>
                </div>

                <div style={{ marginBottom: '10px' }}>
                  <label>
                    <input
                      type="checkbox"
                      checked={config.rendering.enableShadows}
                      onChange={(e) => handleNumberChange('rendering.enableShadows', e.target.checked ? 1 : 0)}
                      style={{ marginRight: '5px' }}
                    />
                    Enable Shadows
                  </label>
                </div>

                <div style={{ marginBottom: '10px' }}>
                  <label>
                    <input
                      type="checkbox"
                      checked={config.rendering.enablePostProcessing}
                      onChange={(e) => handleNumberChange('rendering.enablePostProcessing', e.target.checked ? 1 : 0)}
                      style={{ marginRight: '5px' }}
                    />
                    Enable Post Processing
                  </label>
                </div>
              </div>
            )}

            {/* Presets and Actions */}
            <div style={{ 
              borderTop: '1px solid #F1C40F', 
              marginTop: '20px', 
              paddingTop: '15px' 
            }}>
              <h4 style={{ color: '#F1C40F', marginBottom: '10px' }}>Presets</h4>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '5px' }}>
                <button
                  onClick={() => handlePresetChange('default')}
                  style={{
                    ...inputStyle,
                    cursor: 'pointer',
                    padding: '5px',
                  }}
                >
                  Default
                </button>
                <button
                  onClick={() => handlePresetChange('highPerformance')}
                  style={{
                    ...inputStyle,
                    cursor: 'pointer',
                    padding: '5px',
                  }}
                >
                  High Performance
                </button>
                <button
                  onClick={() => handlePresetChange('darkMode')}
                  style={{
                    ...inputStyle,
                    cursor: 'pointer',
                    padding: '5px',
                  }}
                >
                  Dark Mode
                </button>
                <button
                  onClick={() => handlePresetChange('presentation')}
                  style={{
                    ...inputStyle,
                    cursor: 'pointer',
                    padding: '5px',
                  }}
                >
                  Presentation
                </button>
              </div>

              <h4 style={{ color: '#F1C40F', marginTop: '15px', marginBottom: '10px' }}>
                Agent Management
              </h4>
              <div style={{ marginBottom: '10px' }}>
                <span style={{ fontSize: '11px' }}>Active Agents: {agentCount}</span>
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '5px' }}>
                <button
                  onClick={() => handleAddAgent('coder')}
                  style={{
                    ...inputStyle,
                    cursor: 'pointer',
                    padding: '5px',
                    fontSize: '11px',
                  }}
                >
                  + Coder
                </button>
                <button
                  onClick={() => handleAddAgent('tester')}
                  style={{
                    ...inputStyle,
                    cursor: 'pointer',
                    padding: '5px',
                    fontSize: '11px',
                  }}
                >
                  + Tester
                </button>
                <button
                  onClick={() => handleAddAgent('analyst')}
                  style={{
                    ...inputStyle,
                    cursor: 'pointer',
                    padding: '5px',
                    fontSize: '11px',
                  }}
                >
                  + Analyst
                </button>
              </div>

              <button
                onClick={handleExportConfig}
                style={{
                  ...inputStyle,
                  cursor: 'pointer',
                  padding: '8px',
                  marginTop: '15px',
                  width: '100%',
                  background: '#F1C40F',
                  color: '#000',
                  fontWeight: 'bold',
                }}
              >
                Export Configuration
              </button>

              <button
                onClick={() => configurationMapper.resetToDefault()}
                style={{
                  ...inputStyle,
                  cursor: 'pointer',
                  padding: '8px',
                  marginTop: '5px',
                  width: '100%',
                  color: '#E74C3C',
                  borderColor: '#E74C3C',
                }}
              >
                Reset to Default
              </button>
            </div>
          </>
        )}
      </div>
    </Html>
  );
};