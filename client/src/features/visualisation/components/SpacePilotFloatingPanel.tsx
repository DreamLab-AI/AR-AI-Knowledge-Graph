import React, { useEffect, useState, useRef } from 'react';
import { Card, CardHeader, CardTitle, CardContent } from '../../design-system/components/Card';
import { Button } from '../../design-system/components/Button';
import { Tooltip } from '../../design-system/components/Tooltip';
import { cn } from '../../../utils/cn';
import { X, Gamepad2, Move } from 'lucide-react';

import { SpaceDriver } from '../../../services/SpaceDriverService';

interface SpacePilotFloatingPanelProps {
  className?: string;
  defaultPosition?: { x: number; y: number };
  onButtonPress?: (buttonNumber: number) => void;
  buttonLabels?: Record<number, string>;
}

interface ButtonState {
  pressed: boolean;
  label?: string;
}

/**
 * Floating draggable SpacePilot control panel with connect button
 */
export const SpacePilotFloatingPanel: React.FC<SpacePilotFloatingPanelProps> = ({
  className = '',
  defaultPosition = { x: 20, y: 100 },
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
  const [isConnecting, setIsConnecting] = useState(false);
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [position, setPosition] = useState(defaultPosition);
  const [isDragging, setIsDragging] = useState(false);
  const [webHidAvailable, setWebHidAvailable] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string>('');
  const dragRef = useRef<HTMLDivElement>(null);
  const dragStartPos = useRef({ x: 0, y: 0 });

  // Handle device connection
  const handleConnect = async () => {
    try {
      setIsConnecting(true);
      await SpaceDriver.scan();
      // The connect event will fire when device is connected
    } catch (error) {
      console.error('Failed to connect to SpacePilot:', error);
      setIsConnecting(false);
    }
  };

  const handleDisconnect = () => {
    // SpaceDriver doesn't have a disconnect method, but we can handle the UI state
    setIsConnected(false);
  };

  // Dragging functionality
  const handleMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest('button')) return;
    
    setIsDragging(true);
    const rect = dragRef.current?.getBoundingClientRect();
    if (rect) {
      dragStartPos.current = {
        x: e.clientX - rect.left,
        y: e.clientY - rect.top
      };
    }
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isDragging) return;
      
      setPosition({
        x: e.clientX - dragStartPos.current.x,
        y: e.clientY - dragStartPos.current.y
      });
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    if (isDragging) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging]);

  // Check WebHID availability on mount
  useEffect(() => {
    const checkWebHid = () => {
      if (!navigator.hid) {
        setWebHidAvailable(false);
        if (window.isSecureContext === false) {
          setErrorMessage('WebHID requires HTTPS or localhost');
        } else {
          setErrorMessage('WebHID not supported. Use Chrome or Edge.');
        }
      } else {
        setWebHidAvailable(true);
        setErrorMessage('');
      }
    };
    
    checkWebHid();
  }, []);

  useEffect(() => {
    const handleButtons = (event: CustomEvent) => {
      const { buttons } = event.detail;
      
      // Parse button states from the event
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

    const handleConnectEvent = () => {
      setIsConnected(true);
      setIsConnecting(false);
    };

    const handleDisconnectEvent = () => {
      setIsConnected(false);
      setIsConnecting(false);
      // Reset all button states
      const resetStates: Record<number, ButtonState> = {};
      for (let i = 1; i <= 16; i++) {
        resetStates[i] = { pressed: false, label: buttonLabels[i] };
      }
      setButtonStates(resetStates);
    };

    // Add event listeners
    SpaceDriver.addEventListener('buttons', handleButtons);
    SpaceDriver.addEventListener('connect', handleConnectEvent);
    SpaceDriver.addEventListener('disconnect', handleDisconnectEvent);

    return () => {
      SpaceDriver.removeEventListener('buttons', handleButtons);
      SpaceDriver.removeEventListener('connect', handleConnectEvent);
      SpaceDriver.removeEventListener('disconnect', handleDisconnectEvent);
    };
  }, [buttonStates, onButtonPress, buttonLabels]);

  return (
    <div
      ref={dragRef}
      className={cn(
        'fixed z-50',
        'transition-shadow duration-200',
        isDragging ? 'cursor-grabbing' : 'cursor-grab',
        className
      )}
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`,
        width: isCollapsed ? 'auto' : '320px'
      }}
      onMouseDown={handleMouseDown}
    >
      <Card className={cn(
        'bg-gray-900/95 backdrop-blur-md',
        isConnected ? 'border-green-500/50' : 'border-gray-700',
        'shadow-lg',
        isDragging && 'shadow-xl'
      )}>
        <CardHeader className="p-3 pb-2">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Gamepad2 className={cn("w-5 h-5", isConnected ? "text-green-400" : "text-gray-400")} />
              <CardTitle className="text-base font-medium text-gray-200">
                WebHID SpacePilot
              </CardTitle>
            </div>
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="sm"
                className="p-1 h-6 w-6"
                onClick={() => setIsCollapsed(!isCollapsed)}
              >
                {isCollapsed ? '+' : '-'}
              </Button>
            </div>
          </div>
        </CardHeader>

        {!isCollapsed && (
          <CardContent className="p-3 pt-2">
            {/* Connection status and button */}
            <div className="mb-4">
              {!webHidAvailable ? (
                <div className="space-y-3">
                  <div className="text-center">
                    <div className="text-red-400 text-sm font-medium">WebHID Blocked</div>
                    <div className="text-gray-400 text-xs mt-1">{errorMessage}</div>
                  </div>
                  <div className="bg-gray-800/50 rounded p-2 text-xs space-y-2">
                    <div className="text-gray-300 font-medium">To enable WebHID:</div>
                    <div className="text-gray-400 space-y-1">
                      <div>1. Access via <span className="text-cyan-400">localhost:3000</span></div>
                      <div>2. Or use HTTPS</div>
                      <div>3. Or in Chrome flags:</div>
                      <div className="ml-2 text-gray-500">
                        chrome://flags → "Insecure origins treated as secure" → Add {window.location.origin}
                      </div>
                    </div>
                  </div>
                </div>
              ) : !isConnected ? (
                <div className="space-y-3">
                  <div className="text-center text-gray-400 text-sm">
                    No device connected
                  </div>
                  <Button
                    variant="primary"
                    size="sm"
                    className="w-full"
                    onClick={handleConnect}
                    disabled={isConnecting}
                  >
                    {isConnecting ? 'Connecting...' : 'Connect SpacePilot'}
                  </Button>
                </div>
              ) : (
                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <div className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                      <span className="text-sm text-green-400">Connected</span>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      className="text-xs"
                      onClick={handleDisconnect}
                    >
                      Disconnect
                    </Button>
                  </div>

                  {/* Button grid */}
                  <div className="grid grid-cols-4 gap-1.5">
                    {Object.entries(buttonStates).map(([buttonNum, state]) => {
                      const num = parseInt(buttonNum);
                      const isPressed = state.pressed;
                      const label = state.label || `B${num}`;

                      return (
                        <Tooltip key={num} content={`Button ${num}${label ? ` - ${label}` : ''}`}>
                          <div
                            className={cn(
                              'relative rounded-md border transition-all duration-100',
                              'w-full h-10',
                              'flex items-center justify-center',
                              'select-none cursor-default',
                              isPressed
                                ? 'bg-cyan-500/20 border-cyan-400 shadow-[0_0_10px_rgba(34,211,238,0.3)]'
                                : 'bg-gray-800/50 border-gray-700 hover:border-gray-600',
                              isPressed && 'transform scale-95'
                            )}
                          >
                            <span
                              className={cn(
                                'font-mono text-xs',
                                isPressed ? 'text-cyan-300' : 'text-gray-400'
                              )}
                            >
                              {label && label.length <= 4 ? label : num}
                            </span>

                            {isPressed && (
                              <div className="absolute inset-0 rounded-md animate-pulse bg-cyan-400/10" />
                            )}
                          </div>
                        </Tooltip>
                      );
                    })}
                  </div>

                  {/* Active button count */}
                  <div className="flex items-center justify-between text-xs">
                    <span className="text-gray-400">Active buttons:</span>
                    <span className="font-mono text-cyan-400">
                      {Object.values(buttonStates).filter(state => state.pressed).length} / 16
                    </span>
                  </div>
                </div>
              )}
            </div>
          </CardContent>
        )}

        {/* Drag handle indicator */}
        <div className="absolute top-1 left-1/2 -translate-x-1/2">
          <Move className="w-3 h-3 text-gray-600" />
        </div>
      </Card>
    </div>
  );
};

/**
 * Minimal floating indicator showing connection status
 */
export const SpacePilotFloatingIndicator: React.FC<{
  onClick?: () => void;
  className?: string;
}> = ({ onClick, className = '' }) => {
  const [isConnected, setIsConnected] = useState(false);
  const [activeButtons, setActiveButtons] = useState(0);

  useEffect(() => {
    const handleButtons = (event: CustomEvent) => {
      const { buttons } = event.detail;
      let count = 0;
      for (let i = 1; i <= 16; i++) {
        const buttonString = `[${i.toString(16).toUpperCase()}]`;
        if (buttons.includes(buttonString)) count++;
      }
      setActiveButtons(count);
    };

    const handleConnect = () => setIsConnected(true);
    const handleDisconnect = () => {
      setIsConnected(false);
      setActiveButtons(0);
    };

    SpaceDriver.addEventListener('buttons', handleButtons);
    SpaceDriver.addEventListener('connect', handleConnect);
    SpaceDriver.addEventListener('disconnect', handleDisconnect);

    return () => {
      SpaceDriver.removeEventListener('buttons', handleButtons);
      SpaceDriver.removeEventListener('connect', handleConnect);
      SpaceDriver.removeEventListener('disconnect', handleDisconnect);
    };
  }, []);

  return (
    <div
      className={cn(
        'fixed bottom-4 right-4 z-50',
        'bg-gray-900/90 backdrop-blur-sm rounded-full',
        'border px-3 py-2 cursor-pointer',
        'transition-all duration-200 hover:scale-105',
        isConnected ? 'border-cyan-500/50' : 'border-gray-700',
        className
      )}
      onClick={onClick}
    >
      <div className="flex items-center gap-2">
        <Gamepad2 className={cn(
          'w-4 h-4',
          isConnected ? 'text-cyan-400' : 'text-gray-500'
        )} />
        <span className={cn(
          'text-xs font-mono',
          isConnected ? 'text-cyan-300' : 'text-gray-400'
        )}>
          {isConnected ? `${activeButtons}/16` : 'OFF'}
        </span>
      </div>
    </div>
  );
};