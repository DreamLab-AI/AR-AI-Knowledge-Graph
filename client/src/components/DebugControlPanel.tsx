import React, { useState, useEffect, useCallback } from 'react';
import { Bug, X } from 'lucide-react';
import { useKeyboardShortcuts } from '@/hooks/useKeyboardShortcuts';
import { debugControl, DebugCategory } from '@/utils/console';
import { debugState } from '@/utils/debugState';
import { 
  Dialog, 
  DialogContent, 
  DialogHeader, 
  DialogTitle, 
  DialogDescription 
} from '@/features/design-system/components/Dialog';
import { Switch } from '@/features/design-system/components/Switch';
import { Label } from '@/features/design-system/components/Label';
import { Button } from '@/features/design-system/components/Button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/features/design-system/components/Card';
import { cn } from '@/utils/classNameUtils';

interface DebugControlPanelProps {
  className?: string;
}

export function DebugControlPanel({ className }: DebugControlPanelProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [mainDebugEnabled, setMainDebugEnabled] = useState(false);
  const [enabledCategories, setEnabledCategories] = useState<Set<DebugCategory>>(new Set());
  const [dataDebugEnabled, setDataDebugEnabled] = useState(false);
  const [performanceDebugEnabled, setPerformanceDebugEnabled] = useState(false);

  // Load initial state
  useEffect(() => {
    const loadState = () => {
      setMainDebugEnabled(debugState.isEnabled());
      setDataDebugEnabled(debugState.isDataDebugEnabled());
      setPerformanceDebugEnabled(debugState.isPerformanceDebugEnabled());
      setEnabledCategories(new Set(debugControl.getEnabledCategories()));
    };

    loadState();

    // Listen for storage changes (in case debug state changes in another tab)
    const handleStorageChange = (e: StorageEvent) => {
      if (e.key?.startsWith('debug.')) {
        loadState();
      }
    };

    window.addEventListener('storage', handleStorageChange);
    return () => window.removeEventListener('storage', handleStorageChange);
  }, []);

  // Register keyboard shortcut
  useKeyboardShortcuts({
    'debug-panel': {
      key: 'd',
      ctrl: true,
      shift: true,
      description: 'Toggle Debug Control Panel',
      category: 'Development',
      handler: () => setIsOpen(prev => !prev),
    }
  });

  const handleMainToggle = useCallback((checked: boolean) => {
    if (checked) {
      debugControl.enable();
    } else {
      debugControl.disable();
    }
    setMainDebugEnabled(checked);
  }, []);

  const handleCategoryToggle = useCallback((category: DebugCategory, checked: boolean) => {
    if (checked) {
      debugControl.enableCategory(category);
      setEnabledCategories(prev => new Set([...prev, category]));
    } else {
      debugControl.disableCategory(category);
      setEnabledCategories(prev => {
        const next = new Set(prev);
        next.delete(category);
        return next;
      });
    }
  }, []);

  const handleDataDebugToggle = useCallback((checked: boolean) => {
    if (checked) {
      debugControl.enableData();
    } else {
      debugControl.disableData();
    }
    setDataDebugEnabled(checked);
  }, []);

  const handlePerformanceDebugToggle = useCallback((checked: boolean) => {
    if (checked) {
      debugControl.enablePerformance();
    } else {
      debugControl.disablePerformance();
    }
    setPerformanceDebugEnabled(checked);
  }, []);

  const handlePresetClick = useCallback((preset: keyof typeof debugControl.presets) => {
    debugControl.presets[preset]();
    // Reload state after preset is applied
    setTimeout(() => {
      setMainDebugEnabled(debugState.isEnabled());
      setDataDebugEnabled(debugState.isDataDebugEnabled());
      setPerformanceDebugEnabled(debugState.isPerformanceDebugEnabled());
      setEnabledCategories(new Set(debugControl.getEnabledCategories()));
    }, 50);
  }, []);

  const categoryInfo: Record<DebugCategory, { label: string; description: string }> = {
    [DebugCategory.GENERAL]: {
      label: 'General',
      description: 'General debug messages and logs'
    },
    [DebugCategory.VOICE]: {
      label: 'Voice',
      description: 'Voice recognition and WebRTC debugging'
    },
    [DebugCategory.WEBSOCKET]: {
      label: 'WebSocket',
      description: 'WebSocket connection and message debugging'
    },
    [DebugCategory.PERFORMANCE]: {
      label: 'Performance',
      description: 'Performance metrics and timing logs'
    },
    [DebugCategory.DATA]: {
      label: 'Data',
      description: 'Data flow and state management debugging'
    },
    [DebugCategory.RENDERING]: {
      label: '3D Rendering',
      description: '3D scene and rendering debugging'
    },
    [DebugCategory.AUTH]: {
      label: 'Authentication',
      description: 'Authentication and authorization debugging'
    },
    [DebugCategory.ERROR]: {
      label: 'Errors',
      description: 'Error messages and stack traces'
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={setIsOpen}>
      <DialogContent className={cn("max-w-2xl", className)}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Bug className="h-5 w-5" />
            Debug Control Panel
          </DialogTitle>
          <DialogDescription>
            Configure debug settings and logging categories. Use Ctrl+Shift+D to toggle this panel.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-6">
          {/* Main Debug Toggle */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Master Debug Control</CardTitle>
              <CardDescription>
                Enable or disable all debug functionality
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="flex items-center justify-between">
                <Label htmlFor="main-debug" className="font-medium">
                  Debug Mode
                </Label>
                <Switch
                  id="main-debug"
                  checked={mainDebugEnabled}
                  onCheckedChange={handleMainToggle}
                />
              </div>
            </CardContent>
          </Card>

          {/* Debug Categories */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Debug Categories</CardTitle>
              <CardDescription>
                Enable specific debug categories for targeted logging
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {Object.entries(categoryInfo).map(([category, info]) => (
                <div key={category} className="flex items-start justify-between">
                  <div className="space-y-0.5">
                    <Label htmlFor={`category-${category}`} className="font-medium">
                      {info.label}
                    </Label>
                    <p className="text-sm text-muted-foreground">
                      {info.description}
                    </p>
                  </div>
                  <Switch
                    id={`category-${category}`}
                    checked={enabledCategories.has(category as DebugCategory)}
                    onCheckedChange={(checked) => handleCategoryToggle(category as DebugCategory, checked)}
                    disabled={!mainDebugEnabled}
                  />
                </div>
              ))}
            </CardContent>
          </Card>

          {/* Special Debug Modes */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Special Debug Modes</CardTitle>
              <CardDescription>
                Additional debug features for specific use cases
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-start justify-between">
                <div className="space-y-0.5">
                  <Label htmlFor="data-debug" className="font-medium">
                    Data Debug
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    Log all data flow and state changes
                  </p>
                </div>
                <Switch
                  id="data-debug"
                  checked={dataDebugEnabled}
                  onCheckedChange={handleDataDebugToggle}
                  disabled={!mainDebugEnabled}
                />
              </div>
              <div className="flex items-start justify-between">
                <div className="space-y-0.5">
                  <Label htmlFor="perf-debug" className="font-medium">
                    Performance Debug
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    Enable performance profiling and timing logs
                  </p>
                </div>
                <Switch
                  id="perf-debug"
                  checked={performanceDebugEnabled}
                  onCheckedChange={handlePerformanceDebugToggle}
                  disabled={!mainDebugEnabled}
                />
              </div>
            </CardContent>
          </Card>

          {/* Debug Presets */}
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Debug Presets</CardTitle>
              <CardDescription>
                Quick presets for common debug configurations
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-2 gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => handlePresetClick('off')}
                >
                  Off
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => handlePresetClick('minimal')}
                >
                  Minimal
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => handlePresetClick('standard')}
                >
                  Standard
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => handlePresetClick('verbose')}
                >
                  Verbose
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      </DialogContent>
    </Dialog>
  );
}