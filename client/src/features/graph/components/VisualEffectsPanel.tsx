import React, { useState } from 'react';
import { useSettingsStore } from '../../../store/settingsStore';
import { createLogger } from '../../../utils/logger';
import { Switch } from '../../design-system/components/Switch';
import { Slider } from '../../design-system/components/Slider';
import { Button } from '../../design-system/components/Button';
import { ChevronDown, ChevronUp, Sparkles, Activity, Zap, Eye } from 'lucide-react';

const logger = createLogger('VisualEffectsPanel');

export const VisualEffectsPanel: React.FC = () => {
  const [isExpanded, setIsExpanded] = useState(false);
  const settings = useSettingsStore(state => state.settings);
  const updateSettings = useSettingsStore(state => state.updateSettings);
  
  // Check if any effect is enabled
  const anyEffectEnabled = settings?.visualisation?.nodes?.enableHologram || 
                          settings?.visualisation?.edges?.enableFlowEffect ||
                          settings?.visualisation?.bloom?.enabled ||
                          settings?.visualisation?.animation?.pulseEnabled;
  
  const handleMasterToggle = () => {
    const newState = !anyEffectEnabled;
    logger.info('Master toggle visual effects', { newState });
    
    updateSettings((draft) => {
      if (draft.visualisation) {
        // Nodes
        if (draft.visualisation.nodes) {
          draft.visualisation.nodes.enableHologram = newState;
        }
        
        // Edges
        if (draft.visualisation.edges) {
          draft.visualisation.edges.enableFlowEffect = newState;
          draft.visualisation.edges.useGradient = newState;
        }
        
        // Bloom
        if (draft.visualisation.bloom) {
          draft.visualisation.bloom.enabled = newState;
        }
        
        // Animation
        if (draft.visualisation.animation) {
          draft.visualisation.animation.pulseEnabled = newState;
          draft.visualisation.animation.enableNodeAnimations = newState;
        }
      }
    });
  };
  
  const handleToggleEffect = (path: string, currentValue: boolean) => {
    logger.info('Toggle effect', { path, newValue: !currentValue });
    updateSettings((draft) => {
      const keys = path.split('.');
      let obj: any = draft;
      for (let i = 0; i < keys.length - 1; i++) {
        obj = obj[keys[i]];
      }
      obj[keys[keys.length - 1]] = !currentValue;
    });
  };
  
  const handleSliderChange = (path: string, value: number) => {
    updateSettings((draft) => {
      const keys = path.split('.');
      let obj: any = draft;
      for (let i = 0; i < keys.length - 1; i++) {
        obj = obj[keys[i]];
      }
      obj[keys[keys.length - 1]] = value;
    });
  };
  
  return (
    <div className="fixed top-4 right-4 z-50 bg-gray-900/95 backdrop-blur-lg border border-cyan-500/30 rounded-lg shadow-2xl transition-all duration-300"
         style={{
           minWidth: '320px',
           boxShadow: anyEffectEnabled ? '0 0 30px rgba(0, 255, 255, 0.3)' : '0 4px 20px rgba(0, 0, 0, 0.5)'
         }}>
      {/* Header */}
      <div className="p-4 border-b border-cyan-500/20">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Sparkles className={`w-5 h-5 ${anyEffectEnabled ? 'text-cyan-400 animate-pulse' : 'text-gray-400'}`} />
            <h3 className="text-lg font-semibold text-white">Visual Effects</h3>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setIsExpanded(!isExpanded)}
            className="p-1"
          >
            {isExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
          </Button>
        </div>
        
        {/* Master Toggle */}
        <div className="flex items-center justify-between bg-gray-800/50 rounded-lg p-3">
          <span className="text-sm font-medium text-gray-200">All Effects</span>
          <Switch
            checked={anyEffectEnabled}
            onCheckedChange={handleMasterToggle}
            className="data-[state=checked]:bg-cyan-500"
          />
        </div>
      </div>
      
      {/* Expanded Controls */}
      {isExpanded && (
        <div className="p-4 space-y-4">
          {/* Hologram Effects */}
          <div className="space-y-3">
            <div className="flex items-center gap-2 text-cyan-400 mb-2">
              <Zap className="w-4 h-4" />
              <span className="text-sm font-medium">Hologram</span>
            </div>
            
            <div className="flex items-center justify-between">
              <span className="text-xs text-gray-300">Enable Hologram</span>
              <Switch
                checked={settings?.visualisation?.nodes?.enableHologram || false}
                onCheckedChange={() => handleToggleEffect('visualisation.nodes.enableHologram', 
                  settings?.visualisation?.nodes?.enableHologram || false)}
                className="scale-90 data-[state=checked]:bg-cyan-500"
              />
            </div>
            
            {settings?.visualisation?.nodes?.enableHologram && (
              <div className="space-y-2 pl-4 border-l border-gray-700">
                <div>
                  <label className="text-xs text-gray-400">Glow Strength</label>
                  <Slider
                    value={[settings?.visualisation?.animation?.pulseStrength || 1.0]}
                    onValueChange={([value]) => handleSliderChange('visualisation.animation.pulseStrength', value)}
                    min={0}
                    max={2}
                    step={0.1}
                    className="mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs text-gray-400">Pulse Speed</label>
                  <Slider
                    value={[settings?.visualisation?.animation?.pulseSpeed || 1.0]}
                    onValueChange={([value]) => handleSliderChange('visualisation.animation.pulseSpeed', value)}
                    min={0.1}
                    max={3}
                    step={0.1}
                    className="mt-1"
                  />
                </div>
              </div>
            )}
          </div>
          
          {/* Flow Effects */}
          <div className="space-y-3 pt-3 border-t border-gray-700">
            <div className="flex items-center gap-2 text-blue-400 mb-2">
              <Activity className="w-4 h-4" />
              <span className="text-sm font-medium">Edge Flow</span>
            </div>
            
            <div className="flex items-center justify-between">
              <span className="text-xs text-gray-300">Enable Flow</span>
              <Switch
                checked={settings?.visualisation?.edges?.enableFlowEffect || false}
                onCheckedChange={() => handleToggleEffect('visualisation.edges.enableFlowEffect',
                  settings?.visualisation?.edges?.enableFlowEffect || false)}
                className="scale-90 data-[state=checked]:bg-blue-500"
              />
            </div>
            
            {settings?.visualisation?.edges?.enableFlowEffect && (
              <div className="space-y-2 pl-4 border-l border-gray-700">
                <div>
                  <label className="text-xs text-gray-400">Flow Speed</label>
                  <Slider
                    value={[settings?.visualisation?.edges?.flowSpeed || 1.0]}
                    onValueChange={([value]) => handleSliderChange('visualisation.edges.flowSpeed', value)}
                    min={0.1}
                    max={5}
                    step={0.1}
                    className="mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs text-gray-400">Flow Intensity</label>
                  <Slider
                    value={[settings?.visualisation?.edges?.flowIntensity || 0.5]}
                    onValueChange={([value]) => handleSliderChange('visualisation.edges.flowIntensity', value)}
                    min={0}
                    max={1}
                    step={0.05}
                    className="mt-1"
                  />
                </div>
                <div className="flex items-center justify-between mt-2">
                  <span className="text-xs text-gray-300">Use Gradient</span>
                  <Switch
                    checked={settings?.visualisation?.edges?.useGradient || false}
                    onCheckedChange={() => handleToggleEffect('visualisation.edges.useGradient',
                      settings?.visualisation?.edges?.useGradient || false)}
                    className="scale-90 data-[state=checked]:bg-blue-500"
                  />
                </div>
              </div>
            )}
          </div>
          
          {/* Bloom Effects */}
          <div className="space-y-3 pt-3 border-t border-gray-700">
            <div className="flex items-center gap-2 text-purple-400 mb-2">
              <Eye className="w-4 h-4" />
              <span className="text-sm font-medium">Bloom & Glow</span>
            </div>
            
            <div className="flex items-center justify-between">
              <span className="text-xs text-gray-300">Enable Bloom</span>
              <Switch
                checked={settings?.visualisation?.bloom?.enabled || false}
                onCheckedChange={() => handleToggleEffect('visualisation.bloom.enabled',
                  settings?.visualisation?.bloom?.enabled || false)}
                className="scale-90 data-[state=checked]:bg-purple-500"
              />
            </div>
            
            {settings?.visualisation?.bloom?.enabled && (
              <div className="space-y-2 pl-4 border-l border-gray-700">
                <div>
                  <label className="text-xs text-gray-400">Bloom Strength</label>
                  <Slider
                    value={[settings?.visualisation?.bloom?.strength || 1.5]}
                    onValueChange={([value]) => handleSliderChange('visualisation.bloom.strength', value)}
                    min={0}
                    max={3}
                    step={0.1}
                    className="mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs text-gray-400">Bloom Radius</label>
                  <Slider
                    value={[settings?.visualisation?.bloom?.radius || 0.4]}
                    onValueChange={([value]) => handleSliderChange('visualisation.bloom.radius', value)}
                    min={0}
                    max={1}
                    step={0.05}
                    className="mt-1"
                  />
                </div>
              </div>
            )}
          </div>
          
          {/* Preset Buttons */}
          <div className="pt-4 border-t border-gray-700 space-y-2">
            <span className="text-xs text-gray-400 block mb-2">Quick Presets</span>
            <div className="grid grid-cols-3 gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  updateSettings((draft) => {
                    // Minimal preset
                    if (draft.visualisation) {
                      draft.visualisation.nodes!.enableHologram = false;
                      draft.visualisation.edges!.enableFlowEffect = false;
                      draft.visualisation.bloom!.enabled = false;
                      draft.visualisation.animation!.pulseEnabled = false;
                    }
                  });
                }}
                className="text-xs"
              >
                Minimal
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  updateSettings((draft) => {
                    // Balanced preset
                    if (draft.visualisation) {
                      draft.visualisation.nodes!.enableHologram = true;
                      draft.visualisation.edges!.enableFlowEffect = true;
                      draft.visualisation.edges!.flowSpeed = 1.0;
                      draft.visualisation.bloom!.enabled = false;
                      draft.visualisation.animation!.pulseEnabled = true;
                      draft.visualisation.animation!.pulseSpeed = 1.0;
                    }
                  });
                }}
                className="text-xs"
              >
                Balanced
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  updateSettings((draft) => {
                    // Maximum preset
                    if (draft.visualisation) {
                      draft.visualisation.nodes!.enableHologram = true;
                      draft.visualisation.edges!.enableFlowEffect = true;
                      draft.visualisation.edges!.useGradient = true;
                      draft.visualisation.edges!.flowSpeed = 2.0;
                      draft.visualisation.bloom!.enabled = true;
                      draft.visualisation.bloom!.strength = 2.0;
                      draft.visualisation.animation!.pulseEnabled = true;
                      draft.visualisation.animation!.pulseSpeed = 1.5;
                    }
                  });
                }}
                className="text-xs"
              >
                Maximum
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};