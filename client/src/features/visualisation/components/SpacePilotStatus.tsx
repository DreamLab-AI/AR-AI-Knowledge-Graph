import React from 'react';
import { Button } from '../../design-system/components/Button';
import { Badge } from '../../design-system/components/Badge';
import { Tooltip } from '../../design-system/components/Tooltip';
import { Card } from '../../design-system/components/Card';
import { Settings, Gamepad2, Move3d, Box, Navigation } from 'lucide-react';

interface SpacePilotStatusProps {
  connected: boolean;
  mode: 'camera' | 'object' | 'navigation';
  sensitivity?: number;
  onConnect?: () => void;
  onModeChange?: (mode: 'camera' | 'object' | 'navigation') => void;
  onSettingsClick?: () => void;
  className?: string;
}

const modeIcons = {
  camera: Move3d,
  object: Box,
  navigation: Navigation
};

const modeLabels = {
  camera: 'Camera Control',
  object: 'Object Manipulation',
  navigation: 'Navigation Mode'
};

const modeDescriptions = {
  camera: 'Control camera position and rotation',
  object: 'Manipulate selected 3D objects',
  navigation: 'Fly through the scene freely'
};

export const SpacePilotStatus: React.FC<SpacePilotStatusProps> = ({
  connected,
  mode,
  sensitivity = 1,
  onConnect,
  onModeChange,
  onSettingsClick,
  className = ''
}) => {
  const ModeIcon = modeIcons[mode];

  return (
    <Card className={`p-3 bg-gray-900/80 backdrop-blur-sm border-gray-700 ${className}`}>
      <div className="flex items-center justify-between gap-3">
        {/* Connection Status */}
        <div className="flex items-center gap-2">
          <Gamepad2 
            className={`w-5 h-5 ${connected ? 'text-green-500' : 'text-gray-500'}`} 
          />
          <div className="flex flex-col">
            <span className="text-sm font-medium text-gray-200">
              SpacePilot
            </span>
            <Badge 
              variant={connected ? 'success' : 'secondary'}
              className="text-xs"
            >
              {connected ? 'Connected' : 'Disconnected'}
            </Badge>
          </div>
        </div>

        {/* Mode Selector */}
        {connected && (
          <div className="flex items-center gap-2">
            <Tooltip content={modeDescriptions[mode]}>
              <div className="flex items-center gap-1 px-2 py-1 bg-gray-800 rounded-md">
                <ModeIcon className="w-4 h-4 text-cyan-400" />
                <span className="text-xs text-gray-300">
                  {modeLabels[mode]}
                </span>
              </div>
            </Tooltip>

            {/* Mode Switcher */}
            {onModeChange && (
              <div className="flex gap-1">
                {Object.entries(modeIcons).map(([modeKey, Icon]) => (
                  <Tooltip key={modeKey} content={modeDescriptions[modeKey as keyof typeof modeDescriptions]}>
                    <Button
                      size="sm"
                      variant={mode === modeKey ? 'primary' : 'ghost'}
                      onClick={() => onModeChange(modeKey as 'camera' | 'object' | 'navigation')}
                      className="p-1"
                    >
                      <Icon className="w-4 h-4" />
                    </Button>
                  </Tooltip>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Actions */}
        <div className="flex items-center gap-2">
          {!connected && onConnect && (
            <Button 
              size="sm" 
              variant="primary"
              onClick={onConnect}
              className="text-xs"
            >
              Connect
            </Button>
          )}

          {onSettingsClick && (
            <Tooltip content="SpacePilot Settings">
              <Button
                size="sm"
                variant="ghost"
                onClick={onSettingsClick}
                className="p-1"
              >
                <Settings className="w-4 h-4" />
              </Button>
            </Tooltip>
          )}
        </div>
      </div>

      {/* Sensitivity Indicator */}
      {connected && (
        <div className="mt-2 pt-2 border-t border-gray-700">
          <div className="flex items-center justify-between text-xs">
            <span className="text-gray-400">Sensitivity</span>
            <span className="text-cyan-400 font-mono">{sensitivity.toFixed(1)}x</span>
          </div>
          <div className="mt-1 h-1 bg-gray-800 rounded-full overflow-hidden">
            <div 
              className="h-full bg-cyan-400 transition-all duration-200"
              style={{ width: `${Math.min(100, sensitivity * 20)}%` }}
            />
          </div>
        </div>
      )}
    </Card>
  );
};

/**
 * Compact status indicator for embedding in toolbars
 */
export const SpacePilotStatusCompact: React.FC<{
  connected: boolean;
  mode?: 'camera' | 'object' | 'navigation';
  onClick?: () => void;
}> = ({ connected, mode = 'camera', onClick }) => {
  const ModeIcon = modeIcons[mode];

  return (
    <Tooltip content={connected ? `SpacePilot: ${modeLabels[mode]}` : 'SpacePilot Disconnected'}>
      <Button
        variant="ghost"
        size="sm"
        onClick={onClick}
        className="relative"
      >
        <Gamepad2 className={`w-4 h-4 ${connected ? 'text-green-500' : 'text-gray-500'}`} />
        {connected && (
          <ModeIcon className="w-3 h-3 text-cyan-400 absolute -bottom-1 -right-1" />
        )}
      </Button>
    </Tooltip>
  );
};

// Note: Badge component needs to be created if not exists
const Badge: React.FC<{
  variant?: 'default' | 'success' | 'secondary';
  className?: string;
  children: React.ReactNode;
}> = ({ variant = 'default', className = '', children }) => {
  const variants = {
    default: 'bg-gray-700 text-gray-200',
    success: 'bg-green-900/50 text-green-400 border-green-800',
    secondary: 'bg-gray-800 text-gray-400'
  };

  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium border ${variants[variant]} ${className}`}>
      {children}
    </span>
  );
};