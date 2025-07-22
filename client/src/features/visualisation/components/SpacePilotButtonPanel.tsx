import React, { useEffect, useState } from 'react';
import { Card, CardHeader, CardTitle, CardContent } from '../../design-system/components/Card';
import { Tooltip } from '../../design-system/components/Tooltip';
import { cn } from '../../../utils/classNameUtils';

import { SpaceDriver } from '../../../services/SpaceDriverService';

interface SpacePilotButtonPanelProps {
  className?: string;
  compact?: boolean;
  showLabels?: boolean;
  onButtonPress?: (buttonNumber: number) => void;
  buttonLabels?: Record<number, string>;
}

interface ButtonState {
  pressed: boolean;
  label?: string;
}

/**
 * Displays all 16 SpacePilot button states in real-time
 * Follows VisionFlow panel styling conventions
 */
export const SpacePilotButtonPanel: React.FC<SpacePilotButtonPanelProps> = ({
  className = '',
  compact = false,
  showLabels = true,
  onButtonPress,
  buttonLabels = {
    1: 'Menu',
    2: 'Mode',
    3: 'Fit',
    4: 'Top',
    5: 'Right',
    6: 'Front',
    7: 'Roll 90°',
    8: 'Esc',
    9: 'Alt',
    10: 'Shift',
    11: 'Ctrl',
    12: '1',
    13: '2',
    14: '3',
    15: '4',
    16: 'Panel'
  }
}) => {
  const [buttonStates, setButtonStates] = useState<Record<number, ButtonState>>(() => {
    const initialStates: Record<number, ButtonState> = {};
    for (let i = 1; i <= 16; i++) {
      initialStates[i] = { pressed: false, label: buttonLabels[i] };
    }
    return initialStates;
  });

  const [isConnected, setIsConnected] = useState(false);

  useEffect(() => {
    const handleButtons = (event: CustomEvent) => {
      const { buttons } = event.detail;
      
      // Parse button states from the event
      // SpaceDriver returns hex notation: [1] through [F] and [10]
      const newStates: Record<number, ButtonState> = {};
      for (let i = 1; i <= 16; i++) {
        const buttonString = `[${i.toString(16).toUpperCase()}]`;
        const isPressed = buttons.includes(buttonString);
        newStates[i] = { 
          pressed: isPressed,
          label: buttonLabels[i] 
        };
        
        // Trigger callback for newly pressed buttons
        if (isPressed && !buttonStates[i]?.pressed && onButtonPress) {
          onButtonPress(i);
        }
      }
      
      setButtonStates(newStates);
    };

    const handleConnect = () => {
      setIsConnected(true);
    };

    const handleDisconnect = () => {
      setIsConnected(false);
      // Reset all button states
      const resetStates: Record<number, ButtonState> = {};
      for (let i = 1; i <= 16; i++) {
        resetStates[i] = { pressed: false, label: buttonLabels[i] };
      }
      setButtonStates(resetStates);
    };

    // Add event listeners
    SpaceDriver.addEventListener('buttons', handleButtons);
    SpaceDriver.addEventListener('connect', handleConnect);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);

    // Check if already connected
    // Note: SpaceDriver doesn't expose device property directly
    // The connect event will fire if a device is already connected

    return () => {
      SpaceDriver.removeEventListener('buttons', handleButtons);
      SpaceDriver.removeEventListener('connect', handleConnect);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
    };
  }, [buttonStates, onButtonPress, buttonLabels]);

  if (!isConnected) {
    return (
      <Card className={cn('bg-gray-900/80 backdrop-blur-sm border-gray-700', className)}>
        <CardContent className="p-4">
          <div className="text-center text-gray-500 text-sm">
            SpacePilot not connected
          </div>
        </CardContent>
      </Card>
    );
  }

  const buttonSize = compact ? 'w-8 h-8' : 'w-10 h-10';
  const fontSize = compact ? 'text-xs' : 'text-sm';

  return (
    <Card className={cn('bg-gray-900/80 backdrop-blur-sm border-gray-700', className)}>
      <CardHeader className="p-4 pb-2">
        <CardTitle className="text-lg font-medium text-gray-200">
          SpacePilot Buttons
        </CardTitle>
      </CardHeader>
      <CardContent className="p-4 pt-2">
        <div className="grid grid-cols-4 gap-2">
          {Object.entries(buttonStates).map(([buttonNum, state]) => {
            const num = parseInt(buttonNum);
            const isPressed = state.pressed;
            const label = state.label || `B${num}`;

            return (
              <Tooltip key={num} content={`Button ${num}${label ? ` - ${label}` : ''}`}>
                <div
                  className={cn(
                    'relative rounded-md border transition-all duration-100',
                    buttonSize,
                    'flex items-center justify-center',
                    'select-none cursor-default',
                    isPressed
                      ? 'bg-cyan-500/20 border-cyan-400 shadow-[0_0_10px_rgba(34,211,238,0.3)]'
                      : 'bg-gray-800/50 border-gray-700 hover:border-gray-600',
                    isPressed && 'transform scale-95'
                  )}
                >
                  {/* Button number */}
                  <span
                    className={cn(
                      'font-mono font-medium',
                      fontSize,
                      isPressed ? 'text-cyan-300' : 'text-gray-400'
                    )}
                  >
                    {showLabels && label ? (
                      <span className="text-[10px]">{label}</span>
                    ) : (
                      num
                    )}
                  </span>

                  {/* Press indicator */}
                  {isPressed && (
                    <div className="absolute inset-0 rounded-md animate-pulse bg-cyan-400/10" />
                  )}
                </div>
              </Tooltip>
            );
          })}
        </div>

        {/* Button groups separator (visual only) */}
        <div className="mt-3 pt-3 border-t border-gray-700/50">
          <div className="flex justify-between text-xs text-gray-500">
            <span>Navigation (1-8)</span>
            <span>Modifiers (9-11)</span>
            <span>Function (12-16)</span>
          </div>
        </div>

        {/* Active button count */}
        <div className="mt-2 flex items-center justify-between">
          <span className="text-xs text-gray-400">Active:</span>
          <span className="text-xs font-mono text-cyan-400">
            {Object.values(buttonStates).filter(state => state.pressed).length} / 16
          </span>
        </div>
      </CardContent>
    </Card>
  );
};

/**
 * Compact inline button indicator for toolbars
 */
export const SpacePilotButtonIndicator: React.FC<{
  buttonNumber: number;
  className?: string;
}> = ({ buttonNumber, className = '' }) => {
  const [isPressed, setIsPressed] = useState(false);

  useEffect(() => {
    const handleButtons = (event: CustomEvent) => {
      const { buttons } = event.detail;
      const buttonString = `[${buttonNumber.toString(16).toUpperCase()}]`;
      setIsPressed(buttons.includes(buttonString));
    };

    SpaceDriver.addEventListener('buttons', handleButtons);
    return () => {
      SpaceDriver.removeEventListener('buttons', handleButtons);
    };
  }, [buttonNumber]);

  return (
    <div
      className={cn(
        'w-6 h-6 rounded flex items-center justify-center',
        'font-mono text-xs transition-all duration-100',
        isPressed
          ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-400'
          : 'bg-gray-800 text-gray-500 border border-gray-700',
        className
      )}
    >
      {buttonNumber}
    </div>
  );
};