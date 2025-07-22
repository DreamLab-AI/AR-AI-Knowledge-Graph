import React from 'react';
import { Slider } from '../../design-system/components/Slider';
import { Switch } from '../../design-system/components/Switch';
import { Label } from '../../design-system/components/Label';
import { Card } from '../../design-system/components/Card';
import { Button } from '../../design-system/components/Button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../design-system/components/Tabs';
import { SpacePilotConfig } from '../controls/SpacePilotController';
import { RotateCw, Settings2, Move3d, RotateCcw } from 'lucide-react';

interface SpacePilotSettingsProps {
  config: SpacePilotConfig;
  onConfigChange: (config: Partial<SpacePilotConfig>) => void;
  onCalibrate?: () => void;
  onResetDefaults?: () => void;
}

export const SpacePilotSettings: React.FC<SpacePilotSettingsProps> = ({
  config,
  onConfigChange,
  onCalibrate,
  onResetDefaults
}) => {
  const handleSensitivityChange = (
    axis: 'x' | 'y' | 'z',
    type: 'translation' | 'rotation',
    value: number
  ) => {
    const newConfig = { ...config };
    if (type === 'translation') {
      newConfig.translationSensitivity[axis] = value;
    } else {
      newConfig.rotationSensitivity[axis] = value;
    }
    onConfigChange(newConfig);
  };

  const handleAxisToggle = (
    axis: 'x' | 'y' | 'z' | 'rx' | 'ry' | 'rz',
    enabled: boolean
  ) => {
    onConfigChange({
      enabledAxes: {
        ...config.enabledAxes,
        [axis]: enabled
      }
    });
  };

  const handleInvertToggle = (
    axis: 'x' | 'y' | 'z' | 'rx' | 'ry' | 'rz',
    inverted: boolean
  ) => {
    onConfigChange({
      invertAxes: {
        ...config.invertAxes,
        [axis]: inverted
      }
    });
  };

  return (
    <Card className="p-4 space-y-4 bg-gray-900/80 backdrop-blur-sm border-gray-700">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-gray-200 flex items-center gap-2">
          <Settings2 className="w-5 h-5" />
          SpacePilot Settings
        </h3>
        <div className="flex gap-2">
          {onCalibrate && (
            <Button
              size="sm"
              variant="secondary"
              onClick={onCalibrate}
              className="text-xs"
            >
              <RotateCw className="w-3 h-3 mr-1" />
              Calibrate
            </Button>
          )}
          {onResetDefaults && (
            <Button
              size="sm"
              variant="ghost"
              onClick={onResetDefaults}
              className="text-xs"
            >
              Reset
            </Button>
          )}
        </div>
      </div>

      <Tabs defaultValue="sensitivity" className="w-full">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="sensitivity">Sensitivity</TabsTrigger>
          <TabsTrigger value="axes">Axes</TabsTrigger>
          <TabsTrigger value="behavior">Behavior</TabsTrigger>
        </TabsList>

        {/* Sensitivity Tab */}
        <TabsContent value="sensitivity" className="space-y-4 mt-4">
          <div className="space-y-3">
            <div className="flex items-center gap-2 mb-2">
              <Move3d className="w-4 h-4 text-cyan-400" />
              <Label className="text-sm font-medium">Translation Sensitivity</Label>
            </div>
            
            {(['x', 'y', 'z'] as const).map(axis => (
              <div key={`trans-${axis}`} className="space-y-1">
                <div className="flex items-center justify-between">
                  <Label className="text-xs text-gray-400">
                    {axis.toUpperCase()} Axis
                  </Label>
                  <span className="text-xs font-mono text-cyan-400">
                    {config.translationSensitivity[axis].toFixed(1)}x
                  </span>
                </div>
                <Slider
                  value={[config.translationSensitivity[axis]]}
                  onValueChange={([value]) => handleSensitivityChange(axis, 'translation', value)}
                  min={0.1}
                  max={5}
                  step={0.1}
                  className="w-full"
                />
              </div>
            ))}
          </div>

          <div className="space-y-3 pt-4 border-t border-gray-700">
            <div className="flex items-center gap-2 mb-2">
              <RotateCcw className="w-4 h-4 text-cyan-400" />
              <Label className="text-sm font-medium">Rotation Sensitivity</Label>
            </div>
            
            {(['x', 'y', 'z'] as const).map(axis => (
              <div key={`rot-${axis}`} className="space-y-1">
                <div className="flex items-center justify-between">
                  <Label className="text-xs text-gray-400">
                    R{axis.toUpperCase()} Axis
                  </Label>
                  <span className="text-xs font-mono text-cyan-400">
                    {config.rotationSensitivity[axis].toFixed(1)}x
                  </span>
                </div>
                <Slider
                  value={[config.rotationSensitivity[axis]]}
                  onValueChange={([value]) => handleSensitivityChange(axis, 'rotation', value)}
                  min={0.1}
                  max={5}
                  step={0.1}
                  className="w-full"
                />
              </div>
            ))}
          </div>
        </TabsContent>

        {/* Axes Tab */}
        <TabsContent value="axes" className="space-y-4 mt-4">
          <div className="space-y-3">
            <Label className="text-sm font-medium">Enabled Axes</Label>
            <div className="grid grid-cols-2 gap-3">
              {(['x', 'y', 'z', 'rx', 'ry', 'rz'] as const).map(axis => (
                <div key={axis} className="flex items-center justify-between p-2 bg-gray-800 rounded">
                  <Label className="text-xs">
                    {axis.toUpperCase()} {axis.startsWith('r') ? '(Rotation)' : '(Translation)'}
                  </Label>
                  <Switch
                    checked={config.enabledAxes[axis]}
                    onCheckedChange={(checked) => handleAxisToggle(axis, checked)}
                  />
                </div>
              ))}
            </div>
          </div>

          <div className="space-y-3 pt-4 border-t border-gray-700">
            <Label className="text-sm font-medium">Invert Axes</Label>
            <div className="grid grid-cols-2 gap-3">
              {(['x', 'y', 'z', 'rx', 'ry', 'rz'] as const).map(axis => (
                <div key={`inv-${axis}`} className="flex items-center justify-between p-2 bg-gray-800 rounded">
                  <Label className="text-xs">
                    Invert {axis.toUpperCase()}
                  </Label>
                  <Switch
                    checked={config.invertAxes[axis]}
                    onCheckedChange={(checked) => handleInvertToggle(axis, checked)}
                  />
                </div>
              ))}
            </div>
          </div>
        </TabsContent>

        {/* Behavior Tab */}
        <TabsContent value="behavior" className="space-y-4 mt-4">
          <div className="space-y-3">
            <div className="space-y-1">
              <Label className="text-sm">Deadzone</Label>
              <div className="flex items-center justify-between">
                <span className="text-xs text-gray-400">
                  Ignore small movements below this threshold
                </span>
                <span className="text-xs font-mono text-cyan-400">
                  {(config.deadzone * 100).toFixed(0)}%
                </span>
              </div>
              <Slider
                value={[config.deadzone]}
                onValueChange={([value]) => onConfigChange({ deadzone: value })}
                min={0}
                max={0.3}
                step={0.01}
                className="w-full"
              />
            </div>

            <div className="space-y-1 pt-3">
              <Label className="text-sm">Smoothing</Label>
              <div className="flex items-center justify-between">
                <span className="text-xs text-gray-400">
                  Higher values create smoother but slower response
                </span>
                <span className="text-xs font-mono text-cyan-400">
                  {(config.smoothing * 100).toFixed(0)}%
                </span>
              </div>
              <Slider
                value={[config.smoothing]}
                onValueChange={([value]) => onConfigChange({ smoothing: value })}
                min={0}
                max={0.95}
                step={0.05}
                className="w-full"
              />
            </div>

            <div className="space-y-2 pt-3">
              <Label className="text-sm">Control Mode</Label>
              <div className="grid grid-cols-1 gap-2">
                {(['camera', 'object', 'navigation'] as const).map(mode => (
                  <Button
                    key={mode}
                    variant={config.mode === mode ? 'primary' : 'secondary'}
                    size="sm"
                    onClick={() => onConfigChange({ mode })}
                    className="justify-start"
                  >
                    <span className="capitalize">{mode}</span>
                  </Button>
                ))}
              </div>
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </Card>
  );
};