import React, { useEffect, useRef, useState } from 'react';
import { X, ChevronLeft, ChevronRight, SkipForward } from 'lucide-react';
import { Button } from '../../design-system/components';
import { cn } from '../../../utils/cn';
import { OnboardingStep } from '../types';

interface OnboardingOverlayProps {
  step: OnboardingStep;
  stepNumber: number;
  totalSteps: number;
  onNext: () => void;
  onPrev: () => void;
  onSkip: () => void;
  hasNext: boolean;
  hasPrev: boolean;
}

export function OnboardingOverlay({
  step,
  stepNumber,
  totalSteps,
  onNext,
  onPrev,
  onSkip,
  hasNext,
  hasPrev
}: OnboardingOverlayProps) {
  const [targetRect, setTargetRect] = useState<DOMRect | null>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);

  // Find and highlight target element
  useEffect(() => {
    if (step.target) {
      const element = document.querySelector(step.target);
      if (element) {
        const rect = element.getBoundingClientRect();
        setTargetRect(rect);

        // Scroll element into view
        element.scrollIntoView({ behavior: 'smooth', block: 'center' });
      }
    } else {
      setTargetRect(null);
    }
  }, [step.target]);

  // Calculate tooltip position
  const getTooltipStyle = (): React.CSSProperties => {
    if (!targetRect || !step.target) {
      // Center the tooltip if no target
      return {
        position: 'fixed',
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)',
        zIndex: 10002
      };
    }

    const margin = 16;
    const position = step.position || 'bottom';
    const style: React.CSSProperties = {
      position: 'fixed',
      zIndex: 10002
    };

    switch (position) {
      case 'top':
        style.bottom = `${window.innerHeight - targetRect.top + margin}px`;
        style.left = `${targetRect.left + targetRect.width / 2}px`;
        style.transform = 'translateX(-50%)';
        break;
      case 'bottom':
        style.top = `${targetRect.bottom + margin}px`;
        style.left = `${targetRect.left + targetRect.width / 2}px`;
        style.transform = 'translateX(-50%)';
        break;
      case 'left':
        style.right = `${window.innerWidth - targetRect.left + margin}px`;
        style.top = `${targetRect.top + targetRect.height / 2}px`;
        style.transform = 'translateY(-50%)';
        break;
      case 'right':
        style.left = `${targetRect.right + margin}px`;
        style.top = `${targetRect.top + targetRect.height / 2}px`;
        style.transform = 'translateY(-50%)';
        break;
    }

    return style;
  };

  return (
    <>
      {/* Backdrop */}
      <div className="fixed inset-0 z-10000 bg-black/50 animate-in fade-in duration-200" />

      {/* Highlight cutout */}
      {targetRect && (
        <div
          className="fixed z-10001 ring-4 ring-primary/50 rounded-md animate-in zoom-in-95 duration-200"
          style={{
            top: targetRect.top - 4,
            left: targetRect.left - 4,
            width: targetRect.width + 8,
            height: targetRect.height + 8,
            boxShadow: '0 0 0 9999px rgba(0, 0, 0, 0.5)'
          }}
        />
      )}

      {/* Tooltip */}
      <div
        ref={tooltipRef}
        style={getTooltipStyle()}
        className={cn(
          "w-96 bg-background border rounded-lg shadow-2xl p-6",
          "animate-in fade-in zoom-in-95 duration-200"
        )}
      >
        {/* Header */}
        <div className="flex items-start justify-between mb-4">
          <div>
            <h3 className="text-lg font-semibold">{step.title}</h3>
            <p className="text-sm text-muted-foreground mt-1">
              Step {stepNumber} of {totalSteps}
            </p>
          </div>
          {step.skipable !== false && (
            <Button
              variant="ghost"
              size="icon"
              onClick={onSkip}
              className="h-8 w-8 -mr-2 -mt-2"
            >
              <X className="h-4 w-4" />
            </Button>
          )}
        </div>

        {/* Content */}
        <div className="mb-6">
          <p className="text-sm leading-relaxed">{step.description}</p>
        </div>

        {/* Progress dots */}
        <div className="flex items-center justify-center gap-1 mb-6">
          {Array.from({ length: totalSteps }).map((_, i) => (
            <div
              key={i}
              className={cn(
                "h-2 w-2 rounded-full transition-colors",
                i === stepNumber - 1 ? "bg-primary" : "bg-muted"
              )}
            />
          ))}
        </div>

        {/* Actions */}
        <div className="flex items-center justify-between">
          <Button
            variant="ghost"
            size="sm"
            onClick={onPrev}
            disabled={!hasPrev}
          >
            <ChevronLeft className="h-4 w-4 mr-1" />
            {step.prevButtonText || 'Previous'}
          </Button>

          <div className="flex items-center gap-2">
            {step.skipable !== false && hasNext && (
              <Button
                variant="ghost"
                size="sm"
                onClick={onSkip}
              >
                <SkipForward className="h-4 w-4 mr-1" />
                Skip tour
              </Button>
            )}

            <Button
              size="sm"
              onClick={onNext}
            >
              {step.nextButtonText || (hasNext ? 'Next' : 'Finish')}
              {hasNext && <ChevronRight className="h-4 w-4 ml-1" />}
            </Button>
          </div>
        </div>
      </div>
    </>
  );
}