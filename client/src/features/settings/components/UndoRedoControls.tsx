import React from 'react';
import { Undo, Redo, History } from 'lucide-react';
import { Button } from '@/features/design-system/components/Button';
import { TooltipRoot, TooltipContent, TooltipTrigger } from '@/features/design-system/components/Tooltip';
import { useSettingsHistory } from '../hooks/useSettingsHistory';
import { useKeyboardShortcuts } from '@/hooks/useKeyboardShortcuts';
import { cn } from '@/utils/cn';

interface UndoRedoControlsProps {
  className?: string;
  showHistory?: boolean;
}

export function UndoRedoControls({ className, showHistory = false }: UndoRedoControlsProps) {
  const {
    undo,
    redo,
    canUndo,
    canRedo,
    getHistoryInfo,
    historyLength
  } = useSettingsHistory();

  // Register keyboard shortcuts
  useKeyboardShortcuts({
    'settings-undo': {
      key: 'z',
      ctrl: true,
      description: 'Undo settings change',
      handler: undo,
      enabled: canUndo,
      category: 'Settings'
    },
    'settings-redo': {
      key: 'z',
      ctrl: true,
      shift: true,
      description: 'Redo settings change',
      handler: redo,
      enabled: canRedo,
      category: 'Settings'
    }
  });

  const historyInfo = getHistoryInfo();

  return (
    <div className={cn("flex items-center gap-1", className)}>
      <TooltipRoot>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            onClick={undo}
            disabled={!canUndo}
            className="h-8 w-8"
          >
            <Undo className="h-4 w-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          <div className="text-xs">
            <div>Undo (Ctrl+Z)</div>
            {canUndo && (
              <div className="text-muted-foreground mt-1">
                {historyInfo.currentPosition} of {historyInfo.totalEntries} changes
              </div>
            )}
          </div>
        </TooltipContent>
      </TooltipRoot>

      <TooltipRoot>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            onClick={redo}
            disabled={!canRedo}
            className="h-8 w-8"
          >
            <Redo className="h-4 w-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          <div className="text-xs">
            <div>Redo (Ctrl+Shift+Z)</div>
          </div>
        </TooltipContent>
      </TooltipRoot>

      {showHistory && historyLength > 0 && (
        <TooltipRoot>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={() => {
                // Could open a history panel
                window.dispatchEvent(new CustomEvent('show-settings-history'));
              }}
            >
              <History className="h-4 w-4" />
              <span className="sr-only">History</span>
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            <div className="text-xs">
              <div>Settings History</div>
              <div className="text-muted-foreground mt-1">
                {historyLength} changes recorded
              </div>
            </div>
          </TooltipContent>
        </TooltipRoot>
      )}
    </div>
  );
}