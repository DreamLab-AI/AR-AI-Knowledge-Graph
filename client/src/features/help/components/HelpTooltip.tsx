import React, { useState } from 'react';
import { HelpCircle, Info, ExternalLink } from 'lucide-react';
import { TooltipRoot, TooltipTrigger, TooltipContent } from '@/features/design-system/components/Tooltip';
import { cn } from '@/utils/cn';
import { HelpContent } from '../types';

interface HelpTooltipProps {
  children: React.ReactNode;
  help: HelpContent | string;
  showIndicator?: boolean;
  side?: 'top' | 'right' | 'bottom' | 'left';
  align?: 'start' | 'center' | 'end';
  className?: string;
  indicatorClassName?: string;
}

export function HelpTooltip({
  children,
  help,
  showIndicator = false,
  side = 'top',
  align = 'center',
  className,
  indicatorClassName
}: HelpTooltipProps) {
  const [showDetailed, setShowDetailed] = useState(false);
  const helpContent = typeof help === 'string' ? { id: '', title: '', description: help } as HelpContent : help;

  const tooltipContent = (
    <div className="max-w-xs">
      {helpContent.title && (
        <div className="font-semibold mb-1">{helpContent.title}</div>
      )}
      <div className="text-xs leading-relaxed">
        {helpContent.description}
      </div>
      {helpContent.detailedHelp && !showDetailed && (
        <button
          className="text-xs text-primary hover:underline mt-2 flex items-center gap-1"
          onClick={() => setShowDetailed(true)}
        >
          Learn more <ExternalLink className="h-3 w-3" />
        </button>
      )}
      {showDetailed && helpContent.detailedHelp && (
        <div className="mt-2 pt-2 border-t border-border/50">
          <div className="text-xs leading-relaxed mb-2">
            {helpContent.detailedHelp}
          </div>
          {helpContent.examples && helpContent.examples.length > 0 && (
            <div className="mt-2">
              <div className="font-medium text-xs mb-1">Examples:</div>
              <ul className="list-disc list-inside text-xs space-y-1">
                {helpContent.examples.map((example, i) => (
                  <li key={i}>{example}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );

  return (
    <TooltipRoot delayDuration={300}>
      <TooltipTrigger asChild>
        <div className={cn("relative inline-flex items-center", className)}>
          {children}
          {showIndicator && (
            <div className={cn(
              "ml-1 inline-flex items-center justify-center",
              "w-4 h-4 rounded-full",
              "bg-muted hover:bg-muted/80 transition-colors",
              "cursor-help",
              indicatorClassName
            )}>
              <Info className="h-3 w-3 text-muted-foreground" />
            </div>
          )}
        </div>
      </TooltipTrigger>
      <TooltipContent
        side={side}
        align={align}
        className="bg-popover text-popover-foreground border shadow-md"
      >
        {tooltipContent}
      </TooltipContent>
    </TooltipRoot>
  );
}